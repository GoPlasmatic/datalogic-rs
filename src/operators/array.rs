use serde_json::Value;

#[cfg(feature = "ext-array")]
use std::cmp::Ordering;

use crate::arena::{ArenaContextStack, ArenaValue, value_to_arena};
use crate::node::{MetadataHint, ReduceHint};
use crate::opcode::OpCode;
use crate::{CompiledNode, DataLogic, Error, Result};
use bumpalo::Bump;

/// Check if a compiled node is loop-invariant (doesn't depend on the current iteration context).
/// Used by filter/quantifier fast paths to detect values that can be evaluated once before the loop.
#[inline]
fn is_filter_invariant(node: &CompiledNode) -> bool {
    match node {
        CompiledNode::Value { .. } => true,
        CompiledNode::CompiledVar { scope_level, .. } => *scope_level > 0,
        _ => false,
    }
}

/// Try to extract filter fast-path components from a comparison pair.
/// Returns (field_segments, invariant_node) if `a` is a simple scope_level=0 field var
/// and `b` is loop-invariant (literal value or parent scope var).
#[inline]
fn try_extract_filter_field_cmp<'a>(
    a: &'a CompiledNode,
    b: &'a CompiledNode,
) -> Option<(&'a [crate::node::PathSegment], &'a CompiledNode)> {
    if let CompiledNode::CompiledVar {
        scope_level: 0,
        segments,
        reduce_hint: ReduceHint::None,
        metadata_hint: MetadataHint::None,
        default_value: None,
        ..
    } = a
        && !segments.is_empty()
        && is_filter_invariant(b)
    {
        return Some((segments, b));
    }
    None
}

/// Evaluate a loop-invariant predicate-side node once, before iteration.
/// Literal values are deep-converted into the arena; outer-scope
/// `CompiledVar`s resolve through arena dispatch with a synthesized null iter
/// frame so the var sees the outer context unaffected by the missing iter
/// frame this fast path skips.
#[inline]
fn evaluate_invariant_no_push<'a>(
    invariant_node: &'a CompiledNode,
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if let CompiledNode::Value { value, .. } = invariant_node {
        return Ok(arena.alloc(value_to_arena(value, arena)));
    }
    let null_av: &'a ArenaValue<'a> = arena.alloc(ArenaValue::Null);
    actx.push(null_av);
    let result = engine.evaluate_arena_node(invariant_node, actx, arena);
    actx.pop();
    result
}

/// Represents a detected fast-path predicate pattern for quantifier/filter operators.
/// Avoids per-item context push/pop and dispatch overhead.
/// When `segments` is `Some`, compares a field extracted via path traversal;
/// when `None`, compares the whole item directly.
enum FastPredicate<'a> {
    /// Strict equality (===) or inequality (!==) against a literal
    StrictEq {
        segments: Option<&'a [crate::node::PathSegment]>,
        literal: &'a Value,
        negate: bool,
    },
    /// Ordered numeric comparison (>, >=, <, <=) against a numeric literal
    NumericCmp {
        segments: Option<&'a [crate::node::PathSegment]>,
        literal_f: f64,
        opcode: OpCode,
        var_is_lhs: bool,
    },
    /// Loose numeric equality (==) or inequality (!=) against a numeric literal
    LooseNumericEq {
        segments: Option<&'a [crate::node::PathSegment]>,
        literal_f: f64,
        negate: bool,
    },
}

impl<'a> FastPredicate<'a> {
    /// Try to detect a fast predicate pattern from a compiled predicate node.
    fn try_detect(predicate: &'a CompiledNode) -> Option<Self> {
        if let CompiledNode::BuiltinOperator {
            opcode,
            args: pred_args,
            ..
        } = predicate
            && pred_args.len() == 2
        {
            // Try both orderings: (var, literal) and (literal, var)
            for (var_idx, lit_idx, var_is_lhs) in [(0, 1, true), (1, 0, false)] {
                if let CompiledNode::CompiledVar {
                    scope_level: 0,
                    segments,
                    reduce_hint: ReduceHint::None,
                    metadata_hint: MetadataHint::None,
                    default_value: None,
                    ..
                } = &pred_args[var_idx]
                    && let CompiledNode::Value { value: literal, .. } = &pred_args[lit_idx]
                {
                    let segs = if segments.is_empty() {
                        None
                    } else {
                        Some(&**segments)
                    };

                    match opcode {
                        OpCode::StrictEquals | OpCode::StrictNotEquals => {
                            let negate = matches!(opcode, OpCode::StrictNotEquals);
                            return Some(FastPredicate::StrictEq {
                                segments: segs,
                                literal,
                                negate,
                            });
                        }
                        OpCode::Equals | OpCode::NotEquals => {
                            // For loose equality with numeric literals, we can use a fast
                            // numeric comparison (loose == is same as strict for numbers)
                            if let Some(lit_f) = literal.as_f64() {
                                let negate = matches!(opcode, OpCode::NotEquals);
                                return Some(FastPredicate::LooseNumericEq {
                                    segments: segs,
                                    literal_f: lit_f,
                                    negate,
                                });
                            }
                        }
                        OpCode::GreaterThan
                        | OpCode::GreaterThanEqual
                        | OpCode::LessThan
                        | OpCode::LessThanEqual => {
                            if let Some(lit_f) = literal.as_f64() {
                                return Some(FastPredicate::NumericCmp {
                                    segments: segs,
                                    literal_f: lit_f,
                                    opcode: *opcode,
                                    var_is_lhs,
                                });
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        None
    }

    /// Resolve the value to compare: either the whole item or a field within it.
    #[inline]
    fn resolve_value<'b>(
        segments: Option<&[crate::node::PathSegment]>,
        item: &'b ArenaValue<'b>,
        arena: &'b Bump,
    ) -> Option<&'b ArenaValue<'b>> {
        match segments {
            None => Some(item),
            Some(segs) => crate::arena::value::arena_traverse_segments(item, segs, arena),
        }
    }

    /// Evaluate this predicate against a single item.
    #[inline]
    fn evaluate<'b>(&self, item: &'b ArenaValue<'b>, arena: &'b Bump) -> bool {
        match self {
            FastPredicate::StrictEq {
                segments,
                literal,
                negate,
            } => {
                let matches = match Self::resolve_value(*segments, item, arena) {
                    Some(av) => arena_value_equals_value(av, literal),
                    None => false,
                };
                if *negate { !matches } else { matches }
            }
            FastPredicate::NumericCmp {
                segments,
                literal_f,
                opcode,
                var_is_lhs,
            } => {
                if let Some(val) = Self::resolve_value(*segments, item, arena)
                    && let Some(val_f) = val.as_f64()
                {
                    let (lhs, rhs) = if *var_is_lhs {
                        (val_f, *literal_f)
                    } else {
                        (*literal_f, val_f)
                    };
                    inline_numeric_cmp(lhs, rhs, *opcode)
                } else {
                    false
                }
            }
            FastPredicate::LooseNumericEq {
                segments,
                literal_f,
                negate,
            } => {
                let matches = if let Some(val) = Self::resolve_value(*segments, item, arena)
                    && let Some(val_f) = val.as_f64()
                {
                    val_f == *literal_f
                } else {
                    false
                };
                if *negate { !matches } else { matches }
            }
        }
    }
}

/// Strict equality between two `ArenaValue`s.
#[inline]
fn arena_value_equals_arena(a: &ArenaValue<'_>, b: &ArenaValue<'_>) -> bool {
    match (a, b) {
        (ArenaValue::Null, ArenaValue::Null) => true,
        (ArenaValue::Bool(x), ArenaValue::Bool(y)) => x == y,
        (ArenaValue::Number(x), ArenaValue::Number(y)) => match (x.as_i64(), y.as_i64()) {
            (Some(a), Some(b)) => a == b,
            _ => x.as_f64() == y.as_f64(),
        },
        (ArenaValue::String(x), ArenaValue::String(y)) => *x == *y,
        (ArenaValue::Array(x), ArenaValue::Array(y)) => {
            x.len() == y.len()
                && x.iter()
                    .zip(y.iter())
                    .all(|(a, b)| arena_value_equals_arena(a, b))
        }
        (ArenaValue::Object(x), ArenaValue::Object(y)) => {
            if x.len() != y.len() {
                return false;
            }
            for (k, v) in *x {
                let found = y.iter().find(|(yk, _)| *yk == *k).map(|(_, yv)| yv);
                match found {
                    Some(yv) => {
                        if !arena_value_equals_arena(v, yv) {
                            return false;
                        }
                    }
                    None => return false,
                }
            }
            true
        }
        _ => false,
    }
}

/// Strict equality between an `ArenaValue` and a `serde_json::Value` literal —
/// used by `FastPredicate::StrictEq` to compare an arena-resident item against
/// a compile-time literal without allocating.
#[inline]
fn arena_value_equals_value(av: &ArenaValue<'_>, v: &Value) -> bool {
    match (av, v) {
        (ArenaValue::Null, Value::Null) => true,
        (ArenaValue::Bool(a), Value::Bool(b)) => a == b,
        (ArenaValue::Number(a), Value::Number(b)) => match (a.as_i64(), b.as_i64()) {
            (Some(x), Some(y)) => x == y,
            _ => match (a.as_f64(), b.as_f64()) {
                (x, Some(y)) => x == y,
                _ => false,
            },
        },
        (ArenaValue::String(s), Value::String(b)) => *s == b.as_str(),
        (ArenaValue::Array(items), Value::Array(b)) => {
            items.len() == b.len()
                && items
                    .iter()
                    .zip(b.iter())
                    .all(|(x, y)| arena_value_equals_value(x, y))
        }
        (ArenaValue::Object(pairs), Value::Object(b)) => {
            if pairs.len() != b.len() {
                return false;
            }
            for (k, av) in *pairs {
                match b.get(*k) {
                    Some(bv) => {
                        if !arena_value_equals_value(av, bv) {
                            return false;
                        }
                    }
                    None => return false,
                }
            }
            true
        }
        _ => false,
    }
}

/// Inline numeric comparison for fast predicate evaluation.
#[inline(always)]
fn inline_numeric_cmp(lhs: f64, rhs: f64, opcode: OpCode) -> bool {
    match opcode {
        OpCode::GreaterThan => lhs > rhs,
        OpCode::GreaterThanEqual => lhs >= rhs,
        OpCode::LessThan => lhs < rhs,
        OpCode::LessThanEqual => lhs <= rhs,
        _ => false,
    }
}

#[cfg(feature = "ext-array")]
// Helper function to compare arena values for sorting.
// Type order: null < bool < number < string < array < object.
fn compare_values(a: &ArenaValue<'_>, b: &ArenaValue<'_>) -> Ordering {
    #[inline]
    fn type_rank(v: &ArenaValue<'_>) -> u8 {
        match v {
            ArenaValue::Null => 0,
            ArenaValue::Bool(_) => 1,
            ArenaValue::Number(_) => 2,
            ArenaValue::String(_) => 3,
            ArenaValue::Array(_) => 4,
            ArenaValue::Object(_) => 5,
            #[cfg(feature = "datetime")]
            ArenaValue::DateTime(_) | ArenaValue::Duration(_) => 3,
        }
    }

    match (a, b) {
        (ArenaValue::Null, ArenaValue::Null) => Ordering::Equal,
        (ArenaValue::Bool(a), ArenaValue::Bool(b)) => a.cmp(b),
        (ArenaValue::Number(a), ArenaValue::Number(b)) => {
            let a_f = a.as_f64();
            let b_f = b.as_f64();
            if a_f < b_f {
                Ordering::Less
            } else if a_f > b_f {
                Ordering::Greater
            } else {
                Ordering::Equal
            }
        }
        (ArenaValue::String(a), ArenaValue::String(b)) => a.cmp(b),
        (ArenaValue::Array(_), ArenaValue::Array(_)) => Ordering::Equal,
        (ArenaValue::Object(_), ArenaValue::Object(_)) => Ordering::Equal,
        _ => type_rank(a).cmp(&type_rank(b)),
    }
}

#[cfg(feature = "ext-array")]
// Helper function to slice characters directly without Value conversion
fn slice_chars(
    chars: &[char],
    len: i64,
    start: Option<i64>,
    end: Option<i64>,
    step: i64,
) -> String {
    let mut result = String::new();

    let (actual_start, actual_end) = if step > 0 {
        let s = normalize_index(start.unwrap_or(0), len);
        let e = normalize_index(end.unwrap_or(len), len);
        (s, e)
    } else {
        let default_start = len.saturating_sub(1);
        let s = normalize_index(start.unwrap_or(default_start), len);
        let e = if let Some(e) = end {
            normalize_index(e, len)
        } else {
            -1
        };
        (s, e)
    };

    if step > 0 {
        let mut i = actual_start;
        while i < actual_end && i < len {
            if i >= 0 && (i as usize) < chars.len() {
                result.push(chars[i as usize]);
            }
            i = i.saturating_add(step);
            if step > 0 && i < actual_start {
                break;
            }
        }
    } else {
        let mut i = actual_start;
        while i > actual_end && i >= 0 && i < len {
            if (i as usize) < chars.len() {
                result.push(chars[i as usize]);
            }
            let next_i = i.saturating_add(step);
            if step < 0 && next_i > i {
                break;
            }
            i = next_i;
        }
    }

    result
}

/// Native arena-mode `slice`. Returns array slices as views over arena items;
/// string slices are allocated in the arena.
#[cfg(feature = "ext-array")]
#[inline]
pub(crate) fn evaluate_slice_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a crate::arena::ArenaValue<'a>> {
    if args.is_empty() {
        return Err(crate::constants::invalid_args());
    }

    let coll_av = engine.evaluate_arena_node(&args[0], actx, arena)?;

    // Null passthrough.
    if matches!(coll_av, ArenaValue::Null) {
        return Ok(crate::arena::pool::singleton_null());
    }

    // Resolve start/end/step.
    let start = if args.len() > 1 {
        extract_opt_i64_arena(&args[1], actx, engine, arena)?
    } else {
        None
    };
    let end = if args.len() > 2 {
        extract_opt_i64_arena(&args[2], actx, engine, arena)?
    } else {
        None
    };
    let step = if args.len() > 3 {
        let s = extract_opt_i64_arena(&args[3], actx, engine, arena)?.unwrap_or(1);
        if s == 0 {
            return Err(crate::constants::invalid_args());
        }
        s
    } else {
        1
    };

    // Composite arena array — slice through the arena items.
    if let ArenaValue::Array(items) = coll_av {
        let len = items.len() as i64;
        let indices = slice_indices(len, start, end, step);
        let mut out: bumpalo::collections::Vec<'a, ArenaValue<'a>> =
            bumpalo::collections::Vec::with_capacity_in(indices.len(), arena);
        for i in indices {
            out.push(crate::arena::value::reborrow_arena_value(
                &items[i as usize],
            ));
        }
        return Ok(arena.alloc(ArenaValue::Array(out.into_bump_slice())));
    }

    // String slice — allocate result in the arena.
    if let ArenaValue::String(s) = coll_av {
        let chars: Vec<char> = s.chars().collect();
        let result_string = slice_chars(&chars, chars.len() as i64, start, end, step);
        let s_arena: &'a str = arena.alloc_str(&result_string);
        return Ok(arena.alloc(ArenaValue::String(s_arena)));
    }

    Err(crate::constants::invalid_args())
}

#[cfg(feature = "ext-array")]
#[inline]
fn extract_opt_i64_arena<'a>(
    node: &'a CompiledNode,
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<Option<i64>> {
    if let CompiledNode::Value { value, .. } = node {
        return match value {
            Value::Number(n) => Ok(n.as_i64()),
            Value::Null => Ok(None),
            _ => Err(Error::InvalidArguments("NaN".to_string())),
        };
    }
    let av = engine.evaluate_arena_node(node, actx, arena)?;
    match av {
        ArenaValue::Null => Ok(None),
        _ => match av.as_i64() {
            Some(i) => Ok(Some(i)),
            None => Err(Error::InvalidArguments("NaN".to_string())),
        },
    }
}

/// Index list for a slice given start/end/step. Computes the index sequence
/// without materializing values.
#[cfg(feature = "ext-array")]
#[inline]
fn slice_indices(len: i64, start: Option<i64>, end: Option<i64>, step: i64) -> Vec<i64> {
    let mut out = Vec::new();
    let (actual_start, actual_end) = if step > 0 {
        (
            normalize_index(start.unwrap_or(0), len),
            normalize_index(end.unwrap_or(len), len),
        )
    } else {
        let default_start = len.saturating_sub(1);
        let s = normalize_index(start.unwrap_or(default_start), len);
        let e = if let Some(e) = end {
            if e < -len {
                -1
            } else {
                normalize_index(e, len)
            }
        } else {
            -1
        };
        (s, e)
    };

    let mut i = actual_start;
    while (step > 0 && i < actual_end) || (step < 0 && i > actual_end) {
        if i >= 0 && i < len {
            out.push(i);
        }
        i += step;
    }
    out
}

#[cfg(feature = "ext-array")]
// Helper function to normalize slice indices with overflow protection
fn normalize_index(index: i64, len: i64) -> i64 {
    if index < 0 {
        // Use saturating_add to prevent overflow when index is very negative
        let adjusted = len.saturating_add(index);
        adjusted.max(0)
    } else {
        index.min(len)
    }
}

// =============================================================================
// Iterator operators: filter / map / quantifiers + composition IN.
//
// All inputs are arena-resident. `IterSrc` is a thin wrapper over
// `&[ArenaValue]` so iterator-op bodies have a stable interface even if we
// later add multi-source iteration shapes.
// =============================================================================

/// Unified view over an iterator op's input collection. Single shape:
/// arena slice of `ArenaValue`. Wrapper kept for API stability.
#[derive(Clone, Copy)]
pub(crate) struct IterSrc<'a>(pub(crate) &'a [ArenaValue<'a>]);

impl<'a> IterSrc<'a> {
    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }

    #[inline]
    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Get item by index.
    #[inline]
    pub(crate) fn get(&self, i: usize) -> &'a ArenaValue<'a> {
        &self.0[i]
    }
}

/// Outcome of resolving an iterator op's first arg in arena mode.
pub(crate) enum ResolvedInput<'a> {
    /// Iterable input — proceed with array iteration.
    Iterable(IterSrc<'a>),
    /// Empty/null input — caller returns its empty-collection result.
    Empty,
    /// Object or other non-array input. Carries the resolved arena value
    /// so callers can dispatch natively (object-iteration / error / etc.)
    /// without re-evaluating the arg.
    Bridge(&'a ArenaValue<'a>),
}

/// Resolve `args[0]` for an iterator op. Tries (in order):
///   1. Borrow from root data (cheapest — no eval, no alloc)
///   2. Dispatch to arena (when arg is e.g. another filter — composition path)
///   3. Bridge: caller handles non-borrowable inputs (objects, primitives)
///
/// The returned `IterSrc` borrows from the arena (`'a`) and is safe to iterate
/// while the caller mutates `context` for predicate evaluation, because the
/// underlying data lives in either the input `Arc` (held for the call's
/// duration) or arena slices (allocated on the same arena).
pub(crate) fn resolve_iter_input<'a>(
    arg: &'a CompiledNode,
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<ResolvedInput<'a>> {
    // Path 1: root borrow — args[0] is a simple var that resolves into the
    // input root data.
    if let Some(av) = try_borrow_collection_from_root(arg, actx, arena) {
        return Ok(arena_value_as_iter(av));
    }

    // Path 2: composition — arg is another arena-aware op. Dispatch and inspect.
    if let CompiledNode::BuiltinOperator { opcode, .. } = arg
        && matches!(
            opcode,
            OpCode::Filter
                | OpCode::Map
                | OpCode::All
                | OpCode::Some
                | OpCode::None
                | OpCode::Reduce
                | OpCode::Merge
        )
    {
        let av = engine.evaluate_arena_node(arg, actx, arena)?;
        return Ok(arena_value_as_iter(av));
    }

    // Path 3: anything else — evaluate through arena dispatch so the caller
    // can handle the result natively (Object iteration / single-element wrap /
    // error per op semantics).
    let av = engine.evaluate_arena_node(arg, actx, arena)?;
    Ok(arena_value_as_iter(av))
}

/// Convert a resolved arena value into an `IterSrc` view, or signal Empty/Bridge.
fn arena_value_as_iter<'a>(av: &'a ArenaValue<'a>) -> ResolvedInput<'a> {
    match av {
        ArenaValue::Null => ResolvedInput::Empty,
        ArenaValue::Array(items) => ResolvedInput::Iterable(IterSrc(items)),
        _ => ResolvedInput::Bridge(av),
    }
}

/// `filter`. Fast path: input collection resolves at root scope (the dominant
/// pattern in real workloads). Bridge path handles non-borrowable inputs.
#[inline]
pub(crate) fn evaluate_filter_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if args.len() != 2 {
        return Err(crate::constants::invalid_args());
    }

    // Resolve input via unified helper (root borrow OR upstream arena op).
    let src = match resolve_iter_input(&args[0], actx, engine, arena)? {
        ResolvedInput::Iterable(s) => s,
        ResolvedInput::Empty => return Ok(arena.alloc(ArenaValue::Array(&[]))),
        ResolvedInput::Bridge(av) => {
            return filter_arena_bridge(av, &args[1], actx, engine, arena);
        }
    };

    let predicate = &args[1];
    let len = src.len();
    if len == 0 {
        return Ok(arena.alloc(ArenaValue::Array(&[])));
    }

    // FAST PATH: predicates evaluable by direct field traversal — no context
    // push, no item clone. The arena win materializes here: zero per-item
    // allocations, only the result slice in the arena.
    if let CompiledNode::BuiltinOperator {
        opcode,
        args: pred_args,
        ..
    } = predicate
        && pred_args.len() == 2
        && matches!(opcode, OpCode::StrictEquals | OpCode::StrictNotEquals)
    {
        let fast = try_extract_filter_field_cmp(&pred_args[0], &pred_args[1])
            .or_else(|| try_extract_filter_field_cmp(&pred_args[1], &pred_args[0]));
        if let Some((segments, invariant_node)) = fast {
            let invariant_val = evaluate_invariant_no_push(invariant_node, actx, engine, arena)?;
            let is_eq = matches!(opcode, OpCode::StrictEquals);
            let mut results: bumpalo::collections::Vec<'a, ArenaValue<'a>> =
                bumpalo::collections::Vec::with_capacity_in(len, arena);
            for i in 0..len {
                let item = src.get(i);
                let matches =
                    match crate::arena::value::arena_traverse_segments(item, segments, arena) {
                        Some(av) => arena_value_equals_arena(av, invariant_val),
                        None => false,
                    };
                if matches == is_eq {
                    results.push(crate::arena::value::reborrow_arena_value(item));
                }
            }
            return Ok(arena.alloc(ArenaValue::Array(results.into_bump_slice())));
        }
    }

    if let Some(fast_pred) = FastPredicate::try_detect(predicate) {
        let mut results: bumpalo::collections::Vec<'a, ArenaValue<'a>> =
            bumpalo::collections::Vec::with_capacity_in(len, arena);
        for i in 0..len {
            let item = src.get(i);
            if fast_pred.evaluate(item, arena) {
                results.push(crate::arena::value::reborrow_arena_value(item));
            }
        }
        return Ok(arena.alloc(ArenaValue::Array(results.into_bump_slice())));
    }

    // GENERAL PATH: zero-clone via ArenaContextStack. Frame data is
    // `&'a ArenaValue<'a>` pointing at the arena-resident item; predicate
    // body dispatches through arena and the var-arena reads the frame
    // directly.
    let mut results: bumpalo::collections::Vec<'a, ArenaValue<'a>> =
        bumpalo::collections::Vec::with_capacity_in(len, arena);
    let mut pushed = false;
    for i in 0..len {
        let item = src.get(i);
        if !pushed {
            actx.push_with_index(item, 0);
            pushed = true;
        } else {
            actx.replace_top_data(item, i);
        }
        let keep = engine.eval_iter_body(predicate, actx, arena, i as u32, len as u32)?;
        if crate::arena::is_truthy_arena(keep, engine) {
            results.push(crate::arena::value::reborrow_arena_value(item));
        }
    }
    if pushed {
        actx.pop();
    }
    Ok(arena.alloc(ArenaValue::Array(results.into_bump_slice())))
}

/// Filter Bridge case — input is an Object, an inline arena Array (e.g. a
/// literal `[1,2,3]` arg) or a non-array primitive. Object inputs iterate
/// `(key, value)` pairs into a new arena `Object`; arena Array inputs iterate
/// items into a new arena `Array`; other shapes are an error.
#[inline]
fn filter_arena_bridge<'a>(
    input: &'a ArenaValue<'a>,
    predicate: &'a CompiledNode,
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if let ArenaValue::Object(pairs) = input {
        let mut kept: bumpalo::collections::Vec<'a, (&'a str, ArenaValue<'a>)> =
            bumpalo::collections::Vec::with_capacity_in(pairs.len(), arena);
        let mut pushed = false;
        let total = pairs.len() as u32;
        for (i, (k, v)) in pairs.iter().enumerate() {
            let item_av: &'a ArenaValue<'a> = unsafe { &*(v as *const ArenaValue<'a>) };
            let key_arena: &'a str = k;
            if !pushed {
                actx.push_with_key_index(item_av, 0, key_arena);
                pushed = true;
            } else {
                actx.replace_top_key_data(item_av, i, key_arena);
            }
            let keep = engine.eval_iter_body(predicate, actx, arena, i as u32, total)?;
            if crate::arena::is_truthy_arena(keep, engine) {
                kept.push((
                    key_arena,
                    crate::arena::value::reborrow_arena_value(item_av),
                ));
            }
        }
        if pushed {
            actx.pop();
        }
        return Ok(arena.alloc(ArenaValue::Object(kept.into_bump_slice())));
    }
    if let ArenaValue::Array(items) = input {
        let mut kept: bumpalo::collections::Vec<'a, ArenaValue<'a>> =
            bumpalo::collections::Vec::with_capacity_in(items.len(), arena);
        let mut pushed = false;
        let total = items.len() as u32;
        for i in 0..items.len() {
            let item_av: &'a ArenaValue<'a> = &items[i];
            if !pushed {
                actx.push_with_index(item_av, 0);
                pushed = true;
            } else {
                actx.replace_top_data(item_av, i);
            }
            let keep = engine.eval_iter_body(predicate, actx, arena, i as u32, total)?;
            if crate::arena::is_truthy_arena(keep, engine) {
                kept.push(crate::arena::value::reborrow_arena_value(item_av));
            }
        }
        if pushed {
            actx.pop();
        }
        return Ok(arena.alloc(ArenaValue::Array(kept.into_bump_slice())));
    }
    Err(crate::constants::invalid_args())
}

/// Map Bridge case — Object inputs iterate (key, value) pairs; inline arena
/// Array inputs (e.g. literal `[1,2,3]` arg) iterate items; other shapes are
/// treated as a single-element collection.
#[inline]
fn map_arena_bridge<'a>(
    input: &'a ArenaValue<'a>,
    body: &'a CompiledNode,
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if let ArenaValue::Object(pairs) = input {
        let mut results: bumpalo::collections::Vec<'a, ArenaValue<'a>> =
            bumpalo::collections::Vec::with_capacity_in(pairs.len(), arena);
        let mut pushed = false;
        let total = pairs.len() as u32;
        for (i, (k, v)) in pairs.iter().enumerate() {
            let item_av: &'a ArenaValue<'a> = unsafe { &*(v as *const ArenaValue<'a>) };
            let key_arena: &'a str = k;
            if !pushed {
                actx.push_with_key_index(item_av, 0, key_arena);
                pushed = true;
            } else {
                actx.replace_top_key_data(item_av, i, key_arena);
            }
            let av = engine.eval_iter_body(body, actx, arena, i as u32, total)?;
            results.push(crate::arena::value::reborrow_arena_value(av));
        }
        if pushed {
            actx.pop();
        }
        return Ok(arena.alloc(ArenaValue::Array(results.into_bump_slice())));
    }
    if let ArenaValue::Array(items) = input {
        let mut results: bumpalo::collections::Vec<'a, ArenaValue<'a>> =
            bumpalo::collections::Vec::with_capacity_in(items.len(), arena);
        let mut pushed = false;
        let total = items.len() as u32;
        for i in 0..items.len() {
            let item_av: &'a ArenaValue<'a> = &items[i];
            if !pushed {
                actx.push_with_index(item_av, 0);
                pushed = true;
            } else {
                actx.replace_top_data(item_av, i);
            }
            let av = engine.eval_iter_body(body, actx, arena, i as u32, total)?;
            results.push(crate::arena::value::reborrow_arena_value(av));
        }
        if pushed {
            actx.pop();
        }
        return Ok(arena.alloc(ArenaValue::Array(results.into_bump_slice())));
    }
    // Single-element collection (number, string, bool primitive input).
    let item_av: &'a ArenaValue<'a> = input;
    actx.push_with_index(item_av, 0);
    let av = engine.eval_iter_body(body, actx, arena, 0, 1)?;
    let owned = crate::arena::value::reborrow_arena_value(av);
    actx.pop();
    let slice = arena.alloc_slice_fill_iter(std::iter::once(owned));
    Ok(arena.alloc(ArenaValue::Array(slice)))
}

/// Reduce Bridge case — Object inputs iterate (key, value) pairs; inline
/// arena Array inputs iterate items. Non-array non-object non-null inputs
/// return the initial value.
#[inline]
fn reduce_arena_bridge<'a>(
    input: &'a ArenaValue<'a>,
    body: &'a CompiledNode,
    initial: &'a ArenaValue<'a>,
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if let ArenaValue::Object(pairs) = input {
        let mut acc_av: &'a ArenaValue<'a> = initial;
        let mut pushed = false;
        let total = pairs.len() as u32;
        for (i, (_k, v)) in pairs.iter().enumerate() {
            let item_av: &'a ArenaValue<'a> = unsafe { &*(v as *const ArenaValue<'a>) };
            // reduce frame stores current/accumulator only — key isn't exposed here.
            if !pushed {
                actx.push_reduce(item_av, acc_av);
                pushed = true;
            } else {
                actx.replace_reduce_data(item_av, acc_av);
            }
            acc_av = engine.eval_iter_body(body, actx, arena, i as u32, total)?;
        }
        if pushed {
            actx.pop();
        }
        return Ok(acc_av);
    }
    if let ArenaValue::Array(items) = input {
        let mut acc_av: &'a ArenaValue<'a> = initial;
        let mut pushed = false;
        let total = items.len() as u32;
        for i in 0..items.len() {
            let item_av: &'a ArenaValue<'a> = &items[i];
            if !pushed {
                actx.push_reduce(item_av, acc_av);
                pushed = true;
            } else {
                actx.replace_reduce_data(item_av, acc_av);
            }
            acc_av = engine.eval_iter_body(body, actx, arena, i as u32, total)?;
        }
        if pushed {
            actx.pop();
        }
        return Ok(acc_av);
    }
    // Anything else — return initial.
    Ok(initial)
}

/// Shape of a quantifier (`all` / `some` / `none`) — the three flags
/// distinguishing them are bundled here so callers and helpers don't carry
/// three loose `bool` parameters.
#[derive(Clone, Copy)]
struct QuantifierShape {
    /// Predicate result that triggers early exit.
    short_circuit_on: bool,
    /// If `true`, invert `short_circuit_on` when assembling the final result.
    invert_final: bool,
    /// Result for an empty input collection.
    empty_result: bool,
}

impl QuantifierShape {
    #[inline]
    fn finalize(self, found_short: bool) -> bool {
        if found_short {
            if self.invert_final {
                !self.short_circuit_on
            } else {
                self.short_circuit_on
            }
        } else if self.invert_final {
            self.short_circuit_on
        } else {
            !self.short_circuit_on
        }
    }
}

/// Quantifier Bridge case — Object inputs iterate (key, value) pairs.
#[inline]
fn quantifier_arena_bridge<'a>(
    input: &'a ArenaValue<'a>,
    predicate: &'a CompiledNode,
    shape: QuantifierShape,
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if let ArenaValue::Object(pairs) = input {
        if pairs.is_empty() {
            return Ok(arena.alloc(ArenaValue::Bool(shape.empty_result)));
        }
        let mut pushed = false;
        let mut found_short = false;
        let total = pairs.len() as u32;
        for (i, (k, v)) in pairs.iter().enumerate() {
            let item_av: &'a ArenaValue<'a> = unsafe { &*(v as *const ArenaValue<'a>) };
            let key_arena: &'a str = k;
            if !pushed {
                actx.push_with_key_index(item_av, 0, key_arena);
                pushed = true;
            } else {
                actx.replace_top_key_data(item_av, i, key_arena);
            }
            let av = engine.eval_iter_body(predicate, actx, arena, i as u32, total)?;
            if crate::arena::is_truthy_arena(av, engine) == shape.short_circuit_on {
                found_short = true;
                break;
            }
        }
        if pushed {
            actx.pop();
        }
        return Ok(arena.alloc(ArenaValue::Bool(shape.finalize(found_short))));
    }
    if let ArenaValue::Array(items) = input {
        if items.is_empty() {
            return Ok(arena.alloc(ArenaValue::Bool(shape.empty_result)));
        }
        let mut pushed = false;
        let mut found_short = false;
        let total = items.len() as u32;
        for i in 0..items.len() {
            let item_av: &'a ArenaValue<'a> = &items[i];
            if !pushed {
                actx.push_with_index(item_av, 0);
                pushed = true;
            } else {
                actx.replace_top_data(item_av, i);
            }
            let av = engine.eval_iter_body(predicate, actx, arena, i as u32, total)?;
            if crate::arena::is_truthy_arena(av, engine) == shape.short_circuit_on {
                found_short = true;
                break;
            }
        }
        if pushed {
            actx.pop();
        }
        return Ok(arena.alloc(ArenaValue::Bool(shape.finalize(found_short))));
    }
    // Anything else — treated as empty (returns empty_result).
    Ok(arena.alloc(ArenaValue::Bool(shape.empty_result)))
}

/// `sort`. Borrows input via `IterSrc` (no input clone), runs
/// `slice::sort_by` over indices, and emits `ArenaValue::Array` re-borrowing
/// the original arena items in their sorted order — avoids a deep-clone of
/// the input array, which dominates for object arrays.
///
/// Fast path (extractor is a root-scope `var`): keys come from
/// `arena_traverse_segments` returning `&ArenaValue` directly, no key clones.
#[cfg(feature = "ext-array")]
#[inline]
pub(crate) fn evaluate_sort_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if args.is_empty() {
        return Err(crate::constants::invalid_args());
    }

    // Literal-null first arg is an error.
    if let CompiledNode::Value { value, .. } = &args[0]
        && value.is_null()
    {
        return Err(crate::constants::invalid_args());
    }

    let src = match resolve_iter_input(&args[0], actx, engine, arena)? {
        ResolvedInput::Iterable(s) => s,
        ResolvedInput::Empty => return Ok(arena.alloc(ArenaValue::Null)),
        ResolvedInput::Bridge(av) => {
            return sort_arena_from_value(av, args, actx, engine, arena);
        }
    };

    let len = src.len();
    if len == 0 {
        return Ok(arena.alloc(ArenaValue::Array(&[])));
    }

    // Sort direction: defaults to ascending; non-Bool means ascending too.
    let ascending = if args.len() > 1 {
        let dir = engine.evaluate_arena_node(&args[1], actx, arena)?;
        match dir {
            ArenaValue::Bool(b) => *b,
            _ => true,
        }
    } else {
        true
    };

    let has_extractor = args.len() > 2;

    if !has_extractor {
        // No extractor — sort items directly by ArenaValue order.
        let mut indices: Vec<usize> = (0..len).collect();
        indices.sort_by(|&a, &b| {
            let cmp = compare_values(src.get(a), src.get(b));
            if ascending { cmp } else { cmp.reverse() }
        });
        let slice = arena.alloc_slice_fill_iter(
            indices
                .into_iter()
                .map(|i| crate::arena::value::reborrow_arena_value(src.get(i))),
        );
        return Ok(arena.alloc(ArenaValue::Array(slice)));
    }

    // Extractor present — fast path for root-scope `var` segments.
    let extractor = &args[2];
    if let CompiledNode::CompiledVar {
        scope_level: 0,
        segments,
        reduce_hint: ReduceHint::None,
        metadata_hint: MetadataHint::None,
        default_value: None,
        ..
    } = extractor
        && !segments.is_empty()
    {
        let mut keyed: Vec<(usize, Option<&ArenaValue<'a>>)> = (0..len)
            .map(|i| {
                (
                    i,
                    crate::arena::value::arena_traverse_segments(src.get(i), segments, arena),
                )
            })
            .collect();
        keyed.sort_by(|(_, ka), (_, kb)| {
            let cmp = match (ka, kb) {
                (Some(a), Some(b)) => compare_values(a, b),
                (Some(_), None) => Ordering::Greater,
                (None, Some(_)) => Ordering::Less,
                (None, None) => Ordering::Equal,
            };
            if ascending { cmp } else { cmp.reverse() }
        });
        let slice = arena.alloc_slice_fill_iter(
            keyed
                .into_iter()
                .map(|(i, _)| crate::arena::value::reborrow_arena_value(src.get(i))),
        );
        return Ok(arena.alloc(ArenaValue::Array(slice)));
    }

    // General extractor — push each item into the arena context, evaluate
    // the extractor, collect keys, then sort indices by key. Result re-borrows
    // arena items into the sorted output.
    let mut keys: Vec<ArenaValue<'a>> = Vec::with_capacity(len);
    let mut pushed = false;
    for i in 0..len {
        let item = src.get(i);
        if !pushed {
            actx.push_with_index(item, 0);
            pushed = true;
        } else {
            actx.replace_top_data(item, i);
        }
        let key_av = engine.evaluate_arena_node(extractor, actx, arena)?;
        keys.push(crate::arena::value::reborrow_arena_value(key_av));
    }
    if pushed {
        actx.pop();
    }

    let mut indices: Vec<usize> = (0..len).collect();
    indices.sort_by(|&a, &b| {
        let cmp = compare_values(&keys[a], &keys[b]);
        if ascending { cmp } else { cmp.reverse() }
    });
    let slice = arena.alloc_slice_fill_iter(
        indices
            .into_iter()
            .map(|i| crate::arena::value::reborrow_arena_value(src.get(i))),
    );
    Ok(arena.alloc(ArenaValue::Array(slice)))
}

/// Sort a resolved arena value when the input wasn't borrowable as a
/// flat `&[Value]` — falls into one of: Null (→ Null), Array (→ sort),
/// anything else (→ error). Re-uses the same direction/extractor logic
/// as the borrowed path.
#[cfg(feature = "ext-array")]
#[inline]
fn sort_arena_from_value<'a>(
    av: &'a ArenaValue<'a>,
    args: &'a [CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    let arena_items_slice: &'a [ArenaValue<'a>] = match av {
        ArenaValue::Null => {
            return Ok(crate::arena::pool::singleton_null());
        }
        ArenaValue::Array(items) => *items,
        _ => return Err(crate::constants::invalid_args()),
    };
    if arena_items_slice.is_empty() {
        return Ok(arena.alloc(ArenaValue::Array(&[])));
    }

    let ascending = if args.len() > 1 {
        let dir = engine.evaluate_arena_node(&args[1], actx, arena)?;
        match dir {
            ArenaValue::Bool(b) => *b,
            _ => true,
        }
    } else {
        true
    };

    if args.len() <= 2 {
        let n = arena_items_slice.len();
        let mut indices: Vec<usize> = (0..n).collect();
        indices.sort_by(|&a, &b| {
            let cmp = compare_values(&arena_items_slice[a], &arena_items_slice[b]);
            if ascending { cmp } else { cmp.reverse() }
        });
        let items = arena.alloc_slice_fill_iter(
            indices
                .into_iter()
                .map(|i| crate::arena::value::reborrow_arena_value(&arena_items_slice[i])),
        );
        return Ok(arena.alloc(ArenaValue::Array(items)));
    }

    // Extractor present — push items into arena context, evaluate,
    // collect keys, sort indices.
    let extractor = &args[2];
    let n = arena_items_slice.len();

    let mut keys: Vec<ArenaValue<'a>> = Vec::with_capacity(n);
    let mut pushed = false;
    for (i, item_av) in arena_items_slice.iter().enumerate() {
        if !pushed {
            actx.push_with_index(item_av, 0);
            pushed = true;
        } else {
            actx.replace_top_data(item_av, i);
        }
        let key_av = engine.evaluate_arena_node(extractor, actx, arena)?;
        keys.push(crate::arena::value::reborrow_arena_value(key_av));
    }
    if pushed {
        actx.pop();
    }

    let mut indices: Vec<usize> = (0..n).collect();
    indices.sort_by(|&a, &b| {
        let cmp = compare_values(&keys[a], &keys[b]);
        if ascending { cmp } else { cmp.reverse() }
    });

    let out = arena.alloc_slice_fill_iter(
        indices
            .into_iter()
            .map(|i| crate::arena::value::reborrow_arena_value(&arena_items_slice[i])),
    );
    Ok(arena.alloc(ArenaValue::Array(out)))
}

/// Arena-mode `length`. Critical for the COMPOSITION test: when called as
/// `length(filter(...))`, the filter result lives in the arena and length
/// just reads the slice length — zero conversion cost on the intermediate.
#[cfg(feature = "ext-string")]
#[inline]
pub(crate) fn evaluate_length_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if args.len() != 1 {
        return Err(crate::constants::invalid_args());
    }

    // Recurse into arena dispatcher so composed cases (e.g. length(filter(...)))
    // stay arena-resident on the intermediate.
    let arg = engine.evaluate_arena_node(&args[0], actx, arena)?;

    let n: i64 = match arg {
        ArenaValue::String(s) => s.chars().count() as i64,
        ArenaValue::Array(items) => items.len() as i64,
        _ => return Err(crate::constants::invalid_args()),
    };

    Ok(arena.alloc(ArenaValue::Number(crate::value::NumberValue::from_i64(n))))
}

/// Try to obtain the input collection by borrowing from the caller's root data.
/// Returns `Some(&ArenaValue)` when args[0] is a simple root-scope `var` that
/// resolves into the input data. The returned reference lives for the arena
/// lifetime `'a`.
#[inline]
fn try_borrow_collection_from_root<'a>(
    arg: &'a CompiledNode,
    actx: &ArenaContextStack<'a>,
    arena: &'a Bump,
) -> Option<&'a ArenaValue<'a>> {
    if actx.depth() != 0 {
        return None; // only root-scope borrows
    }
    if let CompiledNode::CompiledVar {
        scope_level: 0,
        segments,
        reduce_hint: ReduceHint::None,
        metadata_hint: MetadataHint::None,
        default_value: None,
        ..
    } = arg
    {
        let root = actx.root_input();
        if segments.is_empty() {
            return Some(root);
        }
        return crate::arena::value::arena_traverse_segments(root, segments, arena);
    }
    None
}

/// `map`. Borrows input from root scope when possible. Body fast path for
/// var/field-extract re-borrows the arena item per output entry with zero
/// iteration allocs. Other body shapes evaluate the body via arena dispatch
/// per item.
#[inline]
pub(crate) fn evaluate_map_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if args.len() != 2 {
        return Err(crate::constants::invalid_args());
    }

    let body = &args[1];
    let src = match resolve_iter_input(&args[0], actx, engine, arena)? {
        ResolvedInput::Iterable(s) => s,
        ResolvedInput::Empty => return Ok(arena.alloc(ArenaValue::Array(&[]))),
        ResolvedInput::Bridge(av) => {
            return map_arena_bridge(av, &args[1], actx, engine, arena);
        }
    };

    let len = src.len();
    if len == 0 {
        return Ok(arena.alloc(ArenaValue::Array(&[])));
    }

    // BODY FAST PATH: var with simple shape — identity (empty segments) or
    // field extract. Both re-borrow arena items with zero per-iteration allocs.
    if let CompiledNode::CompiledVar {
        scope_level: 0,
        segments,
        reduce_hint: ReduceHint::None,
        metadata_hint: MetadataHint::None,
        default_value: None,
        ..
    } = body
    {
        let mut results: bumpalo::collections::Vec<'a, ArenaValue<'a>> =
            bumpalo::collections::Vec::with_capacity_in(len, arena);
        if segments.is_empty() {
            for i in 0..len {
                results.push(crate::arena::value::reborrow_arena_value(src.get(i)));
            }
        } else {
            for i in 0..len {
                let item = src.get(i);
                match crate::arena::value::arena_traverse_segments(item, segments, arena) {
                    Some(v) => results.push(crate::arena::value::reborrow_arena_value(v)),
                    None => results.push(ArenaValue::Null),
                }
            }
        }
        return Ok(arena.alloc(ArenaValue::Array(results.into_bump_slice())));
    }

    // GENERAL PATH: zero-clone via ArenaContextStack — frame data is the
    // arena-resident item; body dispatches through arena.
    let mut results: bumpalo::collections::Vec<'a, ArenaValue<'a>> =
        bumpalo::collections::Vec::with_capacity_in(len, arena);
    let mut pushed = false;
    let total = len as u32;
    for i in 0..len {
        let item = src.get(i);
        if !pushed {
            actx.push_with_index(item, 0);
            pushed = true;
        } else {
            actx.replace_top_data(item, i);
        }
        let av = engine.eval_iter_body(body, actx, arena, i as u32, total)?;
        results.push(crate::arena::value::reborrow_arena_value(av));
    }
    if pushed {
        actx.pop();
    }
    Ok(arena.alloc(ArenaValue::Array(results.into_bump_slice())))
}

/// Internal helper: arena-mode quantifier (all / some / none).
/// `early_truthy` controls short-circuit semantics:
///   - `all`: early_truthy = false (false ⇒ return false immediately)
///   - `some`: early_truthy = true (true ⇒ return true immediately)
///   - `none`: same as `some` but invert the final result
#[inline]
fn evaluate_quantifier_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
    shape: QuantifierShape,
) -> Result<&'a ArenaValue<'a>> {
    if args.len() != 2 {
        return Err(crate::constants::invalid_args());
    }

    let predicate = &args[1];
    let src = match resolve_iter_input(&args[0], actx, engine, arena)? {
        ResolvedInput::Iterable(s) => s,
        ResolvedInput::Empty => return Ok(arena.alloc(ArenaValue::Bool(shape.empty_result))),
        ResolvedInput::Bridge(av) => {
            return quantifier_arena_bridge(av, predicate, shape, actx, engine, arena);
        }
    };

    if src.is_empty() {
        return Ok(arena.alloc(ArenaValue::Bool(shape.empty_result)));
    }

    // Fast predicate path — no context push, no clones.
    if let Some(fast_pred) = FastPredicate::try_detect(predicate) {
        let len = src.len();
        for i in 0..len {
            if fast_pred.evaluate(src.get(i), arena) == shape.short_circuit_on {
                return Ok(arena.alloc(ArenaValue::Bool(shape.finalize(true))));
            }
        }
        return Ok(arena.alloc(ArenaValue::Bool(shape.finalize(false))));
    }

    // General path: zero-clone via ArenaContextStack.
    let mut pushed = false;
    let mut found_short = false;
    let len = src.len();
    let total = len as u32;
    for i in 0..len {
        let item = src.get(i);
        if !pushed {
            actx.push_with_index(item, 0);
            pushed = true;
        } else {
            actx.replace_top_data(item, i);
        }
        let av = engine.eval_iter_body(predicate, actx, arena, i as u32, total)?;
        if crate::arena::is_truthy_arena(av, engine) == shape.short_circuit_on {
            found_short = true;
            break;
        }
    }
    if pushed {
        actx.pop();
    }
    Ok(arena.alloc(ArenaValue::Bool(shape.finalize(found_short))))
}

/// Arena-mode `all` — true iff every item satisfies predicate. Short-circuits on false.
#[inline]
pub(crate) fn evaluate_all_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    // all: early-exit on false; empty array ⇒ false (matching existing impl,
    // which deliberately rejects vacuous truth — see evaluate_all in this file).
    evaluate_quantifier_arena(
        args,
        actx,
        engine,
        arena,
        QuantifierShape {
            short_circuit_on: false,
            invert_final: false,
            empty_result: false,
        },
    )
}

/// Arena-mode `some` — true iff any item satisfies predicate. Short-circuits on true.
#[inline]
pub(crate) fn evaluate_some_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    // some: early-exit on true; empty array ⇒ false.
    evaluate_quantifier_arena(
        args,
        actx,
        engine,
        arena,
        QuantifierShape {
            short_circuit_on: true,
            invert_final: false,
            empty_result: false,
        },
    )
}

/// Arena-mode `none` — true iff no item satisfies predicate. Short-circuits on true.
#[inline]
pub(crate) fn evaluate_none_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    // none: early-exit on true (then return false); empty array ⇒ true.
    evaluate_quantifier_arena(
        args,
        actx,
        engine,
        arena,
        QuantifierShape {
            short_circuit_on: true,
            invert_final: true,
            empty_result: true,
        },
    )
}

/// `reduce` — folds an array into a single value via an accumulator. Input
/// resolves via `resolve_iter_input` (so `reduce(filter(...), +, 0)`
/// composes), with inline arithmetic fast paths for the dominant
/// `current op accumulator` pattern.
#[inline]
pub(crate) fn evaluate_reduce_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if args.len() < 2 || args.len() > 3 {
        return Err(crate::constants::invalid_args());
    }

    let body = &args[1];
    let initial: &'a ArenaValue<'a> = if args.len() == 3 {
        engine.evaluate_arena_node(&args[2], actx, arena)?
    } else {
        crate::arena::pool::singleton_null()
    };

    let src = match resolve_iter_input(&args[0], actx, engine, arena)? {
        ResolvedInput::Iterable(s) => s,
        ResolvedInput::Empty => return Ok(initial),
        ResolvedInput::Bridge(av) => {
            return reduce_arena_bridge(av, body, initial, actx, engine, arena);
        }
    };

    if src.is_empty() {
        return Ok(initial);
    }

    // FAST PATH: {op: [val("current"[+path]), val("accumulator")]} for + / - / *.
    // Mirrors try_reduce_fast_path but iterates IterSrc to support both
    // borrowed-input and arena-Refs cases.
    if let CompiledNode::BuiltinOperator {
        opcode,
        args: body_args,
        ..
    } = body
        && body_args.len() == 2
        && matches!(opcode, OpCode::Add | OpCode::Multiply | OpCode::Subtract)
        && let Some(result) = try_reduce_fast_path_arena(&src, initial, body_args, *opcode, arena)
    {
        return Ok(result);
    }

    // GENERAL PATH: zero-clone via ArenaContextStack. Frame holds
    // `&'a ArenaValue<'a>` for both the current item and the accumulator.
    // Body dispatches through arena and the result threads as
    // `&'a ArenaValue<'a>` between iterations.
    let mut acc_av: &'a ArenaValue<'a> = initial;
    let mut pushed = false;
    let len = src.len();
    let total = len as u32;
    for i in 0..len {
        let item = src.get(i);
        if !pushed {
            actx.push_reduce(item, acc_av);
            pushed = true;
        } else {
            actx.replace_reduce_data(item, acc_av);
        }
        acc_av = engine.eval_iter_body(body, actx, arena, i as u32, total)?;
    }
    if pushed {
        actx.pop();
    }
    Ok(acc_av)
}

/// Arena variant of `try_reduce_fast_path` — same logic, iterates `IterSrc`.
fn try_reduce_fast_path_arena<'a>(
    src: &IterSrc<'a>,
    initial: &'a ArenaValue<'a>,
    body_args: &[CompiledNode],
    opcode: OpCode,
    arena: &'a Bump,
) -> Option<&'a ArenaValue<'a>> {
    // Identify which arg is current and which is accumulator.
    let (current_arg, _acc_arg) = match (&body_args[0], &body_args[1]) {
        (
            CompiledNode::CompiledVar {
                reduce_hint: hint0, ..
            },
            CompiledNode::CompiledVar {
                reduce_hint: hint1, ..
            },
        ) => match (hint0, hint1) {
            (
                ReduceHint::Current | ReduceHint::CurrentPath,
                ReduceHint::Accumulator | ReduceHint::AccumulatorPath,
            ) => (&body_args[0], &body_args[1]),
            (
                ReduceHint::Accumulator | ReduceHint::AccumulatorPath,
                ReduceHint::Current | ReduceHint::CurrentPath,
            ) => (&body_args[1], &body_args[0]),
            _ => return None,
        },
        _ => return None,
    };

    let current_segments = if let CompiledNode::CompiledVar {
        segments,
        reduce_hint,
        ..
    } = current_arg
    {
        match reduce_hint {
            ReduceHint::Current => &[][..],
            ReduceHint::CurrentPath => {
                if segments.len() >= 2 {
                    &segments[1..]
                } else {
                    return None;
                }
            }
            _ => return None,
        }
    } else {
        return None;
    };

    let len = src.len();

    // Integer fast path.
    let mut acc_i = initial.as_i64();
    if acc_i.is_some() {
        let mut all_int = true;
        for i in 0..len {
            let item = src.get(i);
            let current_val = if current_segments.is_empty() {
                item
            } else {
                crate::arena::value::arena_traverse_segments(item, current_segments, arena)?
            };
            if let Some(cur_i) = current_val.as_i64() {
                let a = acc_i.unwrap();
                acc_i = Some(match opcode {
                    OpCode::Add => a.wrapping_add(cur_i),
                    OpCode::Multiply => a.wrapping_mul(cur_i),
                    OpCode::Subtract => a.wrapping_sub(cur_i),
                    _ => return None,
                });
            } else {
                all_int = false;
                break;
            }
        }
        if all_int {
            return acc_i.map(|v| {
                &*arena.alloc(ArenaValue::Number(crate::value::NumberValue::from_i64(v)))
            });
        }
    }

    // f64 fallback.
    let mut acc_f = initial.as_f64()?;
    for i in 0..len {
        let item = src.get(i);
        let current_val = if current_segments.is_empty() {
            item
        } else {
            crate::arena::value::arena_traverse_segments(item, current_segments, arena)?
        };
        let cur_f = current_val.as_f64()?;
        acc_f = match opcode {
            OpCode::Add => acc_f + cur_f,
            OpCode::Multiply => acc_f * cur_f,
            OpCode::Subtract => acc_f - cur_f,
            _ => return None,
        };
    }
    Some(
        arena.alloc(ArenaValue::Number(crate::value::NumberValue::from_f64(
            acc_f,
        ))),
    )
}

/// Arena-mode `merge`. Flattens its args (each may itself be a nested arena
/// op) into a single array, skipping nulls.
#[inline]
pub(crate) fn evaluate_merge_arena<'a>(
    args: &'a [CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    // Pre-size for the scalar-arg case (one push per arg). Array args may push
    // more and trigger growth, but profile shows scalar/single-element args
    // dominate — saves the first reserve_internal_or_panic in the common case.
    let mut results: bumpalo::collections::Vec<'a, ArenaValue<'a>> =
        bumpalo::collections::Vec::with_capacity_in(args.len(), arena);

    for arg in args {
        let av = engine.evaluate_arena_node(arg, actx, arena)?;
        match av {
            // Direct arena Array (e.g. result of upstream arena filter/map).
            ArenaValue::Array(items) => {
                for item in items.iter() {
                    if !item_is_null(item) {
                        results.push(crate::arena::value::reborrow_arena_value(item));
                    }
                }
            }
            // Null inputs are skipped per merge semantics.
            ArenaValue::Null => {}
            // Scalar / object — push as-is.
            other => results.push(crate::arena::value::reborrow_arena_value(other)),
        }
    }

    Ok(arena.alloc(ArenaValue::Array(results.into_bump_slice())))
}

/// True iff this arena value is `null`.
#[inline]
fn item_is_null(av: &ArenaValue<'_>) -> bool {
    matches!(av, ArenaValue::Null)
}
