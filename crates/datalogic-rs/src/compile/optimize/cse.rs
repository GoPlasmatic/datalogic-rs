//! Common-subexpression elimination (CSE).
//!
//! JSONLogic has no `let` bindings, so rule authors repeat pure aggregate
//! subexpressions verbatim (a checkout rule recomputes its subtotal
//! map+reduce everywhere the subtotal is referenced, making evaluation
//! O(items × ref-count)). This pass detects structurally identical pure
//! subtrees worth memoizing and wraps every occurrence of one equivalence
//! class in a [`CompiledNode::Cse`] carrying a shared memo-slot index. At
//! evaluation time `Engine::dispatch_cse` computes the first occurrence and
//! serves the rest from the per-evaluation slot table on `ContextStack`.
//!
//! # Soundness
//!
//! A memoized value may only be reused where re-evaluation would provably
//! produce it again. Two gates guarantee that:
//!
//! - **Purity (compile time):** only subtrees built entirely from pure
//!   builtin operators are candidates. `CustomOperator` (opaque, possibly
//!   re-entrant), `StructuredObject`, `Throw`/`Try` (error control flow),
//!   `Now` (time), and `Fractional`/`SemVer` (kept dynamic by policy, see
//!   `opcode_is_static`) disqualify a subtree. `Var`/`Missing`/`Exists`
//!   remain eligible — they read context, which the runtime gate pins.
//! - **Context (runtime):** the memo is consulted only at
//!   `ctx.depth() == 0`, where every context read resolves against the
//!   root (up-level `val`s clamp to root), so a pure subtree is a function
//!   of (root data, engine config) alone.
//!
//! # Compile-time cacheability prediction
//!
//! The runtime gate is authoritative for correctness, but the pass also
//! predicts at compile time which wrappers could actually pay off, so
//! rules never carry dead memo dispatch:
//!
//! - positions that evaluate under a pushed frame — iterator bodies and
//!   `try` catch arms — are neither wrapped nor counted toward the ≥ 2
//!   occurrence threshold ([`child_never_cacheable`]);
//! - classes whose occurrences are confined to mutually exclusive `if`
//!   value arms get no slot: only one arm runs per evaluation, so the
//!   memo would fill without ever being read ([`has_co_occurring_pair`]).
//!
//! Errors are never cached: the slot fills on `Ok` only, so re-evaluation
//! after a `try` caught the first occurrence's failure is deterministic.
//!
//! # Equivalence
//!
//! Occurrences are bucketed by a bottom-up structural hash and verified
//! with strict structural equality: `1` and `1.0` are distinct (they
//! render differently), floats compare by bit pattern, object literals
//! compare order-sensitively, and the derived fields (`id`,
//! `predicate_hint`, `iter_arg_kind`, `lit`) are skipped — structurally
//! identical subtrees always carry different ids.

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use datavalue::{NumberValue, OwnedDataValue};

use crate::node::{
    CompiledMissingArg, CompiledMissingMin, CompiledMissingPaths, CompiledNode, CseData,
    SYNTHETIC_ID,
};
use crate::opcode::OpCode;

/// Minimum subtree size (node count) for a candidate that contains no
/// iterator opcode. Small repeated scalar expressions are cheaper to
/// recompute than to route through a memo slot.
const MIN_NODE_COUNT: usize = 8;

/// Run the pass over a finished compile tree. Returns the number of memo
/// slots assigned (0 = no `Cse` nodes were inserted).
pub(crate) fn apply(root: &mut CompiledNode) -> u16 {
    let mut table = ClassTable::default();
    let mut choices = Vec::new();
    collect(root, &mut table, &mut choices);
    table.assign_slots();
    if table.slot_count == 0 {
        return 0;
    }
    wrap(root, &table);
    table.slot_count
}

// ---------------------------------------------------------------------------
// Phase 1 — hash, bucket, and verify equivalence classes
// ---------------------------------------------------------------------------

/// An occurrence's position within the rule's exclusive-choice structure:
/// one `(if-node identity, value-arm index)` entry per enclosing `if`
/// value arm. Two occurrences can run in the same evaluation iff no
/// shared choice node maps them to different arms.
type ChoicePath = Box<[(usize, u32)]>;

/// One verified equivalence class: an owned exemplar (cloned so phase 2 can
/// mutate the tree while matching against it) plus the choice path of
/// every wrappable occurrence seen.
struct Class {
    exemplar: CompiledNode,
    occurrences: Vec<ChoicePath>,
    slot: Option<u16>,
}

#[derive(Default)]
struct ClassTable {
    /// Hash → classes with that hash (usually one; collisions are resolved
    /// by exemplar verification).
    buckets: HashMap<u64, Vec<Class>>,
    /// `(hash, index-in-bucket)` in first-seen order, so slot numbering is
    /// deterministic regardless of `HashMap` iteration order.
    order: Vec<(u64, usize)>,
    slot_count: u16,
}

impl ClassTable {
    fn record(&mut self, node: &CompiledNode, choices: &[(usize, u32)]) {
        let hash = subtree_hash(node);
        let bucket = self.buckets.entry(hash).or_default();
        for class in bucket.iter_mut() {
            if structural_eq(&class.exemplar, node) {
                class.occurrences.push(choices.into());
                return;
            }
        }
        bucket.push(Class {
            exemplar: node.clone(),
            occurrences: vec![choices.into()],
            slot: None,
        });
        self.order.push((hash, bucket.len() - 1));
    }

    fn assign_slots(&mut self) {
        for (hash, index) in &self.order {
            if self.slot_count == u16::MAX {
                break;
            }
            let class = &mut self.buckets.get_mut(hash).expect("recorded bucket")[*index];
            // A slot pays off only if two occurrences can run in the same
            // evaluation — occurrences confined to mutually exclusive `if`
            // arms would fill the memo without ever re-reading it. When a
            // co-occurring pair exists, every occurrence gets wrapped
            // (exclusive ones included: whichever arm runs still hits a
            // memo filled by a co-occurring occurrence outside the `if`).
            if class.occurrences.len() >= 2 && has_co_occurring_pair(&class.occurrences) {
                class.slot = Some(self.slot_count);
                self.slot_count += 1;
            }
        }
    }

    /// Slot for `node` if it belongs to a shared class. Phase 2 only calls
    /// this on pristine (not-yet-wrapped) subtrees, so hashing matches
    /// phase 1 exactly.
    fn match_slot(&self, node: &CompiledNode) -> Option<u16> {
        if !matches!(node, CompiledNode::BuiltinOperator { .. }) {
            return None;
        }
        let bucket = self.buckets.get(&subtree_hash(node))?;
        bucket
            .iter()
            .find(|c| c.slot.is_some() && structural_eq(&c.exemplar, node))
            .and_then(|c| c.slot)
    }
}

/// Phase 1 walk: record every candidate subtree, skipping never-cacheable
/// argument positions (occurrences there are never wrapped, so they must
/// not count toward the ≥ 2 threshold either) and tracking the
/// exclusive-choice path so [`ClassTable::assign_slots`] can prune classes
/// whose occurrences never co-run.
fn collect(node: &CompiledNode, table: &mut ClassTable, choices: &mut Vec<(usize, u32)>) {
    if is_candidate(node) {
        table.record(node, choices);
    }
    match node {
        CompiledNode::BuiltinOperator { opcode, args, .. } => {
            let identity = node as *const CompiledNode as usize;
            for (i, child) in args.iter().enumerate() {
                if child_never_cacheable(*opcode, i, args.len()) {
                    continue;
                }
                let is_choice_arm = if_value_arm(*opcode, i, args.len());
                if is_choice_arm {
                    choices.push((identity, i as u32));
                }
                collect(child, table, choices);
                if is_choice_arm {
                    choices.pop();
                }
            }
        }
        _ => node.visit_indexed_children(&mut |_, child| collect(child, table, choices)),
    }
}

/// True when two occurrences with these choice paths can evaluate within
/// one evaluation: no shared `if` node routes them through different
/// value arms. (Conditions carry no arm entry — they co-occur with every
/// arm. `try` arms co-occur too: arm N runs after arm N-1 erred, and an
/// `Ok` memo filled before the error is legitimately reusable. `switch`
/// case results are conservatively treated as co-occurring.)
fn co_occurring(a: &[(usize, u32)], b: &[(usize, u32)]) -> bool {
    for (node_a, arm_a) in a {
        for (node_b, arm_b) in b {
            if node_a == node_b && arm_a != arm_b {
                return false;
            }
        }
    }
    true
}

fn has_co_occurring_pair(occurrences: &[ChoicePath]) -> bool {
    for (i, a) in occurrences.iter().enumerate() {
        for b in occurrences.iter().skip(i + 1) {
            if co_occurring(a, b) {
                return true;
            }
        }
    }
    false
}

/// Is `args[index]` of an `if` a *value* arm (as opposed to a condition)?
/// Layout: `[c1, v1, c2, v2, …, else?]` — values at odd indices plus the
/// trailing else at an even index when the arg count is odd; a single-arg
/// `if` returns its argument. Conditions evaluate on the way to whichever
/// arm is taken, so only value arms are mutually exclusive.
fn if_value_arm(opcode: OpCode, index: usize, len: usize) -> bool {
    if !matches!(opcode, OpCode::If) {
        return false;
    }
    len == 1 || index % 2 == 1 || (len % 2 == 1 && index == len - 1)
}

/// Candidate rule: a pure builtin operator that either contains an
/// iterator opcode (aggregates — the real-world target) or is at least
/// [`MIN_NODE_COUNT`] nodes.
fn is_candidate(node: &CompiledNode) -> bool {
    if !matches!(node, CompiledNode::BuiltinOperator { .. }) {
        return false;
    }
    if !is_cse_pure(node) {
        return false;
    }
    contains_iterator_op(node) || node_count(node) >= MIN_NODE_COUNT
}

// ---------------------------------------------------------------------------
// Phase 2 — wrap occurrences
// ---------------------------------------------------------------------------

fn wrap(node: &mut CompiledNode, table: &ClassTable) {
    if let Some(slot) = table.match_slot(node) {
        let placeholder = CompiledNode::InvalidArgs {
            id: SYNTHETIC_ID,
            op_name: "",
        };
        let inner = std::mem::replace(node, placeholder);
        *node = CompiledNode::Cse(Box::new(CseData { slot, inner }));
        // Keep descending so nested classes (total ⊃ net ⊃ subtotal) each
        // get their own slot inside the wrapped occurrence.
        if let CompiledNode::Cse(data) = node {
            wrap_children(&mut data.inner, table);
        }
        return;
    }
    wrap_children(node, table);
}

fn wrap_children(node: &mut CompiledNode, table: &ClassTable) {
    match node {
        CompiledNode::BuiltinOperator { opcode, args, .. } => {
            let len = args.len();
            for (i, child) in args.iter_mut().enumerate() {
                if !child_never_cacheable(*opcode, i, len) {
                    wrap(child, table);
                }
            }
        }
        _ => node.visit_children_mut(&mut |child| wrap(child, table)),
    }
}

/// Compile-time prediction of argument positions whose subtrees evaluate
/// at `depth() > 0` and could therefore never hit the runtime memo — a
/// wrapper there is pure dispatch overhead. Skipped by both phases (no
/// wrapping, and no occurrence counting toward the ≥ 2 threshold):
///
/// - iterator *bodies*: `args[1]` of filter/map/all/some/none/reduce and
///   `args[2]` of sort (whose `args[1]` is the scalar direction flag) run
///   under a per-item frame. `reduce`'s `args[2]` (initial accumulator)
///   evaluates once outside the iteration frames and stays eligible;
/// - the *catch arm* (last arg) of a multi-arg `try`: it runs under the
///   caught-error context frame when the error was thrown, and only ever
///   runs on the error path, so a memo wrapper there almost never pays.
///
/// The runtime `depth() == 0` gate remains authoritative — this predicate
/// is an overhead optimization, not a correctness gate.
fn child_never_cacheable(opcode: OpCode, index: usize, len: usize) -> bool {
    #[cfg(feature = "ext-array")]
    if matches!(opcode, OpCode::Sort) {
        return index == 2;
    }
    #[cfg(feature = "error-handling")]
    if matches!(opcode, OpCode::Try) {
        return len >= 2 && index == len - 1;
    }
    let _ = len;
    matches!(
        opcode,
        OpCode::Filter | OpCode::Map | OpCode::All | OpCode::Some | OpCode::None | OpCode::Reduce
    ) && index == 1
}

// ---------------------------------------------------------------------------
// Purity
// ---------------------------------------------------------------------------

/// True when evaluating this subtree at `depth() == 0` is a pure function
/// of (root data, engine config): deterministic, side-effect-free, and
/// safe to serve from a memo on repeat occurrences. Context readers
/// (`Var`, `Missing`, `Exists`) are pure *under the runtime depth gate* —
/// at depth 0 they resolve against the root.
fn is_cse_pure(node: &CompiledNode) -> bool {
    match node {
        CompiledNode::Value { .. } => true,
        CompiledNode::Array { nodes, .. } => nodes.iter().all(is_cse_pure),
        CompiledNode::BuiltinOperator { opcode, args, .. } => {
            opcode_is_cse_pure(*opcode) && args.iter().all(is_cse_pure)
        }
        // Opaque user code: may be non-deterministic, stateful, or
        // re-entrant. Never memoize.
        CompiledNode::CustomOperator(_) => false,
        CompiledNode::Cse(data) => is_cse_pure(&data.inner),
        // Conservative: templating output shape. Excluded per the Stage 1
        // design; can be relaxed to per-field purity later.
        #[cfg(feature = "templating")]
        CompiledNode::StructuredObject(_) => false,
        CompiledNode::Var { default_value, .. } => default_value.as_deref().is_none_or(is_cse_pure),
        #[cfg(feature = "ext-control")]
        CompiledNode::Exists(_) => true,
        #[cfg(feature = "error-handling")]
        CompiledNode::Throw(_) => false,
        CompiledNode::Missing(data) => data.args.iter().all(|arg| match arg {
            CompiledMissingArg::Now(_) => true,
            CompiledMissingArg::Later(n) => is_cse_pure(n),
        }),
        CompiledNode::MissingSome(data) => {
            let min_ok = match &data.min_present {
                CompiledMissingMin::Now(_) => true,
                CompiledMissingMin::Later(n) => is_cse_pure(n),
            };
            let paths_ok = match &data.paths {
                CompiledMissingPaths::Now(_) => true,
                CompiledMissingPaths::Later(n) => is_cse_pure(n),
            };
            min_ok && paths_ok
        }
        // Always errors — an Ok-only memo would never fill; wrapping is
        // pure overhead.
        CompiledNode::InvalidArgs { .. } => false,
    }
}

fn opcode_is_cse_pure(opcode: OpCode) -> bool {
    #[cfg(feature = "error-handling")]
    if matches!(opcode, OpCode::Try | OpCode::Throw) {
        return false;
    }
    #[cfg(feature = "datetime")]
    if matches!(opcode, OpCode::Now) {
        return false;
    }
    #[cfg(feature = "flagd")]
    if matches!(opcode, OpCode::Fractional | OpCode::SemVer) {
        return false;
    }
    true
}

// ---------------------------------------------------------------------------
// Subtree metrics
// ---------------------------------------------------------------------------

fn node_count(node: &CompiledNode) -> usize {
    let mut count = 1;
    node.visit_indexed_children(&mut |_, child| count += node_count(child));
    count
}

fn contains_iterator_op(node: &CompiledNode) -> bool {
    if let CompiledNode::BuiltinOperator { opcode, .. } = node {
        if is_iterator_opcode(*opcode) {
            return true;
        }
    }
    let mut found = false;
    node.visit_indexed_children(&mut |_, child| {
        if !found {
            found = contains_iterator_op(child);
        }
    });
    found
}

fn is_iterator_opcode(opcode: OpCode) -> bool {
    #[cfg(feature = "ext-array")]
    if matches!(opcode, OpCode::Sort) {
        return true;
    }
    matches!(
        opcode,
        OpCode::Filter | OpCode::Map | OpCode::All | OpCode::Some | OpCode::None | OpCode::Reduce
    )
}

// ---------------------------------------------------------------------------
// Structural hash + strict structural equality
// ---------------------------------------------------------------------------
//
// Invariant: `structural_eq(a, b)` ⇒ `subtree_hash(a) == subtree_hash(b)`.
// Both skip the derived fields (`id`, `predicate_hint`, `iter_arg_kind`,
// `lit`) and both treat floats by bit pattern, so the pair stays in sync.

fn subtree_hash(node: &CompiledNode) -> u64 {
    let mut hasher = DefaultHasher::new();
    hash_node(node, &mut hasher);
    hasher.finish()
}

fn hash_node<H: Hasher>(node: &CompiledNode, h: &mut H) {
    match node {
        CompiledNode::Value { value, .. } => {
            h.write_u8(0);
            hash_owned(value, h);
        }
        CompiledNode::Array { nodes, .. } => {
            h.write_u8(1);
            h.write_usize(nodes.len());
            for n in nodes.iter() {
                hash_node(n, h);
            }
        }
        CompiledNode::BuiltinOperator { opcode, args, .. } => {
            h.write_u8(2);
            h.write_u8(*opcode as u8);
            h.write_usize(args.len());
            for a in args.iter() {
                hash_node(a, h);
            }
        }
        CompiledNode::CustomOperator(data) => {
            h.write_u8(3);
            data.name.hash(h);
            h.write_usize(data.args.len());
            for a in data.args.iter() {
                hash_node(a, h);
            }
        }
        // Transparent: the pass only sees pristine trees, but stay
        // consistent if a wrapped subtree is ever hashed.
        CompiledNode::Cse(data) => hash_node(&data.inner, h),
        #[cfg(feature = "templating")]
        CompiledNode::StructuredObject(data) => {
            h.write_u8(4);
            h.write_usize(data.fields.len());
            for (key, n) in data.fields.iter() {
                key.hash(h);
                hash_node(n, h);
            }
        }
        CompiledNode::Var {
            scope_level,
            segments,
            reduce_hint,
            metadata_hint,
            default_value,
            ..
        } => {
            h.write_u8(5);
            h.write_u32(*scope_level);
            segments.hash(h);
            reduce_hint.hash(h);
            metadata_hint.hash(h);
            match default_value {
                Option::Some(d) => {
                    h.write_u8(1);
                    hash_node(d, h);
                }
                Option::None => h.write_u8(0),
            }
        }
        #[cfg(feature = "ext-control")]
        CompiledNode::Exists(data) => {
            h.write_u8(6);
            h.write_u32(data.scope_level);
            data.segments.hash(h);
        }
        #[cfg(feature = "error-handling")]
        CompiledNode::Throw(data) => {
            h.write_u8(7);
            hash_owned(&data.error, h);
        }
        CompiledNode::Missing(data) => {
            h.write_u8(8);
            h.write_usize(data.args.len());
            for arg in data.args.iter() {
                match arg {
                    CompiledMissingArg::Now((path, _)) => {
                        h.write_u8(0);
                        path.hash(h);
                    }
                    CompiledMissingArg::Later(n) => {
                        h.write_u8(1);
                        hash_node(n, h);
                    }
                }
            }
        }
        CompiledNode::MissingSome(data) => {
            h.write_u8(9);
            match &data.min_present {
                CompiledMissingMin::Now(n) => {
                    h.write_u8(0);
                    h.write_usize(*n);
                }
                CompiledMissingMin::Later(n) => {
                    h.write_u8(1);
                    hash_node(n, h);
                }
            }
            match &data.paths {
                CompiledMissingPaths::Now(paths) => {
                    h.write_u8(0);
                    h.write_usize(paths.len());
                    for (path, _) in paths.iter() {
                        path.hash(h);
                    }
                }
                CompiledMissingPaths::Later(n) => {
                    h.write_u8(1);
                    hash_node(n, h);
                }
            }
        }
        CompiledNode::InvalidArgs { op_name, .. } => {
            h.write_u8(10);
            op_name.hash(h);
        }
    }
}

/// Strict value hash: `Integer(1)` and `Float(1.0)` hash differently,
/// floats hash by bit pattern, objects hash in field order.
fn hash_owned<H: Hasher>(value: &OwnedDataValue, h: &mut H) {
    match value {
        OwnedDataValue::Null => h.write_u8(0),
        OwnedDataValue::Bool(b) => {
            h.write_u8(1);
            h.write_u8(u8::from(*b));
        }
        OwnedDataValue::Number(NumberValue::Integer(i)) => {
            h.write_u8(2);
            h.write_i64(*i);
        }
        OwnedDataValue::Number(NumberValue::Float(f)) => {
            h.write_u8(3);
            h.write_u64(f.to_bits());
        }
        OwnedDataValue::String(s) => {
            h.write_u8(4);
            s.hash(h);
        }
        OwnedDataValue::Array(items) => {
            h.write_u8(5);
            h.write_usize(items.len());
            for item in items {
                hash_owned(item, h);
            }
        }
        OwnedDataValue::Object(fields) => {
            h.write_u8(6);
            h.write_usize(fields.len());
            for (key, item) in fields {
                key.hash(h);
                hash_owned(item, h);
            }
        }
        #[cfg(feature = "datetime")]
        OwnedDataValue::DateTime(d) => {
            h.write_u8(7);
            format!("{d:?}").hash(h);
        }
        #[cfg(feature = "datetime")]
        OwnedDataValue::Duration(d) => {
            h.write_u8(8);
            format!("{d:?}").hash(h);
        }
    }
}

fn structural_eq(a: &CompiledNode, b: &CompiledNode) -> bool {
    match (a, b) {
        (CompiledNode::Value { value: va, .. }, CompiledNode::Value { value: vb, .. }) => {
            owned_eq(va, vb)
        }
        (CompiledNode::Array { nodes: na, .. }, CompiledNode::Array { nodes: nb, .. }) => {
            na.len() == nb.len() && na.iter().zip(nb.iter()).all(|(x, y)| structural_eq(x, y))
        }
        (
            CompiledNode::BuiltinOperator {
                opcode: oa,
                args: aa,
                ..
            },
            CompiledNode::BuiltinOperator {
                opcode: ob,
                args: ab,
                ..
            },
        ) => {
            oa == ob
                && aa.len() == ab.len()
                && aa.iter().zip(ab.iter()).all(|(x, y)| structural_eq(x, y))
        }
        (CompiledNode::CustomOperator(da), CompiledNode::CustomOperator(db)) => {
            da.name == db.name
                && da.args.len() == db.args.len()
                && da
                    .args
                    .iter()
                    .zip(db.args.iter())
                    .all(|(x, y)| structural_eq(x, y))
        }
        (CompiledNode::Cse(da), CompiledNode::Cse(db)) => {
            da.slot == db.slot && structural_eq(&da.inner, &db.inner)
        }
        #[cfg(feature = "templating")]
        (CompiledNode::StructuredObject(da), CompiledNode::StructuredObject(db)) => {
            da.fields.len() == db.fields.len()
                && da
                    .fields
                    .iter()
                    .zip(db.fields.iter())
                    .all(|((ka, na), (kb, nb))| ka == kb && structural_eq(na, nb))
        }
        (
            CompiledNode::Var {
                scope_level: sa,
                segments: ga,
                reduce_hint: ra,
                metadata_hint: ma,
                default_value: da,
                ..
            },
            CompiledNode::Var {
                scope_level: sb,
                segments: gb,
                reduce_hint: rb,
                metadata_hint: mb,
                default_value: db,
                ..
            },
        ) => {
            sa == sb
                && ga == gb
                && ra == rb
                && ma == mb
                && match (da, db) {
                    (Option::None, Option::None) => true,
                    (Option::Some(x), Option::Some(y)) => structural_eq(x, y),
                    _ => false,
                }
        }
        #[cfg(feature = "ext-control")]
        (CompiledNode::Exists(da), CompiledNode::Exists(db)) => {
            da.scope_level == db.scope_level && da.segments == db.segments
        }
        #[cfg(feature = "error-handling")]
        (CompiledNode::Throw(da), CompiledNode::Throw(db)) => owned_eq(&da.error, &db.error),
        (CompiledNode::Missing(da), CompiledNode::Missing(db)) => {
            da.args.len() == db.args.len()
                && da
                    .args
                    .iter()
                    .zip(db.args.iter())
                    .all(|(x, y)| missing_arg_eq(x, y))
        }
        (CompiledNode::MissingSome(da), CompiledNode::MissingSome(db)) => {
            let min_eq = match (&da.min_present, &db.min_present) {
                (CompiledMissingMin::Now(x), CompiledMissingMin::Now(y)) => x == y,
                (CompiledMissingMin::Later(x), CompiledMissingMin::Later(y)) => structural_eq(x, y),
                _ => false,
            };
            let paths_eq = match (&da.paths, &db.paths) {
                (CompiledMissingPaths::Now(x), CompiledMissingPaths::Now(y)) => {
                    x.len() == y.len() && x.iter().zip(y.iter()).all(|((pa, _), (pb, _))| pa == pb)
                }
                (CompiledMissingPaths::Later(x), CompiledMissingPaths::Later(y)) => {
                    structural_eq(x, y)
                }
                _ => false,
            };
            min_eq && paths_eq
        }
        (
            CompiledNode::InvalidArgs { op_name: na, .. },
            CompiledNode::InvalidArgs { op_name: nb, .. },
        ) => na == nb,
        _ => false,
    }
}

fn missing_arg_eq(a: &CompiledMissingArg, b: &CompiledMissingArg) -> bool {
    match (a, b) {
        (CompiledMissingArg::Now((pa, _)), CompiledMissingArg::Now((pb, _))) => pa == pb,
        (CompiledMissingArg::Later(x), CompiledMissingArg::Later(y)) => structural_eq(x, y),
        _ => false,
    }
}

/// Strict value equality, mirroring [`hash_owned`]: `Integer(1)` ≠
/// `Float(1.0)`, floats compare by bit pattern, objects compare in field
/// order.
fn owned_eq(a: &OwnedDataValue, b: &OwnedDataValue) -> bool {
    match (a, b) {
        (OwnedDataValue::Null, OwnedDataValue::Null) => true,
        (OwnedDataValue::Bool(x), OwnedDataValue::Bool(y)) => x == y,
        (
            OwnedDataValue::Number(NumberValue::Integer(x)),
            OwnedDataValue::Number(NumberValue::Integer(y)),
        ) => x == y,
        (
            OwnedDataValue::Number(NumberValue::Float(x)),
            OwnedDataValue::Number(NumberValue::Float(y)),
        ) => x.to_bits() == y.to_bits(),
        (OwnedDataValue::String(x), OwnedDataValue::String(y)) => x == y,
        (OwnedDataValue::Array(x), OwnedDataValue::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y.iter()).all(|(a, b)| owned_eq(a, b))
        }
        (OwnedDataValue::Object(x), OwnedDataValue::Object(y)) => {
            x.len() == y.len()
                && x.iter()
                    .zip(y.iter())
                    .all(|((ka, va), (kb, vb))| ka == kb && owned_eq(va, vb))
        }
        #[cfg(feature = "datetime")]
        (OwnedDataValue::DateTime(x), OwnedDataValue::DateTime(y)) => {
            format!("{x:?}") == format!("{y:?}")
        }
        #[cfg(feature = "datetime")]
        (OwnedDataValue::Duration(x), OwnedDataValue::Duration(y)) => {
            format!("{x:?}") == format!("{y:?}")
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use crate::Engine;

    /// A pure aggregate over `items` — the canonical CSE candidate.
    const AGG: &str =
        r#"{"reduce": [{"var": "items"}, {"+": [{"var": "accumulator"}, {"var": "current"}]}, 0]}"#;

    fn slot_count(rule: &str) -> u16 {
        Engine::new().compile(rule).unwrap().cse_slot_count
    }

    #[test]
    fn repeated_aggregate_shares_one_slot() {
        let rule = format!(r#"{{"+": [{AGG}, {AGG}]}}"#);
        assert_eq!(slot_count(&rule), 1);
    }

    #[test]
    fn single_occurrence_gets_no_slot() {
        assert_eq!(slot_count(AGG), 0);
    }

    #[test]
    fn small_scalar_repeats_get_no_slot() {
        // Pure and repeated, but no iterator and < MIN_NODE_COUNT nodes.
        let rule = r#"{"+": [{"*": [{"var": "x"}, 2]}, {"*": [{"var": "x"}, 2]}]}"#;
        assert_eq!(slot_count(rule), 0);
    }

    #[test]
    fn integer_and_float_near_twins_do_not_share() {
        // Initial accumulator 0 vs 0.0: strict equality must keep the two
        // classes apart (they render — and can evaluate — differently).
        let agg_float = AGG.replace(", 0]", ", 0.0]");
        let rule = format!(r#"{{"+": [{AGG}, {AGG}, {agg_float}, {agg_float}]}}"#);
        assert_eq!(slot_count(&rule), 2);
    }

    #[test]
    fn iterator_body_occurrences_neither_count_nor_wrap() {
        // One occurrence at root, one inside a map body: the body one is
        // ineligible, so no class reaches the ≥ 2 threshold.
        let rule = format!(
            r#"{{"+": [{AGG}, {{"reduce": [{{"map": [{{"var": "xs"}}, {AGG}]}}, {{"+": [{{"var": "accumulator"}}, {{"var": "current"}}]}}, 0]}}]}}"#
        );
        assert_eq!(slot_count(&rule), 0);
    }

    #[cfg(feature = "error-handling")]
    #[test]
    fn try_catch_arm_occurrences_neither_count_nor_wrap() {
        // The catch arm runs under the caught-error frame — a memo
        // wrapper there could never hit. One protected + one catch
        // occurrence must not form a class.
        let rule = format!(r#"{{"try": [{AGG}, {{"+": [{AGG}, 1]}}]}}"#);
        assert_eq!(slot_count(&rule), 0);
    }

    #[cfg(feature = "error-handling")]
    #[test]
    fn try_protected_arms_share() {
        // Occurrences inside a protected arm evaluate at depth 0 and
        // stay eligible.
        let rule = format!(r#"{{"try": [{{"+": [{AGG}, {AGG}]}}, 0]}}"#);
        assert_eq!(slot_count(&rule), 1);
    }

    #[test]
    fn mutually_exclusive_if_arms_get_no_slot() {
        // Only one value arm runs per evaluation — a shared slot would
        // fill without ever being read.
        let rule = format!(r#"{{"if": [{{"var": "c"}}, {AGG}, {AGG}]}}"#);
        assert_eq!(slot_count(&rule), 0);
    }

    #[test]
    fn if_condition_co_occurs_with_taken_arm() {
        // The condition evaluates on the way to the arm, so these two
        // occurrences can run in one evaluation and share.
        let rule = format!(r#"{{"if": [{{">": [{AGG}, 10]}}, {AGG}, 0]}}"#);
        assert_eq!(slot_count(&rule), 1);
    }

    #[test]
    fn exclusive_arms_still_share_with_a_root_occurrence() {
        // A co-occurring pair (root + either arm) keeps the class; the
        // exclusive arm occurrences are wrapped too and hit the memo the
        // root occurrence filled.
        let rule = format!(r#"{{"+": [{AGG}, {{"if": [{{"var": "c"}}, {AGG}, {AGG}]}}]}}"#);
        assert_eq!(slot_count(&rule), 1);
    }

    #[test]
    fn no_fold_compile_produces_no_slots() {
        let engine = Engine::builder().with_constant_folding(false).build();
        let rule = format!(r#"{{"+": [{AGG}, {AGG}]}}"#);
        assert_eq!(engine.compile(rule.as_str()).unwrap().cse_slot_count, 0);
    }

    #[test]
    fn custom_truthy_evaluator_disables_the_pass() {
        let config = crate::EvaluationConfig::default().with_truthy_evaluator(
            crate::TruthyEvaluator::Custom(std::sync::Arc::new(|_| true)),
        );
        let engine = Engine::builder().with_config(config).build();
        let rule = format!(r#"{{"+": [{AGG}, {AGG}]}}"#);
        assert_eq!(engine.compile(rule.as_str()).unwrap().cse_slot_count, 0);
    }

    #[test]
    fn impure_subtrees_are_not_candidates() {
        // A custom-operator name inside the subtree disqualifies it even
        // though the surrounding shape repeats.
        let agg = r#"{"reduce": [{"var": "items"}, {"my_op": [{"var": "current"}]}, 0]}"#;
        let rule = format!(r#"{{"+": [{agg}, {agg}]}}"#);
        assert_eq!(slot_count(&rule), 0);
    }

    #[test]
    fn nested_classes_get_their_own_slots() {
        // AGG (3 occurrences) and {"*":[AGG,2]} (2 occurrences) both share.
        let rule = format!(r#"{{"+": [{AGG}, {{"*": [{AGG}, 2]}}, {{"*": [{AGG}, 2]}}]}}"#);
        assert_eq!(slot_count(&rule), 2);
    }

    #[test]
    fn serialization_is_wrapper_transparent() {
        let engine = Engine::new();
        let rule = format!(r#"{{"+": [{AGG}, {AGG}]}}"#);
        let with_cse = engine.compile(rule.as_str()).unwrap();
        assert_eq!(with_cse.cse_slot_count, 1);
        let no_cse_engine = Engine::builder().with_constant_folding(false).build();
        let without_cse = no_cse_engine.compile(rule.as_str()).unwrap();
        assert_eq!(with_cse.to_json(), without_cse.to_json());
    }

    #[test]
    fn memo_does_not_leak_across_evaluations() {
        let engine = Engine::new();
        let rule = format!(r#"{{"+": [{AGG}, {AGG}]}}"#);
        let compiled = engine.compile(rule.as_str()).unwrap();
        assert_eq!(compiled.cse_slot_count, 1);
        let mut session = engine.session();
        assert_eq!(
            session
                .eval_str(&compiled, r#"{"items": [1, 2, 3]}"#)
                .unwrap(),
            "12"
        );
        session.reset();
        assert_eq!(
            session.eval_str(&compiled, r#"{"items": [10]}"#).unwrap(),
            "20"
        );
    }

    #[cfg(feature = "trace")]
    #[test]
    fn traced_eval_of_cse_compiled_logic_matches_plain_eval() {
        // A Logic compiled WITH Cse nodes can be run under a tracer
        // (`TracedSession::eval` does not recompile) — the runtime
        // `is_tracing` gate must bypass the memo and produce the same
        // value with full per-occurrence trace coverage.
        let engine = Engine::new();
        let rule = format!(r#"{{"+": [{AGG}, {AGG}]}}"#);
        let compiled = engine.compile(rule.as_str()).unwrap();
        assert_eq!(compiled.cse_slot_count, 1);
        let plain = engine.eval_str(rule.as_str(), r#"{"items": [1, 2, 3]}"#);
        let traced = engine.trace().eval(&compiled, r#"{"items": [1, 2, 3]}"#);
        assert_eq!(
            plain.unwrap(),
            traced.result.unwrap().to_json_string(),
            "traced and plain evaluation of a CSE'd tree must agree"
        );
    }

    #[test]
    fn object_literals_compare_order_sensitively() {
        use datavalue::OwnedDataValue as V;
        let ab = V::Object(vec![
            ("a".into(), V::Bool(true)),
            ("b".into(), V::Bool(false)),
        ]);
        let ba = V::Object(vec![
            ("b".into(), V::Bool(false)),
            ("a".into(), V::Bool(true)),
        ]);
        assert!(!super::owned_eq(&ab, &ba));
        assert!(super::owned_eq(&ab, &ab.clone()));
    }

    #[test]
    fn integer_and_float_values_are_distinct() {
        use datavalue::{NumberValue, OwnedDataValue as V};
        let int_one = V::Number(NumberValue::Integer(1));
        let float_one = V::Number(NumberValue::Float(1.0));
        assert!(!super::owned_eq(&int_one, &float_one));
    }
}
