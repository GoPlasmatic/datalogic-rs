//! Tensor operators: marshalling between JSON and datavalue's `Tensor`.
//!
//! JSON has no tensor. Anything that feeds a model — an ONNX Runtime
//! session, an edge request encoder, an RL observation buffer — needs one
//! value that is neither a scalar nor a JSON array, and needs to get its
//! outputs back out again. datavalue carries that value
//! ([`datavalue::DataTensor`]: a dtype, a shape, and one row-major
//! contiguous native-endian byte buffer). This module is the engine's
//! half: the operators that make, reshape, and read one back.
//!
//! # What is deliberately absent
//!
//! There is no arithmetic here. Every operator's cost is proportional to
//! the data it moves, which is what lets [`charge`] price it honestly. An
//! operator whose work is *not* proportional to its data (a matmul reads
//! 2n² elements and does n³ multiplies) would be under-priced by an
//! unbounded ratio, and a budget built on those numbers would stop
//! measuring anything. If compute is wanted later it belongs in a separate
//! `tensor-math` feature with a FLOP-proportional rule.
//!
//! That line is also what keeps the family small: 20 operators that move
//! bytes, and none that interpret them as a computation.
//!
//! # Operators
//!
//! | Operator | Signature | Charge |
//! |---|---|---|
//! | `tensor` | `(value, dtype?)` | numel |
//! | `zeros` | `(shape, dtype)` | numel |
//! | `full` | `(shape, dtype, value)` | numel |
//! | `scatter` | `(points, shape, dtype, value?)` | max(points, numel) |
//! | `rle_expand` | `(runs, shape, dtype)` | max(runs, numel) |
//! | `one_hot` | `(indices, depth, dtype)` | len × depth |
//! | `stack` | `(tensors, axis)` | Σ numel |
//! | `concat` | `(tensors, axis)` | Σ numel |
//! | `unstack` | `(T, axis)` | numel |
//! | `reshape` | `(T, shape)` | 1 |
//! | `transpose` | `(T, perm?)` | numel |
//! | `pad` | `(T, before, after, value?)` | max(in, out) |
//! | `crop` | `(T, offset, shape)` | max(in, out) |
//! | `cast` | `(T, dtype)` | numel |
//! | `normalize` | `(T, mean, scale?)` | numel |
//! | `argmax` | `(T, axis)` | numel |
//! | `gather` | `(T, indices, axis?)` | max(in, out) |
//! | `to_list` | `(T)` | numel |
//! | `shape` / `dtype` | `(T)` | 1 |
//!
//! # dtype coverage
//!
//! The byte-moving operators (`stack`, `concat`, `unstack`, `reshape`,
//! `transpose`, `pad`, `crop`, `gather`) move `DType::size_of()`-byte
//! cells and never interpret an element, so they work on every dtype,
//! `f16` / `bf16` included, with no `half` dependency. The element
//! operators go through [`Scalar`] and answer
//! [`datavalue::TensorError::UnsupportedDType`] on `f16` / `bf16` unless
//! the `tensor-half` feature is on.

use crate::arena::{ContextStack, DataValue};
use crate::{CompiledNode, Engine, Error, Result};
use bumpalo::Bump;
use datavalue::{DType, DataTensor, TensorError};

mod construct;
mod read;
mod shape_ops;

pub(crate) use construct::{
    evaluate_full, evaluate_one_hot, evaluate_rle_expand, evaluate_scatter, evaluate_tensor,
    evaluate_zeros,
};
pub(crate) use read::{
    evaluate_argmax, evaluate_cast, evaluate_dtype, evaluate_normalize, evaluate_shape,
    evaluate_to_list,
};
pub(crate) use shape_ops::{
    evaluate_concat, evaluate_crop, evaluate_gather, evaluate_pad, evaluate_reshape,
    evaluate_stack, evaluate_transpose, evaluate_unstack,
};

// ---------------------------------------------------------------------------
// Cost
// ---------------------------------------------------------------------------

/// Price this operator's work, in elements.
///
/// Every operator calls this with `max(elements read, elements produced)`
/// **before** it allocates, so a rule that would produce a billion
/// elements is refused rather than run and then reported. The dispatcher
/// separately charges 1 for the node itself.
///
/// The counter it charges against is the `budget` feature; without that
/// feature the call compiles to `Ok(())` and the call sites are simply
/// documentation of where the cost is.
#[inline]
fn charge(ctx: &mut ContextStack<'_>, elements: u64) -> Result<()> {
    ctx.charge(elements)
}

/// `max(a, b)` as a charge, saturating into the counter's `u64`.
#[inline]
fn cost(a: usize, b: usize) -> u64 {
    a.max(b) as u64
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Wrap a `TensorError`, preserving it in the source chain so a caller can
/// downcast to the typed error rather than parse a message.
#[inline]
fn wrap(e: TensorError) -> Error {
    Error::wrap(e)
}

#[inline]
fn bad(msg: &'static str) -> Error {
    Error::invalid_arguments(msg)
}

/// The same error datavalue raises when a nested leaf will not fit its
/// dtype, so every decode path reports an unrepresentable element the
/// same way.
fn element_error(index: usize, expected: DType) -> Error {
    wrap(TensorError::Element { index, expected })
}

// ---------------------------------------------------------------------------
// Argument plumbing
// ---------------------------------------------------------------------------

/// Evaluate argument `i`. Every operator in this family takes a fixed
/// positional argument list, so a missing one is an argument error rather
/// than an implicit null.
#[inline]
fn arg<'a>(
    args: &'a [CompiledNode],
    i: usize,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    let node = args.get(i).ok_or_else(|| bad("missing argument"))?;
    engine.dispatch_node(node, ctx, arena)
}

/// Evaluate optional argument `i`, if it was supplied.
#[inline]
fn opt_arg<'a>(
    args: &'a [CompiledNode],
    i: usize,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<Option<&'a DataValue<'a>>> {
    match args.get(i) {
        None => Ok(None),
        Some(node) => engine.dispatch_node(node, ctx, arena).map(Some),
    }
}

/// Reject a call with more arguments than the operator reads, so a typo in
/// a rule surfaces at evaluation instead of being silently ignored.
#[inline]
fn at_most(args: &[CompiledNode], n: usize) -> Result<()> {
    if args.len() > n {
        return Err(bad("too many arguments"));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Reading the common argument shapes
// ---------------------------------------------------------------------------

/// A tensor-valued argument: an actual `Tensor`, or the tagged
/// `{"tensor": {...}}` wire form that arrived as plain JSON (the parser
/// never produces a `Tensor`, so data read from the input boundary always
/// takes this path).
fn as_tensor<'a>(v: &DataValue<'a>, arena: &'a Bump) -> Result<DataTensor<'a>> {
    match v {
        DataValue::Tensor(t) => Ok(**t),
        DataValue::Object(_) => DataTensor::from_json_value_in(v, arena).map_err(wrap),
        _ => Err(bad("expected a tensor")),
    }
}

/// A dtype-valued argument: the wire name, case-insensitively
/// (`"f32"`, `"BF16"`, `"bool"`, ...).
fn as_dtype(v: &DataValue<'_>) -> Result<DType> {
    let DataValue::String(s) = v else {
        return Err(bad("dtype must be a string"));
    };
    DType::from_name(s).ok_or_else(|| wrap(TensorError::UnknownDType((*s).to_string())))
}

/// A shape-valued argument: an array of non-negative whole numbers. A bare
/// number is accepted as a 1-d shape, and an empty array is a 0-d (scalar)
/// tensor.
fn as_shape<'a>(v: &DataValue<'_>, arena: &'a Bump) -> Result<&'a [usize]> {
    match v {
        DataValue::Number(_) => Ok(arena.alloc_slice_copy(&[as_usize(v)?])),
        DataValue::Array(items) => {
            let mut out = crate::arena::bvec::<usize>(arena, items.len());
            for it in *items {
                out.push(as_usize(it)?);
            }
            Ok(out.into_bump_slice())
        }
        _ => Err(bad("shape must be an array of non-negative integers")),
    }
}

/// A list of signed whole numbers (offsets, paddings, indices).
fn as_i64_list<'a>(v: &DataValue<'_>, arena: &'a Bump) -> Result<&'a [i64]> {
    let DataValue::Array(items) = v else {
        return Err(bad("expected an array of integers"));
    };
    let mut out = crate::arena::bvec::<i64>(arena, items.len());
    for it in *items {
        out.push(as_i64(it)?);
    }
    Ok(out.into_bump_slice())
}

/// One non-negative whole number.
fn as_usize(v: &DataValue<'_>) -> Result<usize> {
    let n = as_i64(v)?;
    usize::try_from(n).map_err(|_| bad("expected a non-negative integer"))
}

/// One whole number. Floats are accepted when they are exactly integral,
/// because JSON has no integer type and a rule that computes an index with
/// `/` legitimately produces `2.0`.
fn as_i64(v: &DataValue<'_>) -> Result<i64> {
    match v {
        DataValue::Number(n) => {
            let f = n.as_f64();
            if f.fract() != 0.0 || !f.is_finite() {
                return Err(bad("expected a whole number"));
            }
            n.as_i64()
                .or_else(|| {
                    // `as_i64` declines on a float-typed number even when
                    // it is integral; the range check makes the cast safe.
                    (f >= -(2f64.powi(63)) && f < 2f64.powi(63)).then_some(f as i64)
                })
                .ok_or_else(|| bad("integer out of range"))
        }
        _ => Err(bad("expected a whole number")),
    }
}

/// One `f64`, for `normalize`'s mean and scale.
fn as_f64(v: &DataValue<'_>) -> Result<f64> {
    match v {
        DataValue::Number(n) => Ok(n.as_f64()),
        _ => Err(bad("expected a number")),
    }
}

/// Resolve a position in `0..extent`, accepting Python-style negative
/// indexing (`-1` is the last position). `None` when out of range either
/// way. Shared by every place this family names an axis or an index.
#[inline]
fn resolve_index(i: i64, extent: usize) -> Option<usize> {
    let resolved = if i < 0 { i + extent as i64 } else { i };
    usize::try_from(resolved).ok().filter(|r| *r < extent)
}

/// Resolve an axis argument against a rank. `extra` is 1 for `stack`,
/// where the axis names a position in the *output* rank. A 0-d tensor
/// still accepts axis 0 so the rank-0 checks each operator does can
/// produce their own, more specific error.
fn as_axis(v: &DataValue<'_>, rank: usize, extra: usize) -> Result<usize> {
    resolve_index(as_i64(v)?, (rank + extra).max(1)).ok_or_else(|| bad("axis out of range"))
}

// ---------------------------------------------------------------------------
// Shape arithmetic
// ---------------------------------------------------------------------------

/// Element count of a shape, or `ShapeOverflow`. datavalue re-checks this
/// in every constructor; we need it up front to charge before allocating.
fn numel_of(shape: &[usize]) -> Result<usize> {
    shape
        .iter()
        .try_fold(1usize, |acc, d| acc.checked_mul(*d))
        .ok_or_else(|| wrap(TensorError::ShapeOverflow))
}

/// Row-major strides in *elements*, outermost first. `strides[i]` is how
/// far one step along axis `i` moves.
fn strides_of<'a>(shape: &[usize], arena: &'a Bump) -> &'a [usize] {
    let mut s = crate::arena::bvec::<usize>(arena, shape.len());
    s.resize(shape.len(), 1);
    for i in (0..shape.len().saturating_sub(1)).rev() {
        s[i] = s[i + 1] * shape[i + 1];
    }
    s.into_bump_slice()
}

/// Step a row-major counter `idx` (one entry per axis) and report whether
/// it wrapped past the end.
#[inline]
fn advance(idx: &mut [usize], shape: &[usize]) -> bool {
    for ax in (0..shape.len()).rev() {
        idx[ax] += 1;
        if idx[ax] < shape[ax] {
            return true;
        }
        idx[ax] = 0;
    }
    false
}

/// Split a shape around `axis` into `(outer, extent, inner)`: the element
/// counts before the axis, along it, and after it. Every operator that
/// walks one axis of a row-major buffer needs exactly these three numbers.
#[inline]
fn split_axis(shape: &[usize], axis: usize) -> (usize, usize, usize) {
    (
        shape[..axis].iter().product(),
        shape[axis],
        shape[axis + 1..].iter().product(),
    )
}

/// `shape` with `axis` removed — the result shape of `unstack` and `argmax`.
fn shape_without_axis<'a>(shape: &[usize], axis: usize, arena: &'a Bump) -> &'a [usize] {
    let mut out = crate::arena::bvec::<usize>(arena, shape.len() - 1);
    out.extend_from_slice(&shape[..axis]);
    out.extend_from_slice(&shape[axis + 1..]);
    out.into_bump_slice()
}

// ---------------------------------------------------------------------------
// Producing a result
// ---------------------------------------------------------------------------

/// Wrap a finished tensor as an arena value. One bump for the 40-byte
/// header; the payload is already in the arena.
#[inline]
fn finish<'a>(t: DataTensor<'a>, arena: &'a Bump) -> Result<&'a DataValue<'a>> {
    Ok(arena.alloc(DataValue::tensor_in(t, arena)))
}

/// [`finish`] over a byte buffer the operator filled itself — the
/// byte-moving operators' exit. `bytes` must already be aligned for
/// `dtype`, which everything `DataTensor::zeroed_bytes_in` hands out is.
#[inline]
fn finish_bytes<'a>(
    dtype: DType,
    shape: &'a [usize],
    bytes: &'a [u8],
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    finish(
        DataTensor::from_bytes(dtype, shape, bytes).map_err(wrap)?,
        arena,
    )
}

/// [`finish`] over an element vector the operator built — the element-wise
/// operators' exit. The vector's own allocation becomes the payload, no
/// copy.
#[inline]
fn finish_slice<'a, T: datavalue::Element>(
    shape: &'a [usize],
    data: bumpalo::collections::Vec<'a, T>,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    finish(
        DataTensor::from_slice(shape, data.into_bump_slice()).map_err(wrap)?,
        arena,
    )
}

// ---------------------------------------------------------------------------
// Elements
// ---------------------------------------------------------------------------

/// The element behaviour the non-byte-moving operators need on top of
/// [`datavalue::Element`]: read one out of a JSON value, read one back as
/// `f64`, and narrow an `f64` into the dtype.
///
/// Sealed in practice by [`datavalue::Element`] being sealed.
pub(crate) trait Scalar: datavalue::Element + PartialOrd {
    /// Zero, for the fill paths.
    const ZERO: Self;
    /// One, for `scatter`'s and `one_hot`'s default written value.
    const ONE: Self;

    /// Exact conversion from a JSON value. `None` when the value is the
    /// wrong JSON type, or a number this dtype cannot hold without
    /// changing it. Never truncates and never manufactures an infinity —
    /// the same contract datavalue's nested decode follows.
    fn from_value(v: &DataValue<'_>) -> Option<Self>;

    /// Saturating conversion, clamping to this dtype's range. Used only by
    /// `cast`, which is defined as a lossy narrowing.
    fn from_f64_saturating(v: f64) -> Self;

    /// Widen for `cast` and `normalize`. Lossy above 2^53 for the 64-bit
    /// integer dtypes; `argmax` therefore compares elements directly
    /// (`Scalar: PartialOrd`) rather than through this.
    fn to_f64(self) -> f64;
}

/// Implement [`Scalar`] for an integer dtype. The exact path goes through
/// `i128`, which holds every `i64` and `u64` without loss, so the range
/// check is a plain comparison rather than a float dance.
macro_rules! int_scalar {
    ($($t:ty),* $(,)?) => {$(
        impl Scalar for $t {
            const ZERO: Self = 0;
            const ONE: Self = 1;

            fn from_value(v: &DataValue<'_>) -> Option<Self> {
                let DataValue::Number(n) = v else { return None };
                if let Some(i) = n.as_i64() {
                    return Self::try_from(i).ok();
                }
                let f = n.as_f64();
                if f.fract() != 0.0 || !f.is_finite() {
                    return None;
                }
                // `u64` values above `i64::MAX` arrive as floats; the
                // i128 window covers both signed and unsigned 64-bit.
                let as_int = f as i128;
                (as_int as f64 == f).then(|| Self::try_from(as_int).ok())?
            }

            fn from_f64_saturating(v: f64) -> Self {
                // Rust's float->int `as` cast is already saturating and
                // maps NaN to 0, which is the behaviour we document.
                v as Self
            }

            fn to_f64(self) -> f64 {
                self as f64
            }
        }
    )*};
}

int_scalar!(i8, u8, i16, u16, i32, u32, i64, u64);

impl Scalar for f32 {
    const ZERO: Self = 0.0;
    const ONE: Self = 1.0;

    fn from_value(v: &DataValue<'_>) -> Option<Self> {
        let DataValue::Number(n) = v else { return None };
        let f = n.as_f64();
        let narrowed = f as f32;
        // Reject only a *silent* overflow to infinity. An explicit
        // infinity in the input passes through, and ordinary precision
        // loss (0.1 -> 0.100000001) is inherent to the dtype.
        (narrowed.is_finite() || !f.is_finite()).then_some(narrowed)
    }

    fn from_f64_saturating(v: f64) -> Self {
        v as f32
    }

    fn to_f64(self) -> f64 {
        self as f64
    }
}

impl Scalar for f64 {
    const ZERO: Self = 0.0;
    const ONE: Self = 1.0;

    fn from_value(v: &DataValue<'_>) -> Option<Self> {
        match v {
            DataValue::Number(n) => Some(n.as_f64()),
            _ => None,
        }
    }

    fn from_f64_saturating(v: f64) -> Self {
        v
    }

    fn to_f64(self) -> f64 {
        self
    }
}

impl Scalar for bool {
    const ZERO: Self = false;
    const ONE: Self = true;

    fn from_value(v: &DataValue<'_>) -> Option<Self> {
        match v {
            DataValue::Bool(b) => Some(*b),
            // 0 and 1 are the only numbers a bool tensor can hold; the
            // byte-level invariant datavalue enforces says the same.
            DataValue::Number(n) => {
                let f = n.as_f64();
                if f == 0.0 {
                    Some(false)
                } else if f == 1.0 {
                    Some(true)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn from_f64_saturating(v: f64) -> Self {
        v != 0.0 && !v.is_nan()
    }

    fn to_f64(self) -> f64 {
        if self { 1.0 } else { 0.0 }
    }
}

#[cfg(feature = "tensor-half")]
macro_rules! half_scalar {
    ($($t:ty),* $(,)?) => {$(
        impl Scalar for $t {
            const ZERO: Self = <$t>::from_f32_const(0.0);
            const ONE: Self = <$t>::from_f32_const(1.0);

            fn from_value(v: &DataValue<'_>) -> Option<Self> {
                let DataValue::Number(n) = v else { return None };
                let f = n.as_f64();
                let narrowed = <$t>::from_f64(f);
                (narrowed.is_finite() || !f.is_finite()).then_some(narrowed)
            }

            fn from_f64_saturating(v: f64) -> Self {
                <$t>::from_f64(v)
            }

            fn to_f64(self) -> f64 {
                <$t>::to_f64(self)
            }
        }
    )*};
}

#[cfg(feature = "tensor-half")]
half_scalar!(datavalue::half::f16, datavalue::half::bf16);

/// Call a generic function once, monomorphised for `$dtype`'s element
/// type. Every element-wise operator routes through this.
///
/// `DType` is `#[non_exhaustive]`, and `f16` / `bf16` have no `Element`
/// impl without `tensor-half`, so the catch-all arm carries both cases:
/// it answers `UnsupportedDType` rather than a panic.
macro_rules! by_dtype {
    ($dtype:expr, $f:ident $(, $arg:expr)* $(,)?) => {{
        // Fully-qualified throughout: this expands in three sibling
        // modules, and none of them should have to import a name just to
        // satisfy the macro.
        match $dtype {
            ::datavalue::DType::Bool => $f::<bool>($($arg),*),
            ::datavalue::DType::I8 => $f::<i8>($($arg),*),
            ::datavalue::DType::U8 => $f::<u8>($($arg),*),
            ::datavalue::DType::I16 => $f::<i16>($($arg),*),
            ::datavalue::DType::U16 => $f::<u16>($($arg),*),
            ::datavalue::DType::I32 => $f::<i32>($($arg),*),
            ::datavalue::DType::U32 => $f::<u32>($($arg),*),
            ::datavalue::DType::I64 => $f::<i64>($($arg),*),
            ::datavalue::DType::U64 => $f::<u64>($($arg),*),
            ::datavalue::DType::F32 => $f::<f32>($($arg),*),
            ::datavalue::DType::F64 => $f::<f64>($($arg),*),
            #[cfg(feature = "tensor-half")]
            ::datavalue::DType::F16 => $f::<::datavalue::half::f16>($($arg),*),
            #[cfg(feature = "tensor-half")]
            ::datavalue::DType::BF16 => $f::<::datavalue::half::bf16>($($arg),*),
            other => ::core::result::Result::Err($crate::operators::tensor::wrap(
                ::datavalue::TensorError::UnsupportedDType(other),
            )),
        }
    }};
}

pub(crate) use by_dtype;
