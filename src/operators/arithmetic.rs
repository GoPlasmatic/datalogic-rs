//! Arithmetic operators for numeric computations.
//!
//! This module provides all arithmetic operators with support for:
//! - Integer and floating-point arithmetic
//! - Overflow protection with automatic promotion to float
//! - DateTime and Duration arithmetic
//! - Configurable NaN handling
//!
//! # Operators
//!
//! | Operator | Description | Example |
//! |----------|-------------|---------|
//! | `+` | Addition | `{"+": [1, 2, 3]}` → `6` |
//! | `-` | Subtraction | `{"-": [10, 3]}` → `7` |
//! | `*` | Multiplication | `{"*": [2, 3, 4]}` → `24` |
//! | `/` | Division | `{"/": [10, 2]}` → `5` |
//! | `%` | Modulo | `{"%": [10, 3]}` → `1` |
//! | `min` | Minimum value | `{"min": [3, 1, 4]}` → `1` |
//! | `max` | Maximum value | `{"max": [3, 1, 4]}` → `4` |
//!
//! # Overflow Handling Pattern
//!
//! All arithmetic operators use the same pattern for overflow protection:
//!
//! 1. **Track integer precision**: Use `all_integers` flag to track if we can stay in i64
//! 2. **Checked arithmetic**: Use `checked_add`, `checked_mul`, etc. for i64 operations
//! 3. **Overflow promotion**: On overflow, switch to f64 and continue accumulating
//! 4. **Result preservation**: Return i64 when possible, f64 otherwise
//!
//! This approach maximizes integer precision while gracefully handling overflow:
//!
//! ```text
//! // Example overflow handling in addition:
//! match int_sum.checked_add(i) {
//!     Some(sum) => int_sum = sum,         // No overflow: continue with integers
//!     None => {
//!         all_integers = false;            // Overflow: switch to float
//!         float_sum = int_sum as f64 + i as f64;
//!     }
//! }
//! ```
//!
//! # DateTime Arithmetic
//!
//! Arithmetic operators also handle DateTime and Duration values:
//! - `datetime + duration` → `datetime`
//! - `datetime - datetime` → `duration`
//! - `duration + duration` → `duration`
//! - `duration * number` → `duration`
//!
//! # NaN Handling
//!
//! When a value cannot be coerced to a number, behavior depends on `NanHandling` config:
//! - `ThrowError`: Return error (default)
//! - `IgnoreValue`: Skip non-numeric values
//! - `CoerceToZero`: Treat as 0
//! - `ReturnNull`: Return null

use serde_json::Value;

use crate::config::NanHandling;
use crate::constants::INVALID_ARGS;
use crate::value_helpers::{coerce_to_number, try_coerce_to_integer};
use crate::{CompiledNode, DataLogic, Error, Result};

/// Result of NaN handling check: what the caller should do with a non-numeric value.
enum NanAction {
    /// Skip/ignore this value (IgnoreValue or CoerceToZero)
    Skip,
    /// Return null immediately
    ReturnNull,
}

/// Checks the engine's NaN handling config and returns the appropriate action.
/// Returns `Err` for ThrowError, `Ok(NanAction)` otherwise.
#[inline]
fn handle_nan(engine: &DataLogic) -> Result<NanAction> {
    match engine.config().arithmetic_nan_handling {
        NanHandling::ThrowError => Err(crate::constants::nan_error()),
        NanHandling::IgnoreValue | NanHandling::CoerceToZero => Ok(NanAction::Skip),
        NanHandling::ReturnNull => Ok(NanAction::ReturnNull),
    }
}

// =============================================================================
// Arena-mode array-consumer ops (Phase 5: max / min / + / *)
//
// These are "pipeline tops" — they consume an array (typically produced by an
// upstream filter/map) and return a single Number. They benefit from arena
// dispatch in two ways:
//   1. Input borrow: when args[0] is a root var, no clone of the input array.
//   2. Composition: when args[0] is filter/map/all/some/none, the arena
//      intermediate slice is consumed directly without value-mode bridging.
//
// Each op handles the SINGLE-ARG ARRAY form (e.g. `max(items)` over an array).
// The multi-arg form (`max(a, b, c)`) stays on the value path — it doesn't
// involve array iteration so arena gives no win.
// =============================================================================

use crate::arena::{ArenaContextStack, ArenaValue};
use crate::operators::array::{ResolvedInput, resolve_iter_input};
use bumpalo::Bump;

/// Generic helper for max/min over an arena-iterable input. `pick_better`
/// returns true when `candidate_f` should replace `best_f` (strictly better).
#[inline]
fn arena_min_max<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
    init: f64,
    pick_better: fn(f64, f64) -> bool,
) -> Result<&'a ArenaValue<'a>> {
    if args.is_empty() {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    // Multi-arg variadic form: evaluate each arg, pick the best Number.
    if args.len() > 1 {
        let mut best_f = init;
        let mut best_av: Option<&'a ArenaValue<'a>> = None;
        for arg in args {
            let av = engine.evaluate_arena_node(arg, actx, arena)?;
            let f = match av {
                ArenaValue::Number(n) => n.as_f64(),
                ArenaValue::InputRef(Value::Number(n)) => n
                    .as_f64()
                    .ok_or_else(|| Error::InvalidArguments(INVALID_ARGS.into()))?,
                _ => return Err(Error::InvalidArguments(INVALID_ARGS.into())),
            };
            if pick_better(f, best_f) {
                best_f = f;
                best_av = Some(av);
            }
        }
        return match best_av {
            Some(av) => Ok(av),
            None => Ok(crate::arena::pool::singleton_null()),
        };
    }

    // Reject literal-array arg shape (matches value-mode error).
    if matches!(&args[0], CompiledNode::Array { .. }) {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }
    if let CompiledNode::Value { value, .. } = &args[0]
        && matches!(value, Value::Array(_))
    {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    let src = match resolve_iter_input(&args[0], actx, engine, arena)? {
        ResolvedInput::Iterable(s) => s,
        ResolvedInput::Empty => return Err(Error::InvalidArguments(INVALID_ARGS.into())),
        ResolvedInput::Bridge(av) => {
            // Array-shaped bridges iterate natively.
            if matches!(
                av,
                ArenaValue::Array(_) | ArenaValue::InputRef(Value::Array(_))
            ) {
                return arena_min_max_from_av(av, init, pick_better, arena);
            }
            // Single non-array arg: value-mode `evaluate_max`/`evaluate_min`
            // requires the operand to be a `Value::Number` and returns it
            // unchanged; non-numeric is InvalidArguments.
            let is_number = matches!(
                av,
                ArenaValue::Number(_) | ArenaValue::InputRef(Value::Number(_))
            );
            if !is_number {
                return Err(Error::InvalidArguments(INVALID_ARGS.into()));
            }
            return Ok(av);
        }
    };

    if src.is_empty() {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    let mut best_f = init;
    let mut best_idx: Option<usize> = None;
    let len = src.len();
    for i in 0..len {
        match src.get(i) {
            Value::Number(n) => {
                if let Some(f) = n.as_f64()
                    && pick_better(f, best_f)
                {
                    best_f = f;
                    best_idx = Some(i);
                }
            }
            _ => return Err(Error::InvalidArguments(INVALID_ARGS.into())),
        }
    }

    match best_idx {
        Some(i) => {
            // Borrow the original Number to preserve integer typing — the arena
            // result is just an InputRef, no Number copy.
            Ok(arena.alloc(ArenaValue::InputRef(src.get(i))))
        }
        None => Ok(arena.alloc(ArenaValue::Null)),
    }
}

/// Iterate an `&'a ArenaValue<'a>` (Array variant) for min/max. Used when
/// the input came from a composed arena op whose items aren't uniformly
/// `InputRef` (e.g. `merge` mixing borrowed and inline numbers).
#[inline]
fn arena_min_max_from_av<'a>(
    av: &'a ArenaValue<'a>,
    init: f64,
    pick_better: fn(f64, f64) -> bool,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    let items: &[ArenaValue<'a>] = match av {
        ArenaValue::Array(items) => items,
        ArenaValue::InputRef(Value::Array(arr)) => {
            // Walk borrowed array directly.
            if arr.is_empty() {
                return Err(Error::InvalidArguments(INVALID_ARGS.into()));
            }
            let mut best_f = init;
            let mut best: Option<&Value> = None;
            for v in arr {
                let f = v
                    .as_f64()
                    .ok_or_else(|| Error::InvalidArguments(INVALID_ARGS.into()))?;
                if pick_better(f, best_f) {
                    best_f = f;
                    best = Some(v);
                }
            }
            return match best {
                Some(v) => Ok(arena.alloc(ArenaValue::InputRef(v))),
                None => Ok(arena.alloc(ArenaValue::Null)),
            };
        }
        _ => return Err(Error::InvalidArguments(INVALID_ARGS.into())),
    };
    if items.is_empty() {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }
    let mut best_f = init;
    let mut best_idx: Option<usize> = None;
    for (i, it) in items.iter().enumerate() {
        let f = it
            .as_f64()
            .ok_or_else(|| Error::InvalidArguments(INVALID_ARGS.into()))?;
        if pick_better(f, best_f) {
            best_f = f;
            best_idx = Some(i);
        }
    }
    match best_idx {
        Some(i) => Ok(arena.alloc(crate::arena::value::reborrow_arena_value(&items[i]))),
        None => Ok(arena.alloc(ArenaValue::Null)),
    }
}

/// Arena-mode max(single_array_arg).
#[inline]
pub(crate) fn evaluate_max_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    arena_min_max(args, actx, engine, arena, f64::NEG_INFINITY, |c, b| c > b)
}

/// Arena-mode min(single_array_arg).
#[inline]
pub(crate) fn evaluate_min_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    arena_min_max(args, actx, engine, arena, f64::INFINITY, |c, b| c < b)
}

/// Arena-mode `+`. Handles 0-arg (identity), 1-arg array (sum elements),
/// 1-arg single value (coerce + return), 2-arg (numeric or datetime native),
/// and variadic (sum all args).
#[inline]
pub(crate) fn evaluate_add_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if args.is_empty() {
        return Ok(arena_number(arena, NumberValue::from_i64(0)));
    }
    if args.len() == 1 {
        return arena_one_arg_arith(&args[0], actx, engine, arena, ArithOp::Add);
    }
    if args.len() == 2 {
        let a_av = engine.evaluate_arena_node(&args[0], actx, arena)?;
        let b_av = engine.evaluate_arena_node(&args[1], actx, arena)?;

        // Integer-preserving fast path (both native Number with i64 values).
        if let (Some(ia), Some(ib)) = (a_av.as_i64(), b_av.as_i64()) {
            return match ia.checked_add(ib) {
                Some(s) => Ok(arena_number(arena, NumberValue::from_i64(s))),
                None => Ok(arena_number(
                    arena,
                    NumberValue::from_f64(ia as f64 + ib as f64),
                )),
            };
        }

        // Cross to Value-Cow for config-aware coercion (free for InputRef
        // operands; one Value clone for inline arena variants).
        let a_cow = crate::arena::arena_to_value_cow(a_av);
        let b_cow = crate::arena::arena_to_value_cow(b_av);
        if let (Some(i1), Some(i2)) = (
            try_coerce_to_integer(&a_cow, engine),
            try_coerce_to_integer(&b_cow, engine),
        ) {
            return match i1.checked_add(i2) {
                Some(s) => Ok(arena_number(arena, NumberValue::from_i64(s))),
                None => Ok(arena_number(
                    arena,
                    NumberValue::from_f64(i1 as f64 + i2 as f64),
                )),
            };
        }
        if let (Some(f1), Some(f2)) = (
            coerce_to_number(&a_cow, engine),
            coerce_to_number(&b_cow, engine),
        ) {
            return Ok(arena_number(arena, NumberValue::from_f64(f1 + f2)));
        }

        // Datetime / duration arithmetic.
        #[cfg(feature = "datetime")]
        {
            if let Some(av) = arena_datetime_add(a_av, b_av, arena) {
                return Ok(av);
            }
        }

        // Non-numeric, non-datetime — handle NaN per config (mirrors
        // value-mode evaluate_add 2-arg path).
        let mut sum = 0.0f64;
        for cow in [&a_cow, &b_cow] {
            if let Some(f) = coerce_to_number(cow, engine) {
                sum += f;
            } else {
                match handle_nan(engine)? {
                    NanAction::Skip => {}
                    NanAction::ReturnNull => return Ok(crate::arena::pool::singleton_null()),
                }
            }
        }
        return Ok(arena_number(arena, NumberValue::from_f64(sum)));
    }
    arena_variadic_fold(
        args,
        actx,
        engine,
        arena,
        VariadicFoldSpec {
            int_init: 0,
            float_init: 0.0,
            i_combine: i64::checked_add,
            f_combine: |a, b| a + b,
        },
    )
}

/// Arena-mode `*`. 0-arg (1), 1-arg array (product), 1-arg scalar,
/// 2-arg (numeric or duration*scalar native), variadic.
#[inline]
pub(crate) fn evaluate_multiply_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    if args.is_empty() {
        return Ok(arena_number(arena, NumberValue::from_i64(1)));
    }
    if args.len() == 1 {
        return arena_one_arg_arith(&args[0], actx, engine, arena, ArithOp::Multiply);
    }
    if args.len() == 2 {
        let a_av = engine.evaluate_arena_node(&args[0], actx, arena)?;
        let b_av = engine.evaluate_arena_node(&args[1], actx, arena)?;

        // Integer-preserving fast path.
        if let (Some(ia), Some(ib)) = (a_av.as_i64(), b_av.as_i64()) {
            return match ia.checked_mul(ib) {
                Some(p) => Ok(arena_number(arena, NumberValue::from_i64(p))),
                None => Ok(arena_number(
                    arena,
                    NumberValue::from_f64(ia as f64 * ib as f64),
                )),
            };
        }

        // Duration * scalar — checked before generic coercion so duration
        // object inputs aren't coerced to None and lost.
        #[cfg(feature = "datetime")]
        {
            if let Some(av) = arena_datetime_multiply(a_av, b_av, arena) {
                return Ok(av);
            }
        }

        // Config-aware coercion for non-Number operands.
        let a_cow = crate::arena::arena_to_value_cow(a_av);
        let b_cow = crate::arena::arena_to_value_cow(b_av);
        if let (Some(i1), Some(i2)) = (
            try_coerce_to_integer(&a_cow, engine),
            try_coerce_to_integer(&b_cow, engine),
        ) {
            return match i1.checked_mul(i2) {
                Some(p) => Ok(arena_number(arena, NumberValue::from_i64(p))),
                None => Ok(arena_number(
                    arena,
                    NumberValue::from_f64(i1 as f64 * i2 as f64),
                )),
            };
        }
        if let (Some(f1), Some(f2)) = (
            coerce_to_number(&a_cow, engine),
            coerce_to_number(&b_cow, engine),
        ) {
            return Ok(arena_number(arena, NumberValue::from_f64(f1 * f2)));
        }

        // Non-numeric — handle NaN per config (multiplicative identity is 1).
        let mut product = 1.0f64;
        for cow in [&a_cow, &b_cow] {
            if let Some(f) = coerce_to_number(cow, engine) {
                product *= f;
            } else {
                match handle_nan(engine)? {
                    NanAction::Skip => {}
                    NanAction::ReturnNull => return Ok(crate::arena::pool::singleton_null()),
                }
            }
        }
        return Ok(arena_number(arena, NumberValue::from_f64(product)));
    }
    arena_variadic_fold(
        args,
        actx,
        engine,
        arena,
        VariadicFoldSpec {
            int_init: 1,
            float_init: 1.0,
            i_combine: i64::checked_mul,
            f_combine: |a, b| a * b,
        },
    )
}

/// Spec for an integer-fast-path / float-fallback variadic fold:
/// inits, the integer combine (with overflow signaling via `None`), and
/// the float combine.
struct VariadicFoldSpec {
    int_init: i64,
    float_init: f64,
    i_combine: fn(i64, i64) -> Option<i64>,
    f_combine: fn(f64, f64) -> f64,
}

/// Variadic fold over arena-evaluated args with integer-fast-path and
/// overflow promotion to f64. Used by `+` and `*` for the 2+ arg form.
/// Non-numeric args trigger NaN handling per engine config.
#[inline]
fn arena_variadic_fold<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
    spec: VariadicFoldSpec,
) -> Result<&'a ArenaValue<'a>> {
    let mut int_acc: i64 = spec.int_init;
    let mut float_acc: f64 = spec.float_init;
    let mut all_int = true;

    for arg in args {
        let av = engine.evaluate_arena_node(arg, actx, arena)?;
        if all_int && let Some(i) = av.as_i64() {
            match (spec.i_combine)(int_acc, i) {
                Some(r) => int_acc = r,
                None => {
                    all_int = false;
                    float_acc = (spec.f_combine)(int_acc as f64, i as f64);
                }
            }
            continue;
        }
        // Try `as_f64` for native numbers first; fall back to value-mode
        // coercion so `true`/`false`/`null`/numeric strings compose like
        // they do in the legacy variadic path.
        let f_opt = av
            .as_f64()
            .or_else(|| coerce_to_number(&crate::arena::arena_to_value_cow(av), engine));
        if let Some(f) = f_opt {
            if all_int {
                all_int = false;
                float_acc = (spec.f_combine)(int_acc as f64, f);
            } else {
                float_acc = (spec.f_combine)(float_acc, f);
            }
        } else {
            // Non-numeric operand — value-mode `evaluate_add` / `evaluate_multiply`
            // for the variadic (>2) case treats arrays/objects/non-coercibles as
            // NaN per `arithmetic_nan_handling` config. Match that behavior.
            match handle_nan(engine)? {
                NanAction::Skip => continue,
                NanAction::ReturnNull => {
                    return Ok(crate::arena::pool::singleton_null());
                }
            }
        }
    }

    if all_int {
        Ok(arena_number(arena, NumberValue::from_i64(int_acc)))
    } else {
        Ok(arena_number(arena, NumberValue::from_f64(float_acc)))
    }
}

// =============================================================================
// Arena-mode binary arithmetic + math ops
// =============================================================================
//
// For binary forms (subtract, divide, modulo) and unary math ops, args are
// pre-evaluated via `evaluate_arena_node` so var lookups borrow into input
// data via `InputRef`. Numeric extraction goes through
// `coerce_arena_to_number`. Result is `ArenaValue::Number(NumberValue)` —
// inline (no heap alloc).

use crate::arena::value::coerce_arena_to_number;
use crate::value::NumberValue;

#[inline]
fn arena_number<'a>(arena: &'a Bump, n: NumberValue) -> &'a ArenaValue<'a> {
    arena.alloc(ArenaValue::Number(n))
}

/// Operation discriminator for the shared 1-arg fold (`+` and `*`).
#[derive(Clone, Copy)]
enum ArithOp {
    Add,
    Multiply,
}

impl ArithOp {
    #[inline]
    fn identity_int(self) -> i64 {
        match self {
            ArithOp::Add => 0,
            ArithOp::Multiply => 1,
        }
    }

    #[inline]
    fn combine_int(self, a: i64, b: i64) -> Option<i64> {
        match self {
            ArithOp::Add => a.checked_add(b),
            ArithOp::Multiply => a.checked_mul(b),
        }
    }

    #[inline]
    fn combine_f(self, a: f64, b: f64) -> f64 {
        match self {
            ArithOp::Add => a + b,
            ArithOp::Multiply => a * b,
        }
    }
}

/// Native arena 1-arg `+` / `*`. Mirrors value-mode `evaluate_add` / `evaluate_multiply`
/// 1-arg semantics: literal-array reject, then either array-fold the elements
/// or treat as a single-value sum/product.
fn arena_one_arg_arith<'a>(
    arg: &CompiledNode,
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
    op: ArithOp,
) -> Result<&'a ArenaValue<'a>> {
    // Literal array argument is invalid for + / *. Apply NaN config (default
    // ThrowError → propagates the error up).
    let is_literal_array = matches!(arg, CompiledNode::Array { .. })
        || matches!(
            arg,
            CompiledNode::Value {
                value: Value::Array(_),
                ..
            }
        );
    if is_literal_array {
        return match handle_nan(engine)? {
            NanAction::Skip => Ok(arena_number(
                arena,
                NumberValue::from_i64(op.identity_int()),
            )),
            NanAction::ReturnNull => Ok(crate::arena::pool::singleton_null()),
        };
    }

    let av = engine.evaluate_arena_node(arg, actx, arena)?;

    // Array result (e.g. from `var "items"`): fold all elements.
    let array_cow: Option<std::borrow::Cow<'_, [Value]>> = match av {
        ArenaValue::InputRef(Value::Array(arr)) => Some(std::borrow::Cow::Borrowed(arr.as_slice())),
        ArenaValue::Array(items) => Some(std::borrow::Cow::Owned(
            items
                .iter()
                .map(crate::arena::arena_to_value)
                .collect::<Vec<_>>(),
        )),
        _ => None,
    };
    if let Some(arr) = array_cow {
        if arr.is_empty() {
            // 1-arg evaluating to empty array: + → 0, * → 1.
            return Ok(arena_number(
                arena,
                NumberValue::from_i64(op.identity_int()),
            ));
        }
        let mut all_int = true;
        let mut int_acc: i64 = op.identity_int();
        let mut float_acc: f64 = op.identity_int() as f64;
        for elem in arr.iter() {
            if let Some(i) = try_coerce_to_integer(elem, engine) {
                if all_int {
                    match op.combine_int(int_acc, i) {
                        Some(r) => int_acc = r,
                        None => {
                            all_int = false;
                            float_acc = op.combine_f(int_acc as f64, i as f64);
                        }
                    }
                } else {
                    float_acc = op.combine_f(float_acc, i as f64);
                }
            } else if let Some(f) = coerce_to_number(elem, engine) {
                if all_int {
                    all_int = false;
                    float_acc = op.combine_f(int_acc as f64, f);
                } else {
                    float_acc = op.combine_f(float_acc, f);
                }
            } else {
                match handle_nan(engine)? {
                    NanAction::Skip => continue,
                    NanAction::ReturnNull => return Ok(crate::arena::pool::singleton_null()),
                }
            }
        }
        return if all_int {
            Ok(arena_number(arena, NumberValue::from_i64(int_acc)))
        } else {
            Ok(arena_number(arena, NumberValue::from_f64(float_acc)))
        };
    }

    // Non-array single value: coerce and return (op identity * coerced).
    let cow = crate::arena::arena_to_value_cow(av);
    if let Some(i) = try_coerce_to_integer(&cow, engine) {
        return match op.combine_int(op.identity_int(), i) {
            Some(r) => Ok(arena_number(arena, NumberValue::from_i64(r))),
            None => Ok(arena_number(
                arena,
                NumberValue::from_f64(op.combine_f(op.identity_int() as f64, i as f64)),
            )),
        };
    }
    if let Some(f) = coerce_to_number(&cow, engine) {
        return Ok(arena_number(
            arena,
            NumberValue::from_f64(op.combine_f(op.identity_int() as f64, f)),
        ));
    }
    match handle_nan(engine)? {
        NanAction::Skip => Ok(arena_number(
            arena,
            NumberValue::from_i64(op.identity_int()),
        )),
        NanAction::ReturnNull => Ok(crate::arena::pool::singleton_null()),
    }
}

/// Native arena datetime/duration subtract.
/// - DateTime - DateTime → Duration string.
/// - DateTime - Duration → DateTime ISO string.
/// - Duration - Duration → Duration string.
///
/// Returns `None` when neither operand is a datetime/duration form.
#[cfg(feature = "datetime")]
#[inline]
fn arena_datetime_subtract<'a>(
    a_av: &'a ArenaValue<'a>,
    b_av: &'a ArenaValue<'a>,
    arena: &'a Bump,
) -> Option<&'a ArenaValue<'a>> {
    use crate::operators::helpers::{extract_datetime_value, extract_duration_value};

    let a = crate::arena::arena_to_value_cow(a_av);
    let b = crate::arena::arena_to_value_cow(b_av);

    let a_dt = extract_datetime_value(a.as_ref());
    let a_dur = if a_dt.is_none() {
        extract_duration_value(a.as_ref())
    } else {
        None
    };
    let b_dt = extract_datetime_value(b.as_ref());
    let b_dur = if b_dt.is_none() {
        extract_duration_value(b.as_ref())
    } else {
        None
    };

    if let (Some(d1), Some(d2)) = (&a_dt, &b_dt) {
        let s = arena.alloc_str(&d1.diff(d2).to_string());
        return Some(arena.alloc(ArenaValue::String(s)));
    }
    if let (Some(d), Some(dur)) = (&a_dt, &b_dur) {
        let s = arena.alloc_str(&d.sub_duration(dur).to_iso_string());
        return Some(arena.alloc(ArenaValue::String(s)));
    }
    if let (Some(d1), Some(d2)) = (&a_dur, &b_dur) {
        let s = arena.alloc_str(&d1.sub(d2).to_string());
        return Some(arena.alloc(ArenaValue::String(s)));
    }
    None
}

/// Native arena datetime/duration add.
/// - DateTime + Duration → DateTime ISO string.
/// - Duration + Duration → Duration string.
///
/// Returns `None` when neither operand is a datetime/duration form.
#[cfg(feature = "datetime")]
#[inline]
fn arena_datetime_add<'a>(
    a_av: &'a ArenaValue<'a>,
    b_av: &'a ArenaValue<'a>,
    arena: &'a Bump,
) -> Option<&'a ArenaValue<'a>> {
    use crate::operators::helpers::{extract_datetime_value, extract_duration_value};

    let a = crate::arena::arena_to_value_cow(a_av);
    let b = crate::arena::arena_to_value_cow(b_av);

    let a_dt = extract_datetime_value(a.as_ref());
    let a_dur = if a_dt.is_none() {
        extract_duration_value(a.as_ref())
    } else {
        None
    };
    let b_dur = extract_duration_value(b.as_ref());

    if let (Some(dt), Some(dur)) = (&a_dt, &b_dur) {
        let s = arena.alloc_str(&dt.add_duration(dur).to_iso_string());
        return Some(arena.alloc(ArenaValue::String(s)));
    }
    if let (Some(d1), Some(d2)) = (&a_dur, &b_dur) {
        let s = arena.alloc_str(&d1.add(d2).to_string());
        return Some(arena.alloc(ArenaValue::String(s)));
    }
    None
}

/// Native arena duration/scalar multiply.
/// - Duration * scalar → Duration string.
/// - scalar * Duration → Duration string.
///
/// Returns `None` when neither operand is a duration paired with a number.
#[cfg(feature = "datetime")]
#[inline]
fn arena_datetime_multiply<'a>(
    a_av: &'a ArenaValue<'a>,
    b_av: &'a ArenaValue<'a>,
    arena: &'a Bump,
) -> Option<&'a ArenaValue<'a>> {
    use crate::operators::helpers::extract_duration_value;

    let a = crate::arena::arena_to_value_cow(a_av);
    let b = crate::arena::arena_to_value_cow(b_av);

    let a_dur = extract_duration_value(a.as_ref());
    let b_dur = extract_duration_value(b.as_ref());

    if let (Some(dur), None) = (&a_dur, &b_dur)
        && let Some(factor) = coerce_arena_to_number(b_av)
    {
        let s = arena.alloc_str(&dur.multiply(factor).to_string());
        return Some(arena.alloc(ArenaValue::String(s)));
    }
    if let (None, Some(dur)) = (&a_dur, &b_dur)
        && let Some(factor) = coerce_arena_to_number(a_av)
    {
        let s = arena.alloc_str(&dur.multiply(factor).to_string());
        return Some(arena.alloc(ArenaValue::String(s)));
    }
    None
}

/// Native arena `Duration / Number`. Mirrors the value-mode branch in
/// `evaluate_divide` (line ~745). Returns `None` for non-duration LHS so
/// the generic numeric path handles regular division.
#[cfg(feature = "datetime")]
#[inline]
fn arena_datetime_divide<'a>(
    a_av: &'a ArenaValue<'a>,
    b_av: &'a ArenaValue<'a>,
    arena: &'a Bump,
) -> Option<crate::Result<&'a ArenaValue<'a>>> {
    use crate::operators::helpers::extract_duration_value;

    let a = crate::arena::arena_to_value_cow(a_av);
    let a_dur = extract_duration_value(a.as_ref())?;
    let divisor = coerce_arena_to_number(b_av).or_else(|| {
        let b = crate::arena::arena_to_value_cow(b_av);
        b.as_f64()
    })?;
    if divisor == 0.0 {
        return Some(Err(crate::constants::nan_error()));
    }
    let s = arena.alloc_str(&a_dur.divide(divisor).to_string());
    Some(Ok(arena.alloc(ArenaValue::String(s))))
}

#[inline]
pub(crate) fn evaluate_subtract_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    // 1-arg subtract: array → fold (first - second - ...); else negate.
    if args.len() == 1 {
        let av = engine.evaluate_arena_node(&args[0], actx, arena)?;
        // Array fold case.
        let array_cow: Option<std::borrow::Cow<'_, [Value]>> = match av {
            ArenaValue::InputRef(Value::Array(arr)) => {
                Some(std::borrow::Cow::Borrowed(arr.as_slice()))
            }
            ArenaValue::Array(items) => Some(std::borrow::Cow::Owned(
                items
                    .iter()
                    .map(crate::arena::arena_to_value)
                    .collect::<Vec<_>>(),
            )),
            _ => None,
        };
        if let Some(arr) = array_cow {
            if arr.is_empty() {
                return Err(Error::InvalidArguments(INVALID_ARGS.into()));
            }
            let mut result =
                coerce_to_number(&arr[0], engine).ok_or_else(crate::constants::nan_error)?;
            for elem in &arr[1..] {
                let n = coerce_to_number(elem, engine).ok_or_else(crate::constants::nan_error)?;
                result -= n;
            }
            return Ok(arena_number(arena, NumberValue::from_f64(result)));
        }
        // Negate single value (preserve integer typing when possible).
        if let Some(i) = av.as_i64() {
            return Ok(arena_number(
                arena,
                i.checked_neg()
                    .map(NumberValue::from_i64)
                    .unwrap_or_else(|| NumberValue::from_f64(-(i as f64))),
            ));
        }
        let cow = crate::arena::arena_to_value_cow(av);
        if let Some(f) = coerce_to_number(&cow, engine) {
            return Ok(arena_number(arena, NumberValue::from_f64(-f)));
        }
        return Err(crate::constants::nan_error());
    }
    if args.len() == 2 {
        let a_av = engine.evaluate_arena_node(&args[0], actx, arena)?;
        let b_av = engine.evaluate_arena_node(&args[1], actx, arena)?;

        // Integer-preserving fast path.
        if let (Some(ia), Some(ib)) = (a_av.as_i64(), b_av.as_i64()) {
            return match ia.checked_sub(ib) {
                Some(d) => Ok(arena_number(arena, NumberValue::from_i64(d))),
                None => Ok(arena_number(
                    arena,
                    NumberValue::from_f64(ia as f64 - ib as f64),
                )),
            };
        }

        // Config-aware coercion path (covers bool/null/string operands).
        let a_cow = crate::arena::arena_to_value_cow(a_av);
        let b_cow = crate::arena::arena_to_value_cow(b_av);
        if let (Some(i1), Some(i2)) = (
            try_coerce_to_integer(&a_cow, engine),
            try_coerce_to_integer(&b_cow, engine),
        ) {
            return match i1.checked_sub(i2) {
                Some(d) => Ok(arena_number(arena, NumberValue::from_i64(d))),
                None => Ok(arena_number(
                    arena,
                    NumberValue::from_f64(i1 as f64 - i2 as f64),
                )),
            };
        }
        if let (Some(f1), Some(f2)) = (
            coerce_to_number(&a_cow, engine),
            coerce_to_number(&b_cow, engine),
        ) {
            return Ok(arena_number(arena, NumberValue::from_f64(f1 - f2)));
        }

        // Datetime / duration arithmetic.
        #[cfg(feature = "datetime")]
        {
            if let Some(av) = arena_datetime_subtract(a_av, b_av, arena) {
                return Ok(av);
            }
        }

        // Non-numeric, non-datetime — NaN.
        return Err(crate::constants::nan_error());
    }

    // Variadic subtractive fold: first - second - third - ...
    // Native port mirrors value-mode evaluate_subtract for the >2 case
    // (see arithmetic.rs:430-500). Integer fast path with overflow promotion.
    if args.is_empty() {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }
    let first_av = engine.evaluate_arena_node(&args[0], actx, arena)?;
    let first_cow = crate::arena::arena_to_value_cow(first_av);
    let mut all_int =
        first_av.as_i64().is_some() || try_coerce_to_integer(&first_cow, engine).is_some();
    let mut int_acc: i64 = first_av
        .as_i64()
        .or_else(|| try_coerce_to_integer(&first_cow, engine))
        .unwrap_or_default();
    let mut float_acc: f64 = match coerce_to_number(&first_cow, engine) {
        Some(f) => f,
        None => return Err(crate::constants::nan_error()),
    };

    for arg in args.iter().skip(1) {
        let av = engine.evaluate_arena_node(arg, actx, arena)?;
        let cow = crate::arena::arena_to_value_cow(av);
        if all_int && let Some(i) = av.as_i64().or_else(|| try_coerce_to_integer(&cow, engine)) {
            match int_acc.checked_sub(i) {
                Some(r) => int_acc = r,
                None => {
                    all_int = false;
                    float_acc = int_acc as f64 - i as f64;
                }
            }
            continue;
        }
        if let Some(f) = coerce_to_number(&cow, engine) {
            if all_int {
                all_int = false;
                float_acc = int_acc as f64 - f;
            } else {
                float_acc -= f;
            }
        } else {
            match handle_nan(engine)? {
                NanAction::Skip => continue,
                NanAction::ReturnNull => return Ok(crate::arena::pool::singleton_null()),
            }
        }
    }

    if all_int {
        Ok(arena_number(arena, NumberValue::from_i64(int_acc)))
    } else {
        Ok(arena_number(arena, NumberValue::from_f64(float_acc)))
    }
}

/// Native arena-mode `/`. Handles 1-arg array (sequential divide), 1-arg
/// scalar (1/x), 2-arg, and divbyzero per engine config.
#[inline]
pub(crate) fn evaluate_divide_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    arena_div_or_mod(args, actx, engine, arena, |a, b| a.div(b), false)
}

/// Native arena-mode `%` (modulo).
#[inline]
pub(crate) fn evaluate_modulo_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    arena_div_or_mod(args, actx, engine, arena, |a, b| a.rem(b), true)
}

#[inline]
fn arena_div_or_mod<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
    op: fn(&NumberValue, &NumberValue) -> Option<NumberValue>,
    is_modulo: bool,
) -> Result<&'a ArenaValue<'a>> {
    if args.is_empty() {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }
    if args.len() == 1 {
        return arena_one_arg_div_mod(&args[0], actx, engine, arena, is_modulo);
    }
    if args.len() > 2 {
        return arena_variadic_div_mod(args, actx, engine, arena, is_modulo);
    }
    let a_av = engine.evaluate_arena_node(&args[0], actx, arena)?;
    let b_av = engine.evaluate_arena_node(&args[1], actx, arena)?;

    // Duration / Number — only for `/`, not `%` (modulo on durations isn't
    // a value-mode op either).
    #[cfg(feature = "datetime")]
    if !is_modulo && let Some(r) = arena_datetime_divide(a_av, b_av, arena) {
        return r;
    }

    // Config-aware coercion via Cow (free for InputRef).
    let a_cow = crate::arena::arena_to_value_cow(a_av);
    let b_cow = crate::arena::arena_to_value_cow(b_av);
    let af = match coerce_to_number(&a_cow, engine) {
        Some(f) => f,
        None => return Err(crate::constants::nan_error()),
    };
    let bf = match coerce_to_number(&b_cow, engine) {
        Some(f) => f,
        None => return Err(crate::constants::nan_error()),
    };
    let na = NumberValue::from_f64(af);
    let nb = NumberValue::from_f64(bf);
    if nb.is_zero() {
        // Match value-mode: integer/integer with divisor=0 errors regardless
        // of `division_by_zero` config (config only governs the float path).
        if a_av.as_i64().is_some() && b_av.as_i64().is_some() {
            return Err(crate::constants::nan_error());
        }
        return divbyzero_arena(arena, na.as_f64(), engine);
    }
    match op(&na, &nb) {
        Some(r) => Ok(arena_number(arena, r)),
        None => Err(crate::constants::nan_error()),
    }
}

#[inline]
fn divbyzero_arena<'a>(
    arena: &'a Bump,
    dividend: f64,
    engine: &DataLogic,
) -> Result<&'a ArenaValue<'a>> {
    use crate::config::DivisionByZeroHandling;
    match engine.config().division_by_zero {
        DivisionByZeroHandling::ThrowError => Err(crate::constants::nan_error()),
        DivisionByZeroHandling::ReturnNull => Ok(crate::arena::pool::singleton_null()),
        DivisionByZeroHandling::ReturnInfinity => {
            let v = if dividend >= 0.0 {
                f64::INFINITY
            } else {
                f64::NEG_INFINITY
            };
            Ok(arena_number(arena, NumberValue::from_f64(v)))
        }
        DivisionByZeroHandling::ReturnBounds => {
            let v = if dividend > 0.0 {
                f64::MAX
            } else if dividend < 0.0 {
                f64::MIN
            } else {
                0.0
            };
            Ok(arena_number(arena, NumberValue::from_f64(v)))
        }
    }
}

/// Native arena 1-arg `/` / `%`. Mirrors value-mode evaluate_divide / evaluate_modulo
/// 1-arg semantics:
///   * `/` with array → fold (a/b/c). `/` with non-array → 1/x.
///   * `%` with array of ≥2 numeric elements → fold (a%b%c). `%` with single
///     non-array argument → InvalidArguments (matches value-mode).
fn arena_one_arg_div_mod<'a>(
    arg: &CompiledNode,
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
    is_modulo: bool,
) -> Result<&'a ArenaValue<'a>> {
    let av = engine.evaluate_arena_node(arg, actx, arena)?;

    let array_cow: Option<std::borrow::Cow<'_, [Value]>> = match av {
        ArenaValue::InputRef(Value::Array(arr)) => Some(std::borrow::Cow::Borrowed(arr.as_slice())),
        ArenaValue::Array(items) => Some(std::borrow::Cow::Owned(
            items
                .iter()
                .map(crate::arena::arena_to_value)
                .collect::<Vec<_>>(),
        )),
        _ => None,
    };
    if let Some(arr) = array_cow {
        // Modulo requires ≥2 elements; divide tolerates 1+ (1-elem returns first).
        if arr.is_empty() || (is_modulo && arr.len() < 2) {
            return Err(Error::InvalidArguments(INVALID_ARGS.into()));
        }
        let mut result =
            coerce_to_number(&arr[0], engine).ok_or_else(crate::constants::nan_error)?;
        for elem in &arr[1..] {
            let n = coerce_to_number(elem, engine).ok_or_else(crate::constants::nan_error)?;
            if n == 0.0 {
                return Err(crate::constants::nan_error());
            }
            result = if is_modulo { result % n } else { result / n };
        }
        return Ok(arena_number(arena, NumberValue::from_f64(result)));
    }

    // Non-array single value.
    if is_modulo {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }
    // 1/x with integer-preserving fast path.
    if let Some(i) = av.as_i64() {
        if i == 0 {
            return Err(crate::constants::nan_error());
        }
        if i == -1 {
            return Ok(arena_number(arena, NumberValue::from_i64(-1)));
        }
        if 1 % i == 0 {
            return Ok(arena_number(arena, NumberValue::from_i64(1 / i)));
        }
        return Ok(arena_number(arena, NumberValue::from_f64(1.0 / i as f64)));
    }
    let cow = crate::arena::arena_to_value_cow(av);
    let f = coerce_to_number(&cow, engine).ok_or_else(crate::constants::nan_error)?;
    if f == 0.0 {
        return Err(crate::constants::nan_error());
    }
    Ok(arena_number(arena, NumberValue::from_f64(1.0 / f)))
}

/// Native arena variadic (≥3 args) `/` / `%`. Folds left-associatively with
/// per-step zero-divisor check.
fn arena_variadic_div_mod<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
    is_modulo: bool,
) -> Result<&'a ArenaValue<'a>> {
    let first_av = engine.evaluate_arena_node(&args[0], actx, arena)?;
    let first_cow = crate::arena::arena_to_value_cow(first_av);
    let mut result =
        coerce_to_number(&first_cow, engine).ok_or_else(crate::constants::nan_error)?;
    for arg in args.iter().skip(1) {
        let av = engine.evaluate_arena_node(arg, actx, arena)?;
        let cow = crate::arena::arena_to_value_cow(av);
        let n = coerce_to_number(&cow, engine).ok_or_else(crate::constants::nan_error)?;
        if n == 0.0 {
            return Err(crate::constants::nan_error());
        }
        result = if is_modulo { result % n } else { result / n };
    }
    Ok(arena_number(arena, NumberValue::from_f64(result)))
}

/// `get_number_strict` for arena values — Number variants and string-as-number
/// only (no bool/null coercion).
#[cfg(feature = "ext-math")]
#[inline]
fn arena_value_strict_f64(av: &ArenaValue<'_>) -> Option<f64> {
    match av {
        ArenaValue::Number(n) => Some(n.as_f64()),
        ArenaValue::String(s) => s.parse().ok(),
        ArenaValue::InputRef(Value::Number(n)) => n.as_f64(),
        ArenaValue::InputRef(Value::String(s)) => s.parse().ok(),
        _ => None,
    }
}

/// Generic native unary math op shared by abs / ceil / floor.
/// - `args.is_empty()` → InvalidArguments
/// - 1 arg, numeric → apply op_fn, return arena Number
/// - 1 arg, non-numeric → InvalidArguments
/// - >1 args → variadic, return arena Array of results (any non-numeric → error)
#[cfg(feature = "ext-math")]
#[inline]
fn arena_unary_math<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
    op_fn: fn(f64) -> f64,
    always_int: bool,
) -> Result<&'a ArenaValue<'a>> {
    if args.is_empty() {
        return Err(Error::InvalidArguments(INVALID_ARGS.into()));
    }

    let to_arena = |x: f64, arena: &'a Bump| -> &'a ArenaValue<'a> {
        if always_int {
            arena_number(arena, NumberValue::from_i64(x as i64))
        } else {
            arena_number(arena, NumberValue::from_f64(x))
        }
    };

    if args.len() == 1 {
        let av = engine.evaluate_arena_node(&args[0], actx, arena)?;
        let n = arena_value_strict_f64(av)
            .ok_or_else(|| Error::InvalidArguments(INVALID_ARGS.into()))?;
        return Ok(to_arena(op_fn(n), arena));
    }

    let mut items: bumpalo::collections::Vec<'a, ArenaValue<'a>> =
        bumpalo::collections::Vec::with_capacity_in(args.len(), arena);
    for arg in args {
        let av = engine.evaluate_arena_node(arg, actx, arena)?;
        let n = arena_value_strict_f64(av)
            .ok_or_else(|| Error::InvalidArguments(INVALID_ARGS.into()))?;
        let r = to_arena(op_fn(n), arena);
        items.push(crate::arena::value::reborrow_arena_value(r));
    }
    Ok(arena.alloc(ArenaValue::Array(items.into_bump_slice())))
}

#[cfg(feature = "ext-math")]
#[inline]
pub(crate) fn evaluate_abs_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    arena_unary_math(args, actx, engine, arena, f64::abs, false)
}

#[cfg(feature = "ext-math")]
#[inline]
pub(crate) fn evaluate_ceil_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    arena_unary_math(args, actx, engine, arena, f64::ceil, true)
}

#[cfg(feature = "ext-math")]
#[inline]
pub(crate) fn evaluate_floor_arena<'a>(
    args: &[CompiledNode],
    actx: &mut ArenaContextStack<'a>,
    engine: &DataLogic,
    arena: &'a Bump,
) -> Result<&'a ArenaValue<'a>> {
    arena_unary_math(args, actx, engine, arena, f64::floor, true)
}
