//! `abs`, `ceil`, `floor` — unary numeric ops.

use crate::arena::{DataContextStack, DataValue, bvec};
use crate::value::NumberValue;
use crate::{CompiledNode, DataLogic, Result};
use bumpalo::Bump;

use super::helpers::arena_number;

/// `get_number_strict` for arena values — Number variants and string-as-number
/// only (no bool/null coercion).
#[inline]
fn arena_value_strict_f64(av: &DataValue<'_>) -> Option<f64> {
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
pub(crate) fn arena_unary_math<'a>(
    args: &'a [CompiledNode],
    actx: &mut DataContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
    op: UnaryMathOp,
) -> Result<&'a DataValue<'a>> {
    if args.is_empty() {
        return Err(crate::constants::invalid_args());
    }

    let to_arena = |x: f64, arena: &'a Bump| -> &'a DataValue<'a> {
        if op.returns_int() {
            arena_number(arena, NumberValue::from_i64(x as i64))
        } else {
            arena_number(arena, NumberValue::from_f64(x))
        }
    };

    if args.len() == 1 {
        let av = engine.evaluate_node(&args[0], actx, arena)?;
        let n = arena_value_strict_f64(av).ok_or_else(crate::constants::invalid_args)?;
        return Ok(to_arena(op.apply(n), arena));
    }

    let mut items = bvec::<DataValue<'a>>(arena, args.len());
    for arg in args {
        let av = engine.evaluate_node(arg, actx, arena)?;
        let n = arena_value_strict_f64(av).ok_or_else(crate::constants::invalid_args)?;
        let r = to_arena(op.apply(n), arena);
        items.push(crate::arena::value::reborrow_arena_value(r));
    }
    Ok(arena.alloc(DataValue::Array(items.into_bump_slice())))
}
