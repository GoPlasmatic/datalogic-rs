//! `abs`, `ceil`, `floor` — unary numeric ops.

use crate::arena::{ContextStack, DataValue, bvec};
use datavalue::NumberValue;
use crate::{CompiledNode, Engine, Result};
use bumpalo::Bump;

use super::helpers::alloc_number;

/// `get_number_strict` for arena values — Number variants and string-as-number
/// only (no bool/null coercion).
#[inline]
fn value_strict_f64(av: &DataValue<'_>) -> Option<f64> {
    match av {
        DataValue::Number(n) => Some(n.as_f64()),
        DataValue::String(s) => s.parse().ok(),
        _ => None,
    }
}

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

    /// True when the result should be quantized to i64 (ceil / floor) rather
    /// than kept as f64 (abs).
    #[inline]
    fn returns_int(self) -> bool {
        matches!(self, UnaryMathOp::Ceil | UnaryMathOp::Floor)
    }
}

/// Generic native unary math op shared by abs / ceil / floor.
/// - `args.is_empty()` → InvalidArguments
/// - 1 arg, numeric → apply op, return arena Number
/// - 1 arg, non-numeric → InvalidArguments
/// - >1 args → variadic, return arena Array of results (any non-numeric → error)
#[inline]
pub(crate) fn unary_math<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
    op: UnaryMathOp,
) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        return Err(crate::Error::invalid_args());
    }

    let to_arena = |x: f64, arena: &'a Bump| -> &'a DataValue<'a> {
        if op.returns_int() {
            alloc_number(arena, NumberValue::from_i64(x as i64))
        } else {
            alloc_number(arena, NumberValue::from_f64(x))
        }
    };

    if args.len() == 1 {
        let av = engine.dispatch_node(&args[0], ctx, arena)?;
        let n = value_strict_f64(av).ok_or_else(crate::Error::invalid_args)?;
        return Ok(to_arena(op.apply(n), arena));
    }

    let mut items = bvec::<DataValue<'a>>(arena, args.len());
    for arg in args {
        let av = engine.dispatch_node(arg, ctx, arena)?;
        let n = value_strict_f64(av).ok_or_else(crate::Error::invalid_args)?;
        let r = to_arena(op.apply(n), arena);
        items.push(*r);
    }
    Ok(arena.alloc(DataValue::Array(items.into_bump_slice())))
}
