//! Reading operators: `cast`, `normalize`, `argmax`, `to_list`, `shape`,
//! `dtype`.
//!
//! These are the ones that interpret elements rather than move bytes, so
//! they go through [`Scalar`] and answer `UnsupportedDType` on `f16` /
//! `bf16` unless `tensor-half` is on. `shape` and `dtype` read only the
//! header and work on every dtype.

use super::{
    Scalar, arg, as_axis, as_dtype, as_f64, as_tensor, at_most, bad, by_dtype, charge, finish,
    opt_arg, wrap,
};
use crate::arena::{ContextStack, DataValue, bvec};
use crate::{CompiledNode, Engine, Result};
use bumpalo::Bump;
use datavalue::{DType, DataTensor, NumberValue};

/// Widen every element to `f64`, once, so the operators below can be
/// written against a single element type instead of a 13 × 13 matrix of
/// source/destination dtype pairs.
///
/// The buffer lives in the evaluation arena and is dropped with it.
/// Elements of the 64-bit integer dtypes above 2^53 lose their low bits
/// here; that is the documented limit of `cast`, `normalize` and `argmax`
/// on `i64` / `u64`.
fn widen<'a>(t: DataTensor<'_>, arena: &'a Bump) -> Result<&'a [f64]> {
    by_dtype!(t.dtype(), widen_impl, t, arena)
}

fn widen_impl<'a, T: Scalar>(t: DataTensor<'_>, arena: &'a Bump) -> Result<&'a [f64]> {
    let src = t
        .as_slice::<T>()
        .ok_or_else(|| bad("tensor: dtype does not match its payload"))?;
    let mut out = bvec::<f64>(arena, src.len());
    out.extend(src.iter().map(|v| v.to_f64()));
    Ok(out.into_bump_slice())
}

/// Narrow an `f64` buffer back into `dtype`, saturating.
fn narrow<'a>(
    dtype: DType,
    values: &[f64],
    shape: &'a [usize],
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    by_dtype!(dtype, narrow_impl, values, shape, arena)
}

fn narrow_impl<'a, T: Scalar>(
    values: &[f64],
    shape: &'a [usize],
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    let mut out = bvec::<T>(arena, values.len());
    out.extend(values.iter().map(|v| T::from_f64_saturating(*v)));
    finish(
        DataTensor::from_slice(shape, out.into_bump_slice()).map_err(wrap)?,
        arena,
    )
}

/// `cast: [T, dtype]` — the family's one cross-dtype conversion.
///
/// Narrowing saturates rather than wrapping (`300` cast to `u8` is `255`,
/// not `44`) and `NaN` becomes zero, matching Rust's own float-to-integer
/// cast. This is deliberately lossy — it is the operator you reach for
/// when you know the model wants `f32` and the JSON gave you `f64`.
pub(crate) fn evaluate_cast<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    at_most(args, 2)?;
    let t = as_tensor(arg(args, 0, ctx, engine, arena)?, arena)?;
    let dtype = as_dtype(arg(args, 1, ctx, engine, arena)?)?;
    charge(ctx, t.numel() as u64)?;

    // Casting to the dtype it already has costs nothing and, importantly,
    // does not round-trip 64-bit integers through `f64`.
    if dtype == t.dtype() {
        return finish(t, arena);
    }
    let values = widen(t, arena)?;
    narrow(dtype, values, t.shape(), arena)
}

/// `normalize: [T, mean, scale?]` — `(x − mean) × scale`, always to `f32`.
///
/// The output dtype is fixed because that is what the operation is for:
/// turning integer sensor or pixel data into the float range a model
/// expects. `scale` defaults to 1, so the two-argument form is a plain
/// mean subtraction. `mean` and `scale` are scalars; per-channel
/// normalization is `unstack` + `normalize` + `stack`.
pub(crate) fn evaluate_normalize<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    at_most(args, 3)?;
    let t = as_tensor(arg(args, 0, ctx, engine, arena)?, arena)?;
    let mean = as_f64(arg(args, 1, ctx, engine, arena)?)?;
    let scale = match opt_arg(args, 2, ctx, engine, arena)? {
        Some(v) => as_f64(v)?,
        None => 1.0,
    };
    charge(ctx, t.numel() as u64)?;

    let values = widen(t, arena)?;
    let mut out = bvec::<f32>(arena, values.len());
    out.extend(values.iter().map(|v| ((v - mean) * scale) as f32));
    finish(
        DataTensor::from_slice(t.shape(), out.into_bump_slice()).map_err(wrap)?,
        arena,
    )
}

/// `argmax: [T, axis]` — index of the largest element along `axis`,
/// returned as plain JSON (nested arrays, or a bare number when the input
/// is 1-d), because an index is something a rule goes on to compare and
/// branch on.
///
/// Ties go to the first occurrence. `NaN` never wins, so an all-`NaN`
/// lane reports 0.
pub(crate) fn evaluate_argmax<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    at_most(args, 2)?;
    let t = as_tensor(arg(args, 0, ctx, engine, arena)?, arena)?;
    if t.ndim() == 0 {
        return Err(bad("argmax: a 0-d tensor has no axis to reduce"));
    }
    let axis = as_axis(arg(args, 1, ctx, engine, arena)?, t.ndim(), 0)?;
    if t.shape()[axis] == 0 {
        return Err(bad("argmax: cannot reduce a zero-length axis"));
    }
    charge(ctx, t.numel() as u64)?;

    let values = widen(t, arena)?;
    let extent = t.shape()[axis];
    let outer: usize = t.shape()[..axis].iter().product();
    let inner: usize = t.shape()[axis + 1..].iter().product();

    let mut out = bvec::<i64>(arena, outer * inner);
    for o in 0..outer {
        for i in 0..inner {
            let at = |k: usize| values[((o * extent) + k) * inner + i];
            let mut best = 0usize;
            let mut best_v = at(0);
            for k in 1..extent {
                // `>` and not `>=` keeps the first maximum, and is false
                // for NaN on either side.
                if at(k) > best_v {
                    best_v = at(k);
                    best = k;
                }
            }
            out.push(best as i64);
        }
    }

    // The reduced shape drops `axis`. Building the answer as an i64
    // tensor and expanding it reuses datavalue's nesting rather than
    // hand-rolling a second one.
    let mut shape = bvec::<usize>(arena, t.ndim() - 1);
    shape.extend_from_slice(&t.shape()[..axis]);
    shape.extend_from_slice(&t.shape()[axis + 1..]);
    let shape = shape.into_bump_slice();

    let reduced = DataTensor::from_slice(shape, out.into_bump_slice()).map_err(wrap)?;
    Ok(arena.alloc(reduced.to_nested_in(arena).map_err(wrap)?))
}

/// `to_list: [T]` — expand into plain nested JSON arrays.
///
/// The general escape hatch, and expensive by design: this is the one
/// operator that turns a compact buffer back into one `DataValue` node per
/// element. Reach for `argmax`, `shape` or a comparison first.
pub(crate) fn evaluate_to_list<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    at_most(args, 1)?;
    let t = as_tensor(arg(args, 0, ctx, engine, arena)?, arena)?;
    charge(ctx, t.numel() as u64)?;
    Ok(arena.alloc(t.to_nested_in(arena).map_err(wrap)?))
}

/// `shape: [T]` — the shape as a JSON array of numbers.
pub(crate) fn evaluate_shape<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    at_most(args, 1)?;
    let t = as_tensor(arg(args, 0, ctx, engine, arena)?, arena)?;
    charge(ctx, 1)?;
    let dims = arena.alloc_slice_fill_iter(
        t.shape()
            .iter()
            .map(|d| DataValue::Number(NumberValue::Integer(*d as i64))),
    );
    Ok(arena.alloc(DataValue::Array(dims)))
}

/// `dtype: [T]` — the dtype's wire name (`"f32"`, `"bool"`, `"bf16"`, …),
/// which is exactly what `tensor`, `zeros`, `full` and `cast` accept back.
pub(crate) fn evaluate_dtype<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    at_most(args, 1)?;
    let t = as_tensor(arg(args, 0, ctx, engine, arena)?, arena)?;
    charge(ctx, 1)?;
    Ok(arena.alloc(DataValue::String(t.dtype().name())))
}
