//! Internal helpers shared by the array operators (filter / map / reduce /
//! quantifiers / sort / slice / merge / length).

use crate::arena::{DataContextStack, DataValue};
use crate::node::{MetadataHint, ReduceHint};
use crate::opcode::OpCode;
use crate::{CompiledNode, DataLogic, Result};
use bumpalo::Bump;

/// Check if a compiled node is loop-invariant (doesn't depend on the current iteration context).
/// Used by filter/quantifier fast paths to detect values that can be evaluated once before the loop.
#[inline]
pub(super) fn is_filter_invariant(node: &CompiledNode) -> bool {
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
pub(super) fn try_extract_filter_field_cmp<'a>(
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
pub(super) fn evaluate_invariant_no_push<'a>(
    invariant_node: &'a CompiledNode,
    actx: &mut DataContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if let CompiledNode::Value { value, .. } = invariant_node {
        return Ok(arena.alloc(value.to_arena(arena)));
    }
    let null_av: &'a DataValue<'a> = arena.alloc(DataValue::Null);
    actx.push(null_av);
    let result = engine.evaluate_node(invariant_node, actx, arena);
    actx.pop();
    result
}

/// Represents a detected fast-path predicate pattern for quantifier/filter operators.
/// Avoids per-item context push/pop and dispatch overhead.
/// When `segments` is `Some`, compares a field extracted via path traversal;
/// when `None`, compares the whole item directly.
pub(super) enum FastPredicate<'a> {
    /// Strict equality (===) or inequality (!==) against a literal
    StrictEq {
        segments: Option<&'a [crate::node::PathSegment]>,
        literal: &'a datavalue::OwnedDataValue,
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
    pub(super) fn try_detect(predicate: &'a CompiledNode) -> Option<Self> {
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
        item: &'b DataValue<'b>,
        arena: &'b Bump,
    ) -> Option<&'b DataValue<'b>> {
        match segments {
            None => Some(item),
            Some(segs) => crate::arena::value::arena_traverse_segments(item, segs, arena),
        }
    }

    /// Evaluate this predicate against a single item.
    #[inline]
    pub(super) fn evaluate<'b>(&self, item: &'b DataValue<'b>, arena: &'b Bump) -> bool {
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

/// Strict equality between two `DataValue`s.
#[inline]
pub(super) fn arena_value_equals_arena(a: &DataValue<'_>, b: &DataValue<'_>) -> bool {
    match (a, b) {
        (DataValue::Null, DataValue::Null) => true,
        (DataValue::Bool(x), DataValue::Bool(y)) => x == y,
        (DataValue::Number(x), DataValue::Number(y)) => match (x.as_i64(), y.as_i64()) {
            (Some(a), Some(b)) => a == b,
            _ => x.as_f64() == y.as_f64(),
        },
        (DataValue::String(x), DataValue::String(y)) => *x == *y,
        (DataValue::Array(x), DataValue::Array(y)) => {
            x.len() == y.len()
                && x.iter()
                    .zip(y.iter())
                    .all(|(a, b)| arena_value_equals_arena(a, b))
        }
        (DataValue::Object(x), DataValue::Object(y)) => {
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

/// Strict equality between a [`DataValue`] (arena) and an
/// [`OwnedDataValue`] literal — used by `FastPredicate::StrictEq` to
/// compare an arena-resident item against a compile-time literal without
/// allocating.
#[inline]
fn arena_value_equals_value(av: &DataValue<'_>, v: &datavalue::OwnedDataValue) -> bool {
    use datavalue::OwnedDataValue;
    match (av, v) {
        (DataValue::Null, OwnedDataValue::Null) => true,
        (DataValue::Bool(a), OwnedDataValue::Bool(b)) => a == b,
        (DataValue::Number(a), OwnedDataValue::Number(b)) => a == b,
        (DataValue::String(s), OwnedDataValue::String(b)) => *s == b.as_str(),
        (DataValue::Array(items), OwnedDataValue::Array(b)) => {
            items.len() == b.len()
                && items
                    .iter()
                    .zip(b.iter())
                    .all(|(x, y)| arena_value_equals_value(x, y))
        }
        (DataValue::Object(pairs), OwnedDataValue::Object(b)) => {
            if pairs.len() != b.len() {
                return false;
            }
            for (k, av) in *pairs {
                match b.iter().find(|(bk, _)| bk == *k) {
                    Some((_, bv)) => {
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

// =============================================================================
// Iterator input resolution
// =============================================================================

/// Unified view over an iterator op's input collection. Single shape:
/// arena slice of `DataValue`. Wrapper kept for API stability.
#[derive(Clone, Copy)]
pub(crate) struct IterSrc<'a>(pub(crate) &'a [DataValue<'a>]);

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
    pub(crate) fn get(&self, i: usize) -> &'a DataValue<'a> {
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
    Bridge(&'a DataValue<'a>),
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
    actx: &mut DataContextStack<'a>,
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
        let av = engine.evaluate_node(arg, actx, arena)?;
        return Ok(arena_value_as_iter(av));
    }

    // Path 3: anything else — evaluate through arena dispatch so the caller
    // can handle the result natively (Object iteration / single-element wrap /
    // error per op semantics).
    let av = engine.evaluate_node(arg, actx, arena)?;
    Ok(arena_value_as_iter(av))
}

/// Convert a resolved arena value into an `IterSrc` view, or signal Empty/Bridge.
fn arena_value_as_iter<'a>(av: &'a DataValue<'a>) -> ResolvedInput<'a> {
    match av {
        DataValue::Null => ResolvedInput::Empty,
        DataValue::Array(items) => ResolvedInput::Iterable(IterSrc(items)),
        _ => ResolvedInput::Bridge(av),
    }
}

/// Try to obtain the input collection by borrowing from the caller's root data.
/// Returns `Some(&DataValue)` when args[0] is a simple root-scope `var` that
/// resolves into the input data. The returned reference lives for the arena
/// lifetime `'a`.
#[inline]
fn try_borrow_collection_from_root<'a>(
    arg: &'a CompiledNode,
    actx: &DataContextStack<'a>,
    arena: &'a Bump,
) -> Option<&'a DataValue<'a>> {
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

/// True iff this arena value is `null`.
#[inline]
pub(super) fn item_is_null(av: &DataValue<'_>) -> bool {
    matches!(av, DataValue::Null)
}
