//! `abs`, `ceil`, `floor` — unary numeric ops.

use crate::Result;
use crate::arena::{DataValue, bvec};
use crate::operators::eager::Cx;
use crate::operators::extract::{RestArgs, StrictNum};
use datavalue::NumberValue;

/// `abs` / `ceil` / `floor` discriminant for the unified unary-math entry
/// point.
#[derive(Clone, Copy)]
pub(crate) enum UnaryMathOp {
    Abs,
    Ceil,
    Floor,
}

impl UnaryMathOp {
    #[inline]
    fn apply(self, x: f64) -> f64 {
        match self {
            UnaryMathOp::Abs => x.abs(),
            UnaryMathOp::Ceil => x.ceil(),
            UnaryMathOp::Floor => x.floor(),
        }
    }
}

/// `abs` / `ceil` / `floor` over one or more numbers (the row reads them
/// with `StrictNum`: a number or a numeric string, anything else is
/// `InvalidArguments`). One argument gives a number; more give an array
/// of results, each argument evaluated and checked in turn.
///
/// `inline(always)`: merged into its adapter, the call costs what the
/// pre-table body did; as a separate function it measured ~2% slower.
#[inline(always)]
pub(crate) fn unary_math<'a>(
    cx: &mut Cx<'_, 'a>,
    first: f64,
    rest: RestArgs<'a, StrictNum>,
    op: UnaryMathOp,
) -> Result<&'a DataValue<'a>> {
    // `from_f64` makes a whole result an integer when it fits in one (every
    // `ceil` / `floor` result within the i64 range), and keeps anything
    // else a float. An `as i64` cast here once saturated: `ceil(1e20)`
    // came back as `i64::MAX`.
    let to_number = NumberValue::from_f64;

    if rest.is_empty() {
        return Ok(cx.alloc(DataValue::Number(to_number(op.apply(first)))));
    }

    let mut items = bvec::<DataValue<'a>>(cx.arena, rest.len() + 1);
    items.push(DataValue::Number(to_number(op.apply(first))));
    for i in 0..rest.len() {
        let n = rest.get(i, cx)?;
        items.push(DataValue::Number(to_number(op.apply(n))));
    }
    Ok(cx.alloc(DataValue::Array(items.into_bump_slice())))
}
