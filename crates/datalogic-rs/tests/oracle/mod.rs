//! A reference interpreter for JSONLogic rules: the semantics the optimized
//! engine must agree with.
//!
//! It walks the rule JSON directly, with no compile step, no folding, no CSE
//! and no fast paths, and keeps its own frame stack. Everything that decides
//! *which* expression runs and *what data* it sees is implemented here:
//! templates, `var` / `val` / `exists` / `missing` / `missing_some`, the
//! control-flow operators, every iterator, and `try` / `throw`. Those are the
//! operators the optimizer rewrites and specialises (dead-code elimination,
//! folding, CSE, scope binding, `FastPredicate`, the `map` / `reduce`
//! fusions), so they are the ones an independent definition has to cover.
//!
//! Every other operator computes a value from its arguments (arithmetic,
//! comparison, strings, datetime, tensor, ...). The oracle hands those back
//! to an engine built with constant folding off, with every argument that
//! is an expression replaced by a *slot*: a custom operator that calls back
//! into the oracle to evaluate that expression, in the oracle's own scope,
//! when the operator asks for it. The operator therefore still decides
//! which arguments run and in what order, and its errors win the way they
//! do in the engine, but no argument it sees was produced by the engine's
//! optimizer. Literal arguments (scalars and arrays) stay literal, because
//! a few operators distinguish a literal array from a computed one
//! (`{"+": [[1, 2]]}` is an error, `{"+": [{"var": "xs"}]}` a sum).
//!
//! What the oracle treats as a literal is decided syntactically, from the
//! rule JSON. Where the engine instead decides from the compiled node, and a
//! folded subexpression therefore reads as a literal, the two can disagree;
//! that is a compile-path split and the differential tests report it.

#![allow(dead_code)]

use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::HashSet;
use std::rc::Rc;

use datalogic_rs::bumpalo::Bump;
use datalogic_rs::operator::EvalContext;
use datalogic_rs::{CustomOperator, DataValue, Engine, Error, ErrorKind, Result};
use datavalue::{NumberValue, OwnedDataValue as V};

/// The name the slot operator is registered under on the delegate engine.
const SLOT: &str = "__oracle_slot";

/// Operators the oracle implements itself. Every other built-in is handed
/// to the engine (see the module docs); `delegation_is_sound` in
/// `oracle_test.rs` checks that none of those pushes a frame, reads the
/// data context or catches errors.
pub const IMPLEMENTED: &[&str] = &[
    "val",
    "var",
    "exists",
    "missing",
    "missing_some",
    "and",
    "or",
    "if",
    "?:",
    "switch",
    "match",
    "try",
    "throw",
    "filter",
    "map",
    "all",
    "some",
    "none",
    "reduce",
    "sort",
    "group_by",
    "distinct",
];

/// Delegated operators that read the data context: they read the root
/// input, which the delegate call is given, never the current frame.
pub const ROOT_READERS: &[&str] = &["fractional"];

// ---------------------------------------------------------------------------
// Scope
// ---------------------------------------------------------------------------

/// One pushed frame, mirroring what each operator pushes in the engine.
#[derive(Clone, Debug)]
enum Frame {
    /// An array element (`map`, `filter`, quantifiers, `sort` / `group_by` /
    /// `distinct` keys), or the scalar a `map` over a scalar runs on.
    Indexed { data: V, index: usize },
    /// An object entry.
    Keyed { data: V, index: usize, key: String },
    /// A `reduce` step. Its data is `current`.
    Reduce { current: V, accumulator: V },
    /// A `try` catch arm, over the caught error object.
    Data(V),
}

impl Frame {
    fn data(&self) -> &V {
        match self {
            Frame::Indexed { data, .. } | Frame::Keyed { data, .. } | Frame::Data(data) => data,
            Frame::Reduce { current, .. } => current,
        }
    }

    fn index(&self) -> Option<usize> {
        match self {
            Frame::Indexed { index, .. } | Frame::Keyed { index, .. } => Some(*index),
            _ => None,
        }
    }

    fn key(&self) -> Option<&str> {
        match self {
            Frame::Keyed { key, .. } => Some(key),
            _ => None,
        }
    }

    fn reduce_current(&self) -> Option<&V> {
        match self {
            Frame::Reduce { current, .. } => Some(current),
            _ => None,
        }
    }

    fn reduce_accumulator(&self) -> Option<&V> {
        match self {
            Frame::Reduce { accumulator, .. } => Some(accumulator),
            _ => None,
        }
    }
}

#[derive(Clone)]
struct Scope {
    root: Rc<V>,
    /// Oldest first; the last one is the current frame.
    frames: Rc<Vec<Frame>>,
}

/// Frames climbed for a data read at level `level`: levels come in pairs,
/// `[[2k-1]]` and `[[2k]]` both climb `k`.
fn data_climb(level: u64) -> u64 {
    level / 2 + (level & 1)
}

/// Frames climbed for an `index` / `key` read, or `None` at an even,
/// non-zero level, where those are ordinary field names.
fn metadata_climb(level: u64) -> Option<u64> {
    (level == 0 || level % 2 == 1).then_some(level / 2)
}

impl Scope {
    fn new(root: V) -> Self {
        Scope {
            root: Rc::new(root),
            frames: Rc::new(Vec::new()),
        }
    }

    fn top(&self) -> Option<&Frame> {
        self.frames.last()
    }

    fn current(&self) -> &V {
        self.top().map_or(&self.root, Frame::data)
    }

    fn push(&self, frame: Frame) -> Scope {
        let mut frames = (*self.frames).clone();
        frames.push(frame);
        Scope {
            root: self.root.clone(),
            frames: Rc::new(frames),
        }
    }

    /// The frame `climb` frames above the current one, or `None` for the
    /// root (a climb at or past the outermost frame clamps to it).
    fn frame_at_climb(&self, climb: u64) -> Option<&Frame> {
        let n = self.frames.len() as u64;
        if climb >= n {
            return None;
        }
        Some(&self.frames[(n - 1 - climb) as usize])
    }

    fn data_at_level(&self, level: u64) -> &V {
        self.frame_at_climb(data_climb(level))
            .map_or(&self.root, Frame::data)
    }

    fn metadata_frame(&self, level: u64) -> Option<&Frame> {
        let climb = metadata_climb(level)?;
        if climb == 0 {
            return self.top();
        }
        self.frame_at_climb(climb)
    }
}

// ---------------------------------------------------------------------------
// Paths
// ---------------------------------------------------------------------------

fn object_get<'v>(pairs: &'v [(String, V)], key: &str) -> Option<&'v V> {
    pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v)
}

/// One step by a string segment: a key on an object, an index on an array.
fn step<'v>(cur: &'v V, seg: &str) -> Option<&'v V> {
    match cur {
        V::Object(pairs) => object_get(pairs, seg),
        V::Array(items) => items.get(seg.parse::<usize>().ok()?),
        _ => None,
    }
}

/// Walk segments that are already split (a compiled path).
fn walk<'v>(cur: &'v V, segments: &[String]) -> Option<&'v V> {
    segments.iter().try_fold(cur, |v, seg| step(v, seg))
}

/// Walk a dot-separated path; the empty path is the value itself.
fn access_path<'v>(cur: &'v V, path: &str) -> Option<&'v V> {
    if path.is_empty() {
        return Some(cur);
    }
    path.split('.').try_fold(cur, step)
}

/// One evaluated path element: a string is a dotted path, a non-negative
/// integer an index (or the key spelling that integer).
fn apply_path_element<'v>(cur: &'v V, elem: &V) -> Option<&'v V> {
    if let Some(s) = elem.as_str() {
        return access_path(cur, s);
    }
    let i = elem.as_i64().filter(|i| *i >= 0)?;
    match cur {
        V::Array(items) => items.get(i as usize),
        V::Object(_) => access_path(cur, &i.to_string()),
        _ => None,
    }
}

/// A dotted path split the way a literal `var` path is compiled.
fn split_path(path: &str) -> Vec<String> {
    if path.is_empty() {
        return Vec::new();
    }
    path.split('.').map(str::to_string).collect()
}

/// The path string an evaluated `val` argument names.
fn path_string(v: &V) -> String {
    match v {
        V::String(s) => s.clone(),
        V::Number(n) => match n.as_i64() {
            Some(i) if (0..100).contains(&i) => i.to_string(),
            _ => n.to_string(),
        },
        _ => String::new(),
    }
}

/// An evaluated path, or a list of path elements, dotted: how a
/// missing-variable error names a computed path.
fn value_path_name(v: &V) -> String {
    match v {
        V::String(s) => s.clone(),
        V::Number(n) => n.to_string(),
        V::Array(items) => items
            .iter()
            .map(value_path_name)
            .collect::<Vec<_>>()
            .join("."),
        other => other.to_json_string(),
    }
}

/// A one-element numeric array: a `[level]` marker, read at runtime.
fn level_marker_value(v: &V) -> Option<i64> {
    match v {
        V::Array(items) if items.len() == 1 => items[0].as_i64(),
        _ => None,
    }
}

/// A literal `[N]` marker in the rule, with the level clamped the way the
/// compiler clamps it.
fn literal_level_marker(node: &V) -> Option<u64> {
    let level = level_marker_value(node)?;
    Some(level.unsigned_abs().min(u64::from(u32::MAX)))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Meta {
    Index,
    Key,
}

fn meta_of(path: &str) -> Option<Meta> {
    match path {
        "index" => Some(Meta::Index),
        "key" => Some(Meta::Key),
        _ => None,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ReduceHint {
    None,
    Current,
    Accumulator,
    CurrentPath,
    AccumulatorPath,
}

/// A literal `var` path: its reduce hint and segments.
fn var_path(path: &str) -> (ReduceHint, Vec<String>) {
    match path {
        "current" => (ReduceHint::Current, vec![path.to_string()]),
        "accumulator" => (ReduceHint::Accumulator, vec![path.to_string()]),
        _ => {
            if let Some(rest) = path.strip_prefix("current.") {
                let mut segs = vec!["current".to_string()];
                segs.extend(split_path(rest));
                (ReduceHint::CurrentPath, segs)
            } else if let Some(rest) = path.strip_prefix("accumulator.") {
                let mut segs = vec!["accumulator".to_string()];
                segs.extend(split_path(rest));
                (ReduceHint::AccumulatorPath, segs)
            } else {
                (ReduceHint::None, split_path(path))
            }
        }
    }
}

/// A literal `val` segment: a string as a whole key, or a non-negative
/// integer.
fn literal_segment(node: &V) -> Option<String> {
    match node {
        V::String(s) => Some(s.clone()),
        V::Number(n) => n.as_i64().filter(|i| *i >= 0).map(|i| i.to_string()),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Values
// ---------------------------------------------------------------------------

fn invalid_args() -> Error {
    Error::invalid_arguments("Invalid Arguments")
}

fn type_name(v: &V) -> &'static str {
    match v {
        V::Null => "null",
        V::Bool(_) => "boolean",
        V::Number(_) => "number",
        V::String(_) => "string",
        V::Array(_) => "array",
        V::Object(_) => "object",
        V::DateTime(_) => "datetime",
        V::Duration(_) => "duration",
        V::Tensor(_) => "tensor",
    }
}

fn int(i: usize) -> V {
    V::Number(NumberValue::from_i64(i as i64))
}

/// The ordering `sort` uses for keys.
fn compare_values(a: &V, b: &V) -> Ordering {
    fn rank(v: &V) -> u8 {
        match v {
            V::Null => 0,
            V::Bool(_) => 1,
            V::Number(_) => 2,
            V::String(_) | V::DateTime(_) | V::Duration(_) => 3,
            V::Array(_) => 4,
            V::Object(_) => 5,
            V::Tensor(_) => 6,
        }
    }
    match (a, b) {
        (V::Null, V::Null) => Ordering::Equal,
        (V::Bool(x), V::Bool(y)) => x.cmp(y),
        // By exact value: whole numbers as integers (an `f64` comparison
        // calls neighbours above 2^53 equal), anything else as `f64`.
        (V::Number(x), V::Number(y)) => {
            let whole = |n: &NumberValue| match *n {
                NumberValue::Integer(i) => Some(i128::from(i)),
                NumberValue::Float(f) if f.fract() == 0.0 && f.abs() < 1e38 => Some(f as i128),
                NumberValue::Float(_) => None,
            };
            match (whole(x), whole(y)) {
                (Some(a), Some(b)) => a.cmp(&b),
                _ => x
                    .as_f64()
                    .partial_cmp(&y.as_f64())
                    .unwrap_or(Ordering::Equal),
            }
        }
        (V::String(x), V::String(y)) => x.cmp(y),
        (V::Array(_), V::Array(_)) | (V::Object(_), V::Object(_)) => Ordering::Equal,
        (V::Tensor(x), V::Tensor(y)) => {
            (x.dtype().name(), x.shape(), x.data()).cmp(&(y.dtype().name(), y.shape(), y.data()))
        }
        _ => rank(a).cmp(&rank(b)),
    }
}

/// The `{"tensor": {"dtype", "shape", "data"}}` wire form, which compiles as
/// a literal argument rather than a rule.
fn is_tensor_wire_body(v: &V) -> bool {
    let V::Object(fields) = v else {
        return false;
    };
    if fields.len() != 3 {
        return false;
    }
    let get = |k: &str| object_get(fields, k);
    matches!(get("dtype"), Some(V::String(_)))
        && matches!(get("data"), Some(V::String(_)))
        && matches!(get("shape"), Some(V::Array(dims)) if dims.iter().all(|d| matches!(d, V::Number(_))))
}

// ---------------------------------------------------------------------------
// Delegation
// ---------------------------------------------------------------------------

/// What a slot evaluates to.
#[derive(Clone)]
enum SlotSrc {
    /// A rule expression, evaluated by the oracle in the call's scope.
    Node(V),
    /// An already computed value.
    Value(V),
}

struct Pending {
    oracle: Rc<Inner>,
    slots: Rc<Vec<SlotSrc>>,
    scope: Scope,
}

thread_local! {
    /// Delegate calls in flight, innermost last. A slot always belongs to
    /// the innermost one: an outer call's slots only run once every call
    /// they started has returned.
    static PENDING: RefCell<Vec<Pending>> = const { RefCell::new(Vec::new()) };
}

struct PendingGuard;

impl PendingGuard {
    fn push(pending: Pending) -> Self {
        PENDING.with(|p| p.borrow_mut().push(pending));
        PendingGuard
    }
}

impl Drop for PendingGuard {
    fn drop(&mut self) {
        PENDING.with(|p| {
            p.borrow_mut().pop();
        });
    }
}

/// The slot operator: `{"__oracle_slot": i}` evaluates slot `i` of the
/// innermost delegate call.
struct Slot;

impl CustomOperator for Slot {
    fn evaluate<'a>(
        &self,
        args: &[&'a DataValue<'a>],
        _ctx: &mut EvalContext<'_, 'a>,
        arena: &'a Bump,
    ) -> Result<&'a DataValue<'a>> {
        let index = args.first().and_then(|v| v.as_i64()).expect("slot index") as usize;
        let (oracle, src, scope) = PENDING.with(|p| {
            let p = p.borrow();
            let top = p.last().expect("a slot runs inside a delegate call");
            (
                top.oracle.clone(),
                top.slots[index].clone(),
                top.scope.clone(),
            )
        });
        let value = match src {
            SlotSrc::Node(node) => oracle.eval(&node, &scope)?,
            SlotSrc::Value(value) => value,
        };
        Ok(arena.alloc(value.to_arena(arena)))
    }
}

// ---------------------------------------------------------------------------
// The interpreter
// ---------------------------------------------------------------------------

/// The reference interpreter for one rule flavour (templating mode and
/// template-key escape).
pub struct Oracle(Rc<Inner>);

struct Inner {
    /// Evaluates the delegated operators. Built without constant folding,
    /// so a delegated call runs its operator's general body.
    delegate: Engine,
    builtins: HashSet<&'static str>,
    templating: bool,
    escape: Option<char>,
    /// `MissingVar::Error`: a read that finds nothing raises.
    missing_var_error: bool,
}

impl Oracle {
    pub fn new(templating: bool, escape: Option<char>) -> Self {
        Self::with_missing_var_error(templating, escape, false)
    }

    /// The oracle for an engine configured with `MissingVar::Error` when
    /// `missing_var_error` holds.
    pub fn with_missing_var_error(
        templating: bool,
        escape: Option<char>,
        missing_var_error: bool,
    ) -> Self {
        let delegate = Engine::builder()
            .with_constant_folding(false)
            .add_operator(SLOT, Slot)
            .build();
        let builtins = delegate.builtin_operator_names().collect();
        Oracle(Rc::new(Inner {
            delegate,
            builtins,
            templating,
            escape,
            missing_var_error,
        }))
    }

    /// Evaluate `rule` against `data`. A rule the engine refuses to compile
    /// is an error here too.
    pub fn evaluate(&self, rule: &V, data: &V) -> Result<V> {
        self.0.check(rule)?;
        self.0.eval(rule, &Scope::new(data.clone()))
    }
}

/// An operator's argument-count policy: what happens below the minimum and
/// above the maximum, before any argument is evaluated.
#[derive(Clone, Copy)]
enum OnMissing {
    InvalidArgs,
    Return(fn() -> V),
}

#[derive(Clone, Copy)]
enum OnExtra {
    Ignore,
    InvalidArgs,
}

fn null() -> V {
    V::Null
}
fn empty_array() -> V {
    V::Array(Vec::new())
}
fn true_() -> V {
    V::Bool(true)
}

fn arity(
    len: usize,
    min: usize,
    max: Option<usize>,
    on_missing: OnMissing,
    on_extra: OnExtra,
) -> Option<Result<V>> {
    if len < min {
        return Some(match on_missing {
            OnMissing::InvalidArgs => Err(invalid_args()),
            OnMissing::Return(value) => Ok(value()),
        });
    }
    if let Some(max) = max
        && len > max
        && matches!(on_extra, OnExtra::InvalidArgs)
    {
        return Some(Err(invalid_args()));
    }
    None
}

/// An iterator's source, resolved.
enum Items {
    Array(Vec<V>),
    Object(Vec<(String, V)>),
    Scalar(V),
}

impl Inner {
    fn truthy(&self, v: &V) -> bool {
        self.delegate.truthy_of(v)
    }

    /// A read that found `found`: the value, or for nothing `null` or the
    /// missing-variable error.
    fn read(&self, found: Option<V>, name: impl FnOnce() -> String) -> Result<V> {
        match found {
            Some(v) => Ok(v),
            None if self.missing_var_error => Err(Error::variable_not_found(name())),
            None => Ok(V::Null),
        }
    }

    // ----- compile-time checks ---------------------------------------------

    /// What the compiler rejects: a multi-key object outside templating
    /// mode, anywhere the compiler reaches.
    fn check(&self, node: &V) -> Result<()> {
        match node {
            V::Object(pairs) if pairs.len() > 1 => {
                if !self.templating {
                    return Err(Error::invalid_operator("Unknown Operator"));
                }
                pairs.iter().try_for_each(|(_, v)| self.check(v))
            }
            V::Object(pairs) if pairs.len() == 1 => {
                let (op, argv) = &pairs[0];
                if self.builtins.contains(op.as_str()) && !self.escaped(op) {
                    // These calls compile without compiling their argument.
                    if matches!(op.as_str(), "and" | "or" | "if" | "?:")
                        && !matches!(argv, V::Array(_))
                    {
                        return Ok(());
                    }
                    if op == "tensor" && is_tensor_wire_body(argv) {
                        return Ok(());
                    }
                }
                self.check(argv)
            }
            V::Array(items) => items.iter().try_for_each(|v| self.check(v)),
            _ => Ok(()),
        }
    }

    fn escaped(&self, key: &str) -> bool {
        self.templating && self.escape.is_some_and(|c| key.starts_with(c))
    }

    // ----- evaluation ------------------------------------------------------

    fn eval(self: &Rc<Self>, node: &V, sc: &Scope) -> Result<V> {
        match node {
            V::Object(pairs) if pairs.len() > 1 => self.template(pairs, sc),
            V::Object(pairs) if pairs.len() == 1 => {
                let (op, argv) = &pairs[0];
                self.call(op, argv, sc)
            }
            V::Array(items) => items
                .iter()
                .map(|item| self.eval(item, sc))
                .collect::<Result<Vec<_>>>()
                .map(V::Array),
            other => Ok(other.clone()),
        }
    }

    /// An output template (templating mode). An escaped key loses one
    /// escape prefix.
    fn template(self: &Rc<Self>, pairs: &[(String, V)], sc: &Scope) -> Result<V> {
        if !self.templating {
            return Err(Error::invalid_operator("Unknown Operator"));
        }
        let strip = pairs.iter().any(|(k, _)| self.escaped(k));
        let mut out = Vec::with_capacity(pairs.len());
        for (key, value) in pairs {
            let value = self.eval(value, sc)?;
            let key = match self.escape {
                Some(c) if strip => key.strip_prefix(c).unwrap_or(key),
                _ => key.as_str(),
            };
            out.push((key.to_string(), value));
        }
        Ok(V::Object(out))
    }

    fn call(self: &Rc<Self>, op: &str, argv: &V, sc: &Scope) -> Result<V> {
        if self.escaped(op) || (self.templating && !self.builtins.contains(op)) {
            return self.template(&[(op.to_string(), argv.clone())], sc);
        }
        if !self.builtins.contains(op) {
            return Err(Error::invalid_operator(op.to_string()));
        }
        if matches!(op, "and" | "or" | "if" | "?:") && !matches!(argv, V::Array(_)) {
            return Err(invalid_args());
        }
        if op == "tensor" && is_tensor_wire_body(argv) {
            return self.run_delegate(op, argv.clone(), Vec::new(), sc);
        }
        let args: Vec<&V> = match argv {
            V::Array(items) => items.iter().collect(),
            other => vec![other],
        };
        match op {
            "var" => self.var(&args, sc),
            "val" => self.val(&args, sc),
            "exists" => self.exists(&args, sc),
            "missing" => self.missing(&args, sc),
            "missing_some" => self.missing_some(&args, sc),
            "and" | "or" => self.short_circuit(&args, op == "or", sc),
            "if" | "?:" => self.if_(&args, sc),
            "switch" | "match" => self.switch(&args, sc),
            "try" => self.try_(&args, sc),
            "throw" => self.throw(&args, sc),
            "filter" | "map" | "all" | "some" | "none" => self.each(op, &args, sc),
            "reduce" => self.reduce(&args, sc),
            "sort" => self.sort(&args, sc),
            "group_by" | "distinct" => self.keyed(op, &args, sc),
            _ => self.delegate(op, &args, sc),
        }
    }

    // ----- delegation ------------------------------------------------------

    /// Replace every expression in `node` with a slot, keeping scalars and
    /// array structure literal.
    fn slotify(node: &V, slots: &mut Vec<SlotSrc>) -> V {
        match node {
            V::Array(items) => V::Array(items.iter().map(|i| Self::slotify(i, slots)).collect()),
            V::Object(pairs) if !pairs.is_empty() => {
                slots.push(SlotSrc::Node(node.clone()));
                V::Object(vec![(SLOT.to_string(), int(slots.len() - 1))])
            }
            other => other.clone(),
        }
    }

    fn run_delegate(
        self: &Rc<Self>,
        op: &str,
        argv: V,
        slots: Vec<SlotSrc>,
        sc: &Scope,
    ) -> Result<V> {
        let rule = V::Object(vec![(op.to_string(), argv)]);
        let logic = self.delegate.compile(&rule)?;
        let _guard = PendingGuard::push(Pending {
            oracle: self.clone(),
            slots: Rc::new(slots),
            scope: sc.clone(),
        });
        let mut session = self.delegate.session();
        session.eval(&logic, &*sc.root)
    }

    /// Call built-in `op` on unevaluated argument expressions.
    fn delegate(self: &Rc<Self>, op: &str, args: &[&V], sc: &Scope) -> Result<V> {
        let mut slots = Vec::new();
        let args = args.iter().map(|a| Self::slotify(a, &mut slots)).collect();
        self.run_delegate(op, V::Array(args), slots, sc)
    }

    /// Call built-in `op` on values that are already computed.
    fn delegate_values(self: &Rc<Self>, op: &str, values: &[V], sc: &Scope) -> Result<V> {
        let slots: Vec<SlotSrc> = values.iter().cloned().map(SlotSrc::Value).collect();
        let args = (0..values.len())
            .map(|i| V::Object(vec![(SLOT.to_string(), int(i))]))
            .collect();
        self.run_delegate(op, V::Array(args), slots, sc)
    }

    fn strict_eq(self: &Rc<Self>, a: &V, b: &V, sc: &Scope) -> Result<bool> {
        let v = self.delegate_values("===", &[a.clone(), b.clone()], sc)?;
        Ok(v.as_bool().expect("=== returns a boolean"))
    }

    // ----- variables -------------------------------------------------------

    fn var(self: &Rc<Self>, args: &[&V], sc: &Scope) -> Result<V> {
        let Some(first) = args.first() else {
            return Ok(sc.current().clone());
        };
        if literal_level_marker(first).is_some() {
            return self.val(args, sc);
        }
        let default = args.get(1).copied();
        match first {
            V::String(s) => {
                let (hint, segments) = var_path(s);
                self.resolve_compiled(sc, &segments, hint, default)
            }
            V::Number(n) => {
                let segments = split_path(&n.to_string());
                self.resolve_compiled(sc, &segments, ReduceHint::None, default)
            }
            _ if args.len() >= 2 => self.var_default(args[0], args[1], sc),
            _ => self.val_dynamic(args, sc),
        }
    }

    fn val(self: &Rc<Self>, args: &[&V], sc: &Scope) -> Result<V> {
        let Some(first) = args.first() else {
            return Ok(sc.current().clone());
        };
        if let Some(level) = literal_level_marker(first) {
            let tail = &args[1..];
            if metadata_climb(level).is_some()
                && let [V::String(path)] = tail
                && let Some(meta) = meta_of(path)
            {
                return Ok(self.metadata(meta, level, sc));
            }
            let Some(segments) = tail
                .iter()
                .map(|a| literal_segment(a))
                .collect::<Option<Vec<_>>>()
            else {
                return self.val_dynamic(args, sc);
            };
            let data = if level == 0 {
                sc.current()
            } else {
                sc.data_at_level(level)
            };
            return self.read(walk(data, &segments).cloned(), || segments.join("."));
        }
        if args.len() == 1 {
            return match first {
                V::String(s) if !s.is_empty() => {
                    let hint = match s.as_str() {
                        "current" => ReduceHint::Current,
                        "accumulator" => ReduceHint::Accumulator,
                        _ => ReduceHint::None,
                    };
                    self.resolve_compiled(sc, std::slice::from_ref(s), hint, None)
                }
                _ => self.val_dynamic(args, sc),
            };
        }
        let segments = args
            .iter()
            .map(|a| literal_segment(a))
            .collect::<Option<Vec<_>>>();
        match segments {
            Some(segments) => {
                let hint = match first {
                    V::String(s) if s == "current" => ReduceHint::CurrentPath,
                    V::String(s) if s == "accumulator" => ReduceHint::AccumulatorPath,
                    _ => ReduceHint::None,
                };
                self.resolve_compiled(sc, &segments, hint, None)
            }
            None => self.val_dynamic(args, sc),
        }
    }

    /// A literal path at the current scope, with the reduce shortcuts.
    fn resolve_compiled(
        self: &Rc<Self>,
        sc: &Scope,
        segments: &[String],
        hint: ReduceHint,
        default: Option<&V>,
    ) -> Result<V> {
        let or_default = |found: Option<&V>| -> Result<V> {
            match (found, default) {
                (Some(v), _) => Ok(v.clone()),
                (None, Some(d)) => self.eval(d, sc),
                (None, None) => self.read(None, || segments.join(".")),
            }
        };
        if let Some(Frame::Reduce {
            current,
            accumulator,
        }) = sc.top()
        {
            match hint {
                ReduceHint::Current => return Ok(current.clone()),
                ReduceHint::Accumulator => return Ok(accumulator.clone()),
                ReduceHint::CurrentPath => return or_default(walk(current, &segments[1..])),
                ReduceHint::AccumulatorPath => {
                    return or_default(walk(accumulator, &segments[1..]));
                }
                ReduceHint::None => {}
            }
        }
        or_default(walk(sc.current(), segments))
    }

    fn metadata(&self, meta: Meta, level: u64, sc: &Scope) -> V {
        let frame = sc.metadata_frame(level);
        let found = match meta {
            Meta::Index => frame.and_then(Frame::index).map(int),
            Meta::Key => frame.and_then(Frame::key).map(|k| V::String(k.to_string())),
        };
        found.unwrap_or(V::Null)
    }

    /// `[[level], "index"]` read with the path known only at runtime.
    fn metadata_lookup(&self, sc: &Scope, level: i64, path: &str) -> Option<V> {
        let meta = meta_of(path)?;
        let level = level.unsigned_abs();
        metadata_climb(level)?;
        Some(self.metadata(meta, level, sc))
    }

    /// `val` with arguments known only at runtime.
    fn val_dynamic(self: &Rc<Self>, args: &[&V], sc: &Scope) -> Result<V> {
        if args.is_empty() {
            return Ok(sc.current().clone());
        }
        if args.len() >= 2 {
            return self.val_multiarg(args, sc);
        }
        let path = self.eval(args[0], sc)?;
        if path.is_null() {
            return Ok(sc.current().clone());
        }
        let found = if matches!(path, V::Array(_)) {
            self.lookup_array_path(&path, sc)?
        } else {
            self.lookup_scalar_path(&path, sc)
        };
        self.read(found, || value_path_name(&path))
    }

    fn val_multiarg(self: &Rc<Self>, args: &[&V], sc: &Scope) -> Result<V> {
        let first = self.eval(args[0], sc)?;
        if let Some(level) = level_marker_value(&first) {
            if args.len() == 2 {
                let path = self.eval(args[1], sc)?;
                if let Some(v) = self.metadata_lookup(sc, level, path.as_str().unwrap_or("")) {
                    return Ok(v);
                }
                let data = sc.data_at_level(level.unsigned_abs());
                let path = path_string(&path);
                return self.read(access_path(data, &path).cloned(), || path);
            }
            let mut paths = Vec::with_capacity(args.len() - 1);
            for arg in &args[1..] {
                paths.push(path_string(&self.eval(arg, sc)?));
            }
            let mut cur = sc.data_at_level(level.unsigned_abs());
            for path in &paths {
                match access_path(cur, path) {
                    Some(next) => cur = next,
                    None => return self.read(None, || paths.join(".")),
                }
            }
            return Ok(cur.clone());
        }
        let mut evaluated = vec![first];
        for arg in &args[1..] {
            evaluated.push(self.eval(arg, sc)?);
        }
        let start = match (sc.top(), evaluated[0].as_str()) {
            (Some(frame), Some("current")) => frame.reduce_current(),
            (Some(frame), Some("accumulator")) => frame.reduce_accumulator(),
            _ => None,
        };
        let (mut cur, rest) = match start {
            Some(slot) => (slot, 1),
            None => (sc.current(), 0),
        };
        for elem in &evaluated[rest..] {
            match apply_path_element(cur, elem) {
                Some(next) => cur = next,
                None => {
                    return self.read(None, || {
                        evaluated
                            .iter()
                            .map(value_path_name)
                            .collect::<Vec<_>>()
                            .join(".")
                    });
                }
            }
        }
        Ok(cur.clone())
    }

    /// An evaluated array path; `None` on a miss.
    fn lookup_array_path(&self, path: &V, sc: &Scope) -> Result<Option<V>> {
        let V::Array(items) = path else {
            unreachable!("caller checked")
        };
        if items.is_empty() {
            return Ok(Some(sc.current().clone()));
        }
        if items.len() >= 2
            && let Some(level) = level_marker_value(&items[0])
        {
            if items.len() == 2
                && let Some(v) = self.metadata_lookup(sc, level, items[1].as_str().unwrap_or(""))
            {
                return Ok(Some(v));
            }
            let mut cur = sc.data_at_level(level.unsigned_abs());
            for item in &items[1..] {
                let Some(seg) = item.as_str() else {
                    return Ok(None);
                };
                match access_path(cur, seg) {
                    Some(next) => cur = next,
                    None => return Ok(None),
                }
            }
            return Ok(Some(cur.clone()));
        }
        let mut cur = sc.current();
        for item in items {
            match apply_path_element(cur, item) {
                Some(next) => cur = next,
                None => return Ok(None),
            }
        }
        Ok(Some(cur.clone()))
    }

    /// An evaluated scalar path; `None` on a miss.
    fn lookup_scalar_path(&self, path: &V, sc: &Scope) -> Option<V> {
        if let Some(s) = path.as_str() {
            if let Some(frame) = sc.top() {
                if s == "current" {
                    if let Some(v) = frame.reduce_current() {
                        return Some(v.clone());
                    }
                } else if s == "accumulator" {
                    if let Some(v) = frame.reduce_accumulator() {
                        return Some(v.clone());
                    }
                } else if let Some(rest) = s.strip_prefix("current.") {
                    if let Some(cur) = frame.reduce_current() {
                        return access_path(cur, rest).cloned();
                    }
                } else if let Some(rest) = s.strip_prefix("accumulator.")
                    && let Some(acc) = frame.reduce_accumulator()
                {
                    return access_path(acc, rest).cloned();
                }
            }
            let cur = sc.current();
            // A key spelled with dots (or empty) wins over the dotted walk.
            if let V::Object(pairs) = cur
                && let Some(v) = object_get(pairs, s)
            {
                return Some(v.clone());
            }
            return access_path(cur, s).cloned();
        }
        let i = path.as_i64().filter(|i| *i >= 0)?;
        access_path(sc.current(), &i.to_string()).cloned()
    }

    /// `{"var": [path, default]}` with a path that is not a string or
    /// number literal.
    fn var_default(self: &Rc<Self>, path: &V, default: &V, sc: &Scope) -> Result<V> {
        let path = self.eval(path, sc)?;
        let found = if path.is_null() {
            Some(sc.current().clone())
        } else if matches!(path, V::Array(_)) {
            self.lookup_array_path(&path, sc)?
        } else {
            self.lookup_scalar_path(&path, sc)
        };
        match found {
            Some(v) => Ok(v),
            None => self.eval(default, sc),
        }
    }

    fn exists(self: &Rc<Self>, args: &[&V], sc: &Scope) -> Result<V> {
        if let Some(early) = arity(
            args.len(),
            1,
            None,
            OnMissing::Return(true_),
            OnExtra::Ignore,
        ) {
            return early;
        }
        let contains = |v: &V, key: &str| matches!(v, V::Object(p) if object_get(p, key).is_some());
        let first = self.eval(args[0], sc)?;
        let cur = sc.current();
        if args.len() == 1 {
            if let Some(s) = first.as_str() {
                return Ok(V::Bool(contains(cur, s)));
            }
            let V::Array(items) = &first else {
                return Ok(V::Bool(false));
            };
            if items.is_empty() {
                return Ok(V::Bool(false));
            }
            let mut walk_v = cur;
            for (i, item) in items.iter().enumerate() {
                let Some(seg) = item.as_str() else {
                    return Ok(V::Bool(false));
                };
                if i == items.len() - 1 {
                    return Ok(V::Bool(contains(walk_v, seg)));
                }
                match walk_v {
                    V::Object(p) => match object_get(p, seg) {
                        Some(next) => walk_v = next,
                        None => return Ok(V::Bool(false)),
                    },
                    _ => return Ok(V::Bool(false)),
                }
            }
            return Ok(V::Bool(true));
        }
        let Some(head) = first.as_str() else {
            return Ok(V::Bool(false));
        };
        let mut segments = vec![head.to_string()];
        for arg in &args[1..] {
            match self.eval(arg, sc)?.as_str() {
                Some(seg) => segments.push(seg.to_string()),
                None => return Ok(V::Bool(false)),
            }
        }
        let (last, init) = segments.split_last().expect("head");
        let mut walk_v = cur;
        for seg in init {
            match walk_v {
                V::Object(p) => match object_get(p, seg) {
                    Some(next) => walk_v = next,
                    None => return Ok(V::Bool(false)),
                },
                _ => return Ok(V::Bool(false)),
            }
        }
        Ok(V::Bool(contains(walk_v, last)))
    }

    fn missing(self: &Rc<Self>, args: &[&V], sc: &Scope) -> Result<V> {
        let mut out = Vec::new();
        for arg in args {
            let v = self.eval(arg, sc)?;
            let cur = sc.current();
            match &v {
                V::Array(items) => {
                    for item in items {
                        if let Some(path) = item.as_str()
                            && access_path(cur, path).is_none()
                        {
                            out.push(V::String(path.to_string()));
                        }
                    }
                }
                V::String(s) if access_path(cur, s).is_none() => out.push(V::String(s.clone())),
                _ => {}
            }
        }
        Ok(V::Array(out))
    }

    fn missing_some(self: &Rc<Self>, args: &[&V], sc: &Scope) -> Result<V> {
        if let Some(early) = arity(
            args.len(),
            2,
            Some(2),
            OnMissing::Return(empty_array),
            OnExtra::Ignore,
        ) {
            return early;
        }
        let min = self.eval(args[0], sc)?;
        let paths = self.eval(args[1], sc)?;
        // At least this many present, rounded up; a non-number needs 1.
        let min_present = match &min {
            V::Number(n) => match n.as_i64() {
                Some(i) => i.max(0) as usize,
                None => n.as_f64().ceil().max(0.0) as usize,
            },
            _ => 1,
        };
        let cur = sc.current();
        let mut missing = Vec::new();
        let mut present = 0usize;
        let mut short = false;
        if let V::Array(items) = &paths {
            for item in items {
                let Some(path) = item.as_str() else { continue };
                if access_path(cur, path).is_none() {
                    missing.push(V::String(path.to_string()));
                } else {
                    present += 1;
                    if present >= min_present {
                        short = true;
                        break;
                    }
                }
            }
        }
        if short || present >= min_present || missing.is_empty() {
            return Ok(empty_array());
        }
        Ok(V::Array(missing))
    }

    // ----- control ---------------------------------------------------------

    fn short_circuit(self: &Rc<Self>, args: &[&V], or: bool, sc: &Scope) -> Result<V> {
        if let Some(early) = arity(
            args.len(),
            1,
            None,
            OnMissing::Return(null),
            OnExtra::Ignore,
        ) {
            return early;
        }
        let (last, init) = args.split_last().expect("arity checked");
        for arg in init {
            let v = self.eval(arg, sc)?;
            if self.truthy(&v) == or {
                return Ok(v);
            }
        }
        self.eval(last, sc)
    }

    fn if_(self: &Rc<Self>, args: &[&V], sc: &Scope) -> Result<V> {
        if let Some(early) = arity(
            args.len(),
            1,
            None,
            OnMissing::Return(null),
            OnExtra::Ignore,
        ) {
            return early;
        }
        let mut i = 0;
        while i < args.len() {
            if i == args.len() - 1 {
                return self.eval(args[i], sc);
            }
            let cond = self.eval(args[i], sc)?;
            if self.truthy(&cond) {
                return self.eval(args[i + 1], sc);
            }
            i += 2;
        }
        Ok(V::Null)
    }

    fn switch(self: &Rc<Self>, args: &[&V], sc: &Scope) -> Result<V> {
        if let Some(early) = arity(
            args.len(),
            2,
            Some(3),
            OnMissing::Return(null),
            OnExtra::Ignore,
        ) {
            return early;
        }
        let disc = self.eval(args[0], sc)?;
        if let V::Array(cases) = args[1] {
            for case in cases {
                if let V::Array(pair) = case
                    && pair.len() >= 2
                {
                    let value = self.eval(&pair[0], sc)?;
                    if self.strict_eq(&disc, &value, sc)? {
                        return self.eval(&pair[1], sc);
                    }
                }
            }
        }
        match args.get(2) {
            Some(default) => self.eval(default, sc),
            None => Ok(V::Null),
        }
    }

    fn try_(self: &Rc<Self>, args: &[&V], sc: &Scope) -> Result<V> {
        if let Some(early) = arity(
            args.len(),
            1,
            None,
            OnMissing::Return(null),
            OnExtra::Ignore,
        ) {
            return early;
        }
        if args.len() == 1 {
            return self.eval(args[0], sc);
        }
        let (last, arms) = args.split_last().expect("two or more");
        let mut last_err = None;
        for arm in arms {
            match self.eval(arm, sc) {
                Ok(v) => return Ok(v),
                Err(e) if matches!(e.kind, ErrorKind::BudgetExceeded { .. }) => return Err(e),
                Err(e) => last_err = Some(e),
            }
        }
        let err = last_err.expect("every protected arm failed");
        let object = match &err.kind {
            ErrorKind::Thrown(v) => v.clone(),
            ErrorKind::InvalidOperator(_) => V::object([("type", "Unknown Operator")]),
            ErrorKind::InvalidArguments(m) => V::object([("type", m.to_string())]),
            kind => V::object([("type", Error::new(kind.clone()).to_string())]),
        };
        self.eval(last, &sc.push(Frame::Data(object)))
    }

    fn throw(self: &Rc<Self>, args: &[&V], sc: &Scope) -> Result<V> {
        let payload = match args.first() {
            Some(arg) => self.eval(arg, sc)?,
            None => V::Null,
        };
        let payload = match payload {
            V::Object(_) => payload,
            V::String(s) => V::object([("type", s)]),
            other => V::object([("type", type_name(&other))]),
        };
        Err(Error::thrown(payload))
    }

    // ----- iterators -------------------------------------------------------

    /// Resolve an iterator source: `None` for null or an empty array.
    fn source(self: &Rc<Self>, node: &V, sc: &Scope) -> Result<Option<Items>> {
        Ok(match self.eval(node, sc)? {
            V::Null => None,
            V::Array(items) if items.is_empty() => None,
            V::Array(items) => Some(Items::Array(items)),
            V::Object(pairs) => Some(Items::Object(pairs)),
            other => Some(Items::Scalar(other)),
        })
    }

    /// `filter`, `map`, `all`, `some`, `none`.
    fn each(self: &Rc<Self>, op: &str, args: &[&V], sc: &Scope) -> Result<V> {
        if let Some(early) = arity(
            args.len(),
            2,
            Some(2),
            OnMissing::InvalidArgs,
            OnExtra::InvalidArgs,
        ) {
            return early;
        }
        let body = args[1];
        let Some(items) = self.source(args[0], sc)? else {
            return Ok(match op {
                "filter" | "map" => empty_array(),
                "none" => V::Bool(true),
                _ => V::Bool(false),
            });
        };
        let run = |frame: Frame| self.eval(body, &sc.push(frame));
        match op {
            "map" => {
                let out = match items {
                    Items::Array(items) => items
                        .into_iter()
                        .enumerate()
                        .map(|(index, data)| run(Frame::Indexed { data, index }))
                        .collect::<Result<Vec<_>>>()?,
                    Items::Object(pairs) => pairs
                        .into_iter()
                        .enumerate()
                        .map(|(index, (key, data))| run(Frame::Keyed { data, index, key }))
                        .collect::<Result<Vec<_>>>()?,
                    Items::Scalar(data) => vec![run(Frame::Indexed { data, index: 0 })?],
                };
                Ok(V::Array(out))
            }
            "filter" => match items {
                Items::Array(items) => {
                    let mut kept = Vec::new();
                    for (index, data) in items.into_iter().enumerate() {
                        if self.truthy(&run(Frame::Indexed {
                            data: data.clone(),
                            index,
                        })?) {
                            kept.push(data);
                        }
                    }
                    Ok(V::Array(kept))
                }
                Items::Object(pairs) => {
                    let mut kept = Vec::new();
                    for (index, (key, data)) in pairs.into_iter().enumerate() {
                        if self.truthy(&run(Frame::Keyed {
                            data: data.clone(),
                            index,
                            key: key.clone(),
                        })?) {
                            kept.push((key, data));
                        }
                    }
                    Ok(V::Object(kept))
                }
                Items::Scalar(_) => Err(invalid_args()),
            },
            _ => {
                // `some` stops at the first truthy item, `all` and `none` at
                // the first falsy / truthy one.
                let stop_on = op != "all";
                let empty_result = op == "none";
                let frames: Vec<Frame> = match items {
                    Items::Array(items) => items
                        .into_iter()
                        .enumerate()
                        .map(|(index, data)| Frame::Indexed { data, index })
                        .collect(),
                    Items::Object(pairs) => pairs
                        .into_iter()
                        .enumerate()
                        .map(|(index, (key, data))| Frame::Keyed { data, index, key })
                        .collect(),
                    Items::Scalar(_) => return Ok(V::Bool(empty_result)),
                };
                if frames.is_empty() {
                    return Ok(V::Bool(empty_result));
                }
                let mut found = false;
                for frame in frames {
                    if self.truthy(&run(frame)?) == stop_on {
                        found = true;
                        break;
                    }
                }
                Ok(V::Bool(if op == "some" { found } else { !found }))
            }
        }
    }

    fn reduce(self: &Rc<Self>, args: &[&V], sc: &Scope) -> Result<V> {
        if let Some(early) = arity(
            args.len(),
            2,
            Some(3),
            OnMissing::InvalidArgs,
            OnExtra::InvalidArgs,
        ) {
            return early;
        }
        let initial = match args.get(2) {
            Some(init) => self.eval(init, sc)?,
            None => V::Null,
        };
        let items: Vec<V> = match self.eval(args[0], sc)? {
            V::Array(items) => items,
            V::Object(pairs) => pairs.into_iter().map(|(_, v)| v).collect(),
            _ => return Ok(initial),
        };
        let mut acc = initial;
        for current in items {
            acc = self.eval(
                args[1],
                &sc.push(Frame::Reduce {
                    current,
                    accumulator: acc,
                }),
            )?;
        }
        Ok(acc)
    }

    fn sort(self: &Rc<Self>, args: &[&V], sc: &Scope) -> Result<V> {
        if let Some(early) = arity(
            args.len(),
            1,
            Some(3),
            OnMissing::InvalidArgs,
            OnExtra::Ignore,
        ) {
            return early;
        }
        if args[0].is_null() {
            return Err(invalid_args());
        }
        let items = match self.eval(args[0], sc)? {
            V::Null => return Ok(V::Null),
            V::Array(items) => items,
            _ => return Err(invalid_args()),
        };
        if items.is_empty() {
            return Ok(empty_array());
        }
        let ascending = match args.get(1) {
            Some(dir) => match self.eval(dir, sc)? {
                V::Bool(b) => b,
                _ => true,
            },
            None => true,
        };
        let keys: Vec<V> = match args.get(2) {
            Some(key) => items
                .iter()
                .enumerate()
                .map(|(index, data)| {
                    self.eval(
                        key,
                        &sc.push(Frame::Indexed {
                            data: data.clone(),
                            index,
                        }),
                    )
                })
                .collect::<Result<_>>()?,
            None => items.clone(),
        };
        let mut order: Vec<usize> = (0..items.len()).collect();
        // Stable, so equal keys keep their input order either way round.
        // Numbers compare by exact value (zeros of either sign tie), which
        // is also what the engine's all-numbers fast path computes.
        order.sort_by(|&a, &b| {
            let cmp = compare_values(&keys[a], &keys[b]);
            if ascending { cmp } else { cmp.reverse() }
        });
        Ok(V::Array(
            order.into_iter().map(|i| items[i].clone()).collect(),
        ))
    }

    /// `group_by` and `distinct`.
    fn keyed(self: &Rc<Self>, op: &str, args: &[&V], sc: &Scope) -> Result<V> {
        let min = if op == "group_by" { 2 } else { 1 };
        if let Some(early) = arity(
            args.len(),
            min,
            Some(2),
            OnMissing::InvalidArgs,
            OnExtra::Ignore,
        ) {
            return early;
        }
        let items = match self.source(args[0], sc)? {
            None => return Ok(empty_array()),
            Some(Items::Array(items)) => items,
            Some(_) => return Err(invalid_args()),
        };
        let key_of = |index: usize, data: &V| -> Result<V> {
            match args.get(1) {
                Some(key) => self.eval(
                    key,
                    &sc.push(Frame::Indexed {
                        data: data.clone(),
                        index,
                    }),
                ),
                None => Ok(data.clone()),
            }
        };
        if op == "group_by" {
            let mut groups: Vec<(V, Vec<V>)> = Vec::new();
            for (index, item) in items.into_iter().enumerate() {
                let key = key_of(index, &item)?;
                let mut matched = false;
                for (k, members) in groups.iter_mut() {
                    if self.strict_eq(k, &key, sc)? {
                        members.push(item.clone());
                        matched = true;
                        break;
                    }
                }
                if !matched {
                    groups.push((key, vec![item]));
                }
            }
            return Ok(V::Array(
                groups
                    .into_iter()
                    .map(|(key, members)| {
                        V::Object(vec![
                            ("key".to_string(), key),
                            ("items".to_string(), V::Array(members)),
                        ])
                    })
                    .collect(),
            ));
        }
        let mut seen: Vec<V> = Vec::new();
        let mut kept = Vec::new();
        for (index, item) in items.into_iter().enumerate() {
            let key = key_of(index, &item)?;
            let mut dup = false;
            for prev in &seen {
                if self.strict_eq(prev, &key, sc)? {
                    dup = true;
                    break;
                }
            }
            if !dup {
                seen.push(key);
                kept.push(item);
            }
        }
        Ok(V::Array(kept))
    }
}
