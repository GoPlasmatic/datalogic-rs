//! Shared pieces of the fused `map` / `reduce` fast paths: the hinted
//! per-row field resolver, the exact arithmetic combine, and the fusible
//! map-body classification.

use crate::CompiledNode;
use crate::arena::DataValue;
use crate::node::{MetadataHint, ReduceHint};
use crate::operators::meta::ArithOp;
use datavalue::NumberValue;

/// Per-loop resolver for a scope-0 var path against successive row items.
/// Single object-key paths (the dominant row shape) carry a remembered pair
/// index across rows — see `object_lookup_field_hinted` — so homogeneous
/// rows resolve in one key compare after the first. Everything else
/// delegates to the general segment traversal. Shared by the map fast
/// paths and the reduce(map(...)) fusion loops.
pub(super) struct FieldCursor<'n> {
    segments: &'n [crate::node::PathSegment],
    /// Key of a single-`Field`/`FieldOrIndex` segment path, when applicable.
    single_key: Option<&'n str>,
    /// Last hit index for the hinted lookup.
    hint: usize,
}

impl<'n> FieldCursor<'n> {
    #[inline]
    pub(super) fn new(segments: &'n [crate::node::PathSegment]) -> Self {
        use crate::node::PathSegment;
        let single_key = match segments {
            [PathSegment::Field(k)] => Some(k.as_ref()),
            [PathSegment::FieldOrIndex(k, _)] => Some(k.as_ref()),
            _ => None,
        };
        Self {
            segments,
            single_key,
            hint: 0,
        }
    }

    #[inline(always)]
    pub(super) fn resolve<'a>(&mut self, item: &'a DataValue<'a>) -> Option<&'a DataValue<'a>> {
        if let (Some(key), DataValue::Object(pairs)) = (self.single_key, item) {
            return crate::arena::value::object_lookup_field_hinted(pairs, key, &mut self.hint);
        }
        if self.segments.is_empty() {
            Some(item)
        } else {
            crate::arena::value::traverse_segments(item, self.segments)
        }
    }
}

// =============================================================================
// Exact arithmetic for the map / reduce fast paths
// =============================================================================

/// `a op b` with the binary arithmetic operators' representation rules,
/// for one pair: integer math when both operands are exactly `i64` (whole
/// floats included), promoting this result alone to `from_f64` on
/// overflow; `from_f64` otherwise. Generic over the two combines so each
/// operation's loop gets them inlined (see [`with_arith`]).
///
/// Every map / reduce arithmetic fast path computes element by element
/// through this, never in a separate integer pass that restarts the whole
/// collection in `f64` on the first overflow: that restart rounded exact
/// neighbours past 2^53 (`i64::MIN + 3` came back as `i64::MIN`), the bug
/// class of issue #61. Deliberately not `NumberValue::add` / `sub` / `mul`,
/// which leave an overflowed whole result as `Float` where the operators
/// collapse it back to `Integer`.
#[inline(always)]
pub(super) fn combine(
    a: NumberValue,
    b: NumberValue,
    int_op: impl Fn(i64, i64) -> Option<i64>,
    float_op: impl Fn(f64, f64) -> f64,
) -> NumberValue {
    match (a.as_i64(), b.as_i64()) {
        (Some(x), Some(y)) => match int_op(x, y) {
            Some(r) => NumberValue::from_i64(r),
            None => NumberValue::from_f64(float_op(x as f64, y as f64)),
        },
        _ => NumberValue::from_f64(float_op(a.as_f64(), b.as_f64())),
    }
}

/// [`combine`] for a runtime operation: a `match` per call. For a loop,
/// use [`with_arith`], which matches once outside it.
#[inline(always)]
pub(super) fn arith_number(op: ArithOp, a: NumberValue, b: NumberValue) -> NumberValue {
    with_arith!(op, |f| f(a, b))
}

/// Evaluate `$body` with `$int` bound to `op`'s checked integer operation
/// (`fn(i64, i64) -> Option<i64>`) and `$float` to its `f64` operation,
/// once per operation, so a loop inside `$body` does no dispatch on the
/// operation per item. For loops that classify an operand up front (a
/// literal known to be integral); everything else uses [`with_arith`].
macro_rules! with_ops {
    ($op:expr, |$int:ident, $float:ident| $body:expr) => {
        match $op {
            ArithOp::Add => {
                let ($int, $float) = (i64::checked_add, |x: f64, y: f64| x + y);
                $body
            }
            ArithOp::Sub => {
                let ($int, $float) = (i64::checked_sub, |x: f64, y: f64| x - y);
                $body
            }
            ArithOp::Mul => {
                let ($int, $float) = (i64::checked_mul, |x: f64, y: f64| x * y);
                $body
            }
        }
    };
}
pub(super) use with_ops;

/// Evaluate `$body` with `$f` bound to `op`'s exact combine
/// (`Fn(NumberValue, NumberValue) -> NumberValue`), once per operation, so
/// a loop inside `$body` does no dispatch on the operation per item.
macro_rules! with_arith {
    ($op:expr, |$f:ident| $body:expr) => {
        $crate::operators::array::fused::with_ops!($op, |int_op, float_op| {
            let $f = |a: NumberValue, b: NumberValue| {
                $crate::operators::array::fused::combine(a, b, int_op, float_op)
            };
            $body
        })
    };
}
pub(super) use with_arith;

/// `x op y` for an integer pair, promoting this result to `from_f64` on
/// overflow: [`combine`]'s integer arm, for loops that already know both
/// operands are integers.
#[inline(always)]
pub(super) fn combine_ints(
    x: i64,
    y: i64,
    int_op: impl Fn(i64, i64) -> Option<i64>,
    float_op: impl Fn(f64, f64) -> f64,
) -> NumberValue {
    match int_op(x, y) {
        Some(r) => NumberValue::from_i64(r),
        None => NumberValue::from_f64(float_op(x as f64, y as f64)),
    }
}

/// Classified fusible map body shape — the three per-item transforms the
/// map fast paths execute without per-item context pushes. Shared with the
/// reduce(map(...)) fusion in `reduce.rs`, which runs the same transforms
/// feeding a fold instead of materializing the intermediate array.
pub(super) enum FusedMapBody<'n> {
    /// `{"var": "path"}` — extract (identity when segments are empty).
    Extract {
        segments: &'n [crate::node::PathSegment],
    },
    /// `{op: [var, literal]}` or `{op: [literal, var]}` for + / - / *.
    ArithVarLit {
        op: ArithOp,
        segments: &'n [crate::node::PathSegment],
        lit: &'n datavalue::OwnedDataValue,
        var_is_lhs: bool,
    },
    /// `{op: [var_a, var_b]}` for + / - / * — the line-total shape.
    ArithVarVar {
        op: ArithOp,
        a_segments: &'n [crate::node::PathSegment],
        b_segments: &'n [crate::node::PathSegment],
    },
}

/// Match a plain scope-0 var with no reduce/metadata hints and no default —
/// the only var shape the fused loops can resolve with a `FieldCursor`.
#[inline]
fn plain_var_segments(node: &CompiledNode) -> Option<&[crate::node::PathSegment]> {
    if let CompiledNode::Var {
        scope_level: 0,
        segments,
        reduce_hint: ReduceHint::None,
        metadata_hint: MetadataHint::None,
        default_value: None,
        ..
    } = node
    {
        Some(segments.as_ref())
    } else {
        None
    }
}

impl FusedMapBody<'_> {
    /// Structural classification only — numeric checks (e.g. the literal
    /// being coercible) stay in the executing loops, which fall back to
    /// the general path when they fail.
    pub(super) fn detect(body: &CompiledNode) -> Option<FusedMapBody<'_>> {
        if let Some(segments) = plain_var_segments(body) {
            return Some(FusedMapBody::Extract { segments });
        }
        let CompiledNode::BuiltinOperator { opcode, args, .. } = body else {
            return None;
        };
        let op = opcode.arith_op()?;
        if args.len() != 2 {
            return None;
        }
        match (&args[0], &args[1]) {
            (var, CompiledNode::Value { value, .. }) => {
                let segments = plain_var_segments(var)?;
                Some(FusedMapBody::ArithVarLit {
                    op,
                    segments,
                    lit: value,
                    var_is_lhs: true,
                })
            }
            (CompiledNode::Value { value, .. }, var) => {
                let segments = plain_var_segments(var)?;
                Some(FusedMapBody::ArithVarLit {
                    op,
                    segments,
                    lit: value,
                    var_is_lhs: false,
                })
            }
            (a, b) => {
                let a_segments = plain_var_segments(a)?;
                let b_segments = plain_var_segments(b)?;
                Some(FusedMapBody::ArithVarVar {
                    op,
                    a_segments,
                    b_segments,
                })
            }
        }
    }
}
