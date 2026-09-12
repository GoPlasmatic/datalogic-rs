//! Shape operators: `stack`, `concat`, `unstack`, `reshape`, `transpose`,
//! `pad`, `crop`, `gather`.
//!
//! These move `DType::size_of()`-byte cells and never interpret one, so
//! they work on every dtype — `f16` and `bf16` included — without the
//! `tensor-half` feature and without a `half` dependency. The single
//! exception is `pad`'s optional non-zero fill value, which has to be
//! converted into the dtype before it can be written; a zero pad (the
//! default) stays universal because a zeroed buffer is a valid value of
//! every dtype.

use super::{
    Scalar, arg, as_axis, as_i64_list, as_shape, as_tensor, at_most, bad, by_dtype, charge, cost,
    finish, numel_of, opt_arg, strides_of, wrap,
};
use crate::arena::{ContextStack, DataValue, bvec};
use crate::{CompiledNode, Engine, Result};
use bumpalo::Bump;
use datavalue::{DType, DataTensor, TensorError};

/// Collect a tensor-array argument, checking that the members agree on
/// dtype (and, for `stack`, on shape — `concat` checks that itself).
fn tensor_list<'a>(v: &DataValue<'a>, arena: &'a Bump) -> Result<Vec<DataTensor<'a>>> {
    let DataValue::Array(items) = v else {
        return Err(bad("expected an array of tensors"));
    };
    if items.is_empty() {
        return Err(bad("expected at least one tensor"));
    }
    let out: Vec<DataTensor<'a>> = items
        .iter()
        .map(|it| as_tensor(it, arena))
        .collect::<Result<_>>()?;
    let dtype = out[0].dtype();
    if out.iter().any(|t| t.dtype() != dtype) {
        return Err(bad("tensors must share a dtype"));
    }
    Ok(out)
}

/// `stack: [tensors, axis]` — join equal-shaped tensors along a **new**
/// axis, so the result has one more dimension than its inputs.
pub(crate) fn evaluate_stack<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    at_most(args, 2)?;
    let parts = tensor_list(arg(args, 0, ctx, engine, arena)?, arena)?;
    let first = parts[0];
    let axis = as_axis(arg(args, 1, ctx, engine, arena)?, first.ndim(), 1)?;

    if parts.iter().any(|t| t.shape() != first.shape()) {
        return Err(bad("stack: every tensor must have the same shape"));
    }
    let total: usize = parts.iter().map(|t| t.numel()).sum();
    charge(ctx, total as u64)?;

    // Insert the new axis: [outer…, n, inner…].
    let mut shape = bvec::<usize>(arena, first.ndim() + 1);
    shape.extend_from_slice(&first.shape()[..axis]);
    shape.push(parts.len());
    shape.extend_from_slice(&first.shape()[axis..]);
    let shape = shape.into_bump_slice();

    let dtype = first.dtype();
    let cell = dtype.size_of();
    let inner: usize = first.shape()[axis..].iter().product();
    let outer: usize = first.shape()[..axis].iter().product();

    let out = DataTensor::zeroed_bytes_in(dtype, shape, arena).map_err(wrap)?;
    for o in 0..outer {
        for (k, t) in parts.iter().enumerate() {
            let dst = ((o * parts.len()) + k) * inner * cell;
            let src = o * inner * cell;
            out[dst..dst + inner * cell].copy_from_slice(&t.data()[src..src + inner * cell]);
        }
    }
    finish(
        DataTensor::from_bytes(dtype, shape, out).map_err(wrap)?,
        arena,
    )
}

/// `concat: [tensors, axis]` — join along an **existing** axis. Shapes
/// must agree on every axis but that one.
pub(crate) fn evaluate_concat<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    at_most(args, 2)?;
    let parts = tensor_list(arg(args, 0, ctx, engine, arena)?, arena)?;
    let first = parts[0];
    if first.ndim() == 0 {
        return Err(bad(
            "concat: cannot concatenate 0-d tensors; stack them instead",
        ));
    }
    let axis = as_axis(arg(args, 1, ctx, engine, arena)?, first.ndim(), 0)?;

    let agrees = |t: &DataTensor<'_>| {
        t.ndim() == first.ndim()
            && t.shape()
                .iter()
                .zip(first.shape())
                .enumerate()
                .all(|(i, (a, b))| i == axis || a == b)
    };
    if !parts.iter().all(agrees) {
        return Err(bad(
            "concat: shapes must match on every axis but the concatenated one",
        ));
    }
    let total: usize = parts.iter().map(|t| t.numel()).sum();
    charge(ctx, total as u64)?;

    let mut shape = bvec::<usize>(arena, first.ndim());
    shape.extend_from_slice(first.shape());
    shape[axis] = parts.iter().map(|t| t.shape()[axis]).sum();
    let shape = shape.into_bump_slice();

    let dtype = first.dtype();
    let cell = dtype.size_of();
    let outer: usize = first.shape()[..axis].iter().product();
    // Bytes each tensor contributes per outer step: its own extent along
    // the axis times everything inside it.
    let trailing: usize = first.shape()[axis + 1..].iter().product();

    let out = DataTensor::zeroed_bytes_in(dtype, shape, arena).map_err(wrap)?;
    let row = shape[axis] * trailing * cell;
    for o in 0..outer {
        let mut at = o * row;
        for t in &parts {
            let chunk = t.shape()[axis] * trailing * cell;
            let src = o * chunk;
            out[at..at + chunk].copy_from_slice(&t.data()[src..src + chunk]);
            at += chunk;
        }
    }
    finish(
        DataTensor::from_bytes(dtype, shape, out).map_err(wrap)?,
        arena,
    )
}

/// `unstack: [T, axis]` — the inverse of `stack`: split along `axis` and
/// drop it, giving an array of tensors one rank lower.
pub(crate) fn evaluate_unstack<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    at_most(args, 2)?;
    let t = as_tensor(arg(args, 0, ctx, engine, arena)?, arena)?;
    if t.ndim() == 0 {
        return Err(bad("unstack: a 0-d tensor has no axis to split"));
    }
    let axis = as_axis(arg(args, 1, ctx, engine, arena)?, t.ndim(), 0)?;
    charge(ctx, t.numel() as u64)?;

    let n = t.shape()[axis];
    let dtype = t.dtype();
    let cell = dtype.size_of();
    let outer: usize = t.shape()[..axis].iter().product();
    let inner: usize = t.shape()[axis + 1..].iter().product();

    let mut shape = bvec::<usize>(arena, t.ndim() - 1);
    shape.extend_from_slice(&t.shape()[..axis]);
    shape.extend_from_slice(&t.shape()[axis + 1..]);
    let shape = shape.into_bump_slice();

    let mut parts = bvec::<DataValue<'a>>(arena, n);
    for k in 0..n {
        let out = DataTensor::zeroed_bytes_in(dtype, shape, arena).map_err(wrap)?;
        for o in 0..outer {
            let src = ((o * n) + k) * inner * cell;
            let dst = o * inner * cell;
            out[dst..dst + inner * cell].copy_from_slice(&t.data()[src..src + inner * cell]);
        }
        let part = DataTensor::from_bytes(dtype, shape, out).map_err(wrap)?;
        parts.push(DataValue::tensor_in(part, arena));
    }
    Ok(arena.alloc(DataValue::Array(parts.into_bump_slice())))
}

/// `reshape: [T, shape]` — reinterpret the same bytes under a new shape.
///
/// The only operator in the family that allocates nothing but a header,
/// hence its charge of 1: the payload is shared with the input, not
/// copied. The element count must match exactly; there is no inferred
/// `-1` dimension.
pub(crate) fn evaluate_reshape<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    at_most(args, 2)?;
    let t = as_tensor(arg(args, 0, ctx, engine, arena)?, arena)?;
    let shape = as_shape(arg(args, 1, ctx, engine, arena)?, arena)?;
    charge(ctx, 1)?;

    if numel_of(shape)? != t.numel() {
        return Err(bad(
            "reshape: the new shape must hold exactly as many elements",
        ));
    }
    // `t.data()` is already aligned for its dtype, so this is the
    // zero-copy `from_bytes` rather than the copying `from_bytes_in`.
    finish(
        DataTensor::from_bytes(t.dtype(), shape, t.data()).map_err(wrap)?,
        arena,
    )
}

/// `transpose: [T, perm?]` — permute the axes. `perm` defaults to a full
/// reversal, so a 2-d transpose needs no second argument.
pub(crate) fn evaluate_transpose<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    at_most(args, 2)?;
    let t = as_tensor(arg(args, 0, ctx, engine, arena)?, arena)?;
    let rank = t.ndim();

    let perm: &[usize] = match opt_arg(args, 1, ctx, engine, arena)? {
        Some(v) => {
            let p = as_shape(v, arena)?;
            if p.len() != rank {
                return Err(bad("transpose: perm must name every axis"));
            }
            let mut seen = bvec::<bool>(arena, rank);
            seen.resize(rank, false);
            for &ax in p {
                if ax >= rank || seen[ax] {
                    return Err(bad("transpose: perm must be a permutation of the axes"));
                }
                seen[ax] = true;
            }
            p
        }
        None => {
            let mut p = bvec::<usize>(arena, rank);
            p.extend((0..rank).rev());
            p.into_bump_slice()
        }
    };
    charge(ctx, t.numel() as u64)?;

    let mut shape = bvec::<usize>(arena, rank);
    for &ax in perm {
        shape.push(t.shape()[ax]);
    }
    let shape = shape.into_bump_slice();

    let dtype = t.dtype();
    let cell = dtype.size_of();
    let in_strides = strides_of(t.shape(), arena);
    let out = DataTensor::zeroed_bytes_in(dtype, shape, arena).map_err(wrap)?;

    // Walk the output in row-major order; for each position, map its
    // coordinates back through `perm` to find the source cell.
    let mut idx = bvec::<usize>(arena, rank);
    idx.resize(rank, 0);
    let mut at = 0usize;
    if t.numel() > 0 {
        loop {
            let src: usize = (0..rank).map(|i| idx[i] * in_strides[perm[i]]).sum();
            out[at * cell..(at + 1) * cell]
                .copy_from_slice(&t.data()[src * cell..(src + 1) * cell]);
            at += 1;
            if !super::advance(&mut idx, shape) {
                break;
            }
        }
    }
    finish(
        DataTensor::from_bytes(dtype, shape, out).map_err(wrap)?,
        arena,
    )
}

/// `pad: [T, before, after, value?]` — grow every axis by a leading and a
/// trailing margin, filling the margin with `value` (0 by default).
pub(crate) fn evaluate_pad<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    at_most(args, 4)?;
    let t = as_tensor(arg(args, 0, ctx, engine, arena)?, arena)?;
    let rank = t.ndim();
    let before = as_shape(arg(args, 1, ctx, engine, arena)?, arena)?;
    let after = as_shape(arg(args, 2, ctx, engine, arena)?, arena)?;
    let fill = opt_arg(args, 3, ctx, engine, arena)?;

    if before.len() != rank || after.len() != rank {
        return Err(bad("pad: before and after must have one entry per axis"));
    }
    let mut shape = bvec::<usize>(arena, rank);
    for ax in 0..rank {
        shape.push(
            before[ax]
                .checked_add(t.shape()[ax])
                .and_then(|s| s.checked_add(after[ax]))
                .ok_or_else(|| wrap(TensorError::ShapeOverflow))?,
        );
    }
    let shape = shape.into_bump_slice();
    charge(ctx, cost(t.numel(), numel_of(shape)?))?;

    let dtype = t.dtype();
    let cell = dtype.size_of();
    let out = DataTensor::zeroed_bytes_in(dtype, shape, arena).map_err(wrap)?;

    // A zero fill is already done — the buffer arrives zeroed, and that
    // is a valid value of every dtype. Only a non-zero fill needs the
    // element conversion, and only then does `pad` require `tensor-half`.
    if let Some(v) = fill {
        let pattern = scalar_bytes(dtype, v, arena)?;
        if pattern.iter().any(|b| *b != 0) {
            for chunk in out.chunks_exact_mut(cell) {
                chunk.copy_from_slice(pattern);
            }
        }
    }

    copy_window(t, out, shape, before, arena)?;
    finish(
        DataTensor::from_bytes(dtype, shape, out).map_err(wrap)?,
        arena,
    )
}

/// `crop: [T, offset, shape]` — the inverse of `pad`: cut a sub-block out
/// of the tensor. The window must lie inside the input.
pub(crate) fn evaluate_crop<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    at_most(args, 3)?;
    let t = as_tensor(arg(args, 0, ctx, engine, arena)?, arena)?;
    let rank = t.ndim();
    let offset = as_shape(arg(args, 1, ctx, engine, arena)?, arena)?;
    let shape = as_shape(arg(args, 2, ctx, engine, arena)?, arena)?;

    if offset.len() != rank || shape.len() != rank {
        return Err(bad("crop: offset and shape must have one entry per axis"));
    }
    for ax in 0..rank {
        let end = offset[ax]
            .checked_add(shape[ax])
            .ok_or_else(|| wrap(TensorError::ShapeOverflow))?;
        if end > t.shape()[ax] {
            return Err(bad("crop: the window runs past the end of the tensor"));
        }
    }
    charge(ctx, cost(t.numel(), numel_of(shape)?))?;

    let dtype = t.dtype();
    let cell = dtype.size_of();
    let in_strides = strides_of(t.shape(), arena);
    let out = DataTensor::zeroed_bytes_in(dtype, shape, arena).map_err(wrap)?;

    let mut idx = bvec::<usize>(arena, rank);
    idx.resize(rank, 0);
    let mut at = 0usize;
    if numel_of(shape)? > 0 {
        loop {
            let src: usize = (0..rank)
                .map(|i| (idx[i] + offset[i]) * in_strides[i])
                .sum();
            out[at * cell..(at + 1) * cell]
                .copy_from_slice(&t.data()[src * cell..(src + 1) * cell]);
            at += 1;
            if !super::advance(&mut idx, shape) {
                break;
            }
        }
    }
    finish(
        DataTensor::from_bytes(dtype, shape, out).map_err(wrap)?,
        arena,
    )
}

/// `gather: [T, indices, axis?]` — select slices along `axis` (0 by
/// default) in the order `indices` gives, so it both reorders and
/// resamples. Every index must be in range: unlike `scatter`, dropping one
/// would silently change the output shape.
pub(crate) fn evaluate_gather<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    at_most(args, 3)?;
    let t = as_tensor(arg(args, 0, ctx, engine, arena)?, arena)?;
    if t.ndim() == 0 {
        return Err(bad("gather: a 0-d tensor has no axis to index"));
    }
    let indices = as_i64_list(arg(args, 1, ctx, engine, arena)?, arena)?;
    let axis = match opt_arg(args, 2, ctx, engine, arena)? {
        Some(v) => as_axis(v, t.ndim(), 0)?,
        None => 0,
    };

    let extent = t.shape()[axis];
    let mut resolved = bvec::<usize>(arena, indices.len());
    for &i in indices {
        // Negative indices count from the end, as they do everywhere else
        // an axis position is named in this family.
        let r = if i < 0 { i + extent as i64 } else { i };
        if r < 0 || r as usize >= extent {
            return Err(bad("gather: index out of range"));
        }
        resolved.push(r as usize);
    }
    let resolved = resolved.into_bump_slice();

    let mut shape = bvec::<usize>(arena, t.ndim());
    shape.extend_from_slice(t.shape());
    shape[axis] = resolved.len();
    let shape = shape.into_bump_slice();
    charge(ctx, cost(t.numel(), numel_of(shape)?))?;

    let dtype = t.dtype();
    let cell = dtype.size_of();
    let outer: usize = t.shape()[..axis].iter().product();
    let inner: usize = t.shape()[axis + 1..].iter().product();

    let out = DataTensor::zeroed_bytes_in(dtype, shape, arena).map_err(wrap)?;
    for o in 0..outer {
        for (k, &src_k) in resolved.iter().enumerate() {
            let dst = ((o * resolved.len()) + k) * inner * cell;
            let src = ((o * extent) + src_k) * inner * cell;
            out[dst..dst + inner * cell].copy_from_slice(&t.data()[src..src + inner * cell]);
        }
    }
    finish(
        DataTensor::from_bytes(dtype, shape, out).map_err(wrap)?,
        arena,
    )
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Copy `t` into `out` at `offset`, where `out` has `shape` and is at
/// least as large as `t` on every axis. Shared by `pad`.
fn copy_window(
    t: DataTensor<'_>,
    out: &mut [u8],
    shape: &[usize],
    offset: &[usize],
    arena: &Bump,
) -> Result<()> {
    if t.numel() == 0 {
        return Ok(());
    }
    let rank = t.ndim();
    let cell = t.dtype().size_of();
    let out_strides = strides_of(shape, arena);

    let mut idx = bvec::<usize>(arena, rank);
    idx.resize(rank, 0);
    let mut at = 0usize;
    loop {
        let dst: usize = (0..rank)
            .map(|i| (idx[i] + offset[i]) * out_strides[i])
            .sum();
        out[dst * cell..(dst + 1) * cell].copy_from_slice(&t.data()[at * cell..(at + 1) * cell]);
        at += 1;
        if !super::advance(&mut idx, t.shape()) {
            break;
        }
    }
    Ok(())
}

/// The byte pattern of one element of `dtype` holding `v`. Built by making
/// a one-element tensor and reading its payload, which keeps every dtype's
/// encoding in datavalue rather than duplicating it here.
fn scalar_bytes<'a>(dtype: DType, v: &DataValue<'_>, arena: &'a Bump) -> Result<&'a [u8]> {
    by_dtype!(dtype, scalar_bytes_impl, v, arena)
}

fn scalar_bytes_impl<'a, T: Scalar>(v: &DataValue<'_>, arena: &'a Bump) -> Result<&'a [u8]> {
    let x = T::from_value(v).ok_or_else(|| {
        wrap(TensorError::Element {
            index: 0,
            expected: T::DTYPE,
        })
    })?;
    let one = arena.alloc_slice_copy(&[x]);
    let shape = arena.alloc_slice_copy(&[1usize]);
    Ok(DataTensor::from_slice(shape, one).map_err(wrap)?.data())
}
