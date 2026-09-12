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
    Scalar, advance, arg, as_axis, as_i64_list, as_shape, as_tensor, at_most, bad, by_dtype,
    charge, cost, element_error, finish_bytes, numel_of, opt_arg, resolve_index,
    shape_without_axis, split_axis, strides_of, wrap,
};
use crate::arena::{ContextStack, DataValue, bvec};
use crate::{CompiledNode, Engine, Result};
use bumpalo::Bump;
use datavalue::{DType, DataTensor, TensorError};

/// Collect a tensor-array argument, checking that the members agree on
/// dtype (and, for `stack`, on shape — `concat` checks that itself).
fn tensor_list<'a>(v: &DataValue<'a>, arena: &'a Bump) -> Result<&'a [DataTensor<'a>]> {
    let DataValue::Array(items) = v else {
        return Err(bad("expected an array of tensors"));
    };
    if items.is_empty() {
        return Err(bad("expected at least one tensor"));
    }
    let mut out = bvec::<DataTensor<'a>>(arena, items.len());
    for it in *items {
        out.push(as_tensor(it, arena)?);
    }
    let dtype = out[0].dtype();
    if out.iter().any(|t| t.dtype() != dtype) {
        return Err(bad("tensors must share a dtype"));
    }
    Ok(out.into_bump_slice())
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
    finish_bytes(dtype, shape, out, arena)
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
    // Bytes each tensor contributes per outer step: its own extent along
    // the axis times everything inside it.
    let (outer, _, trailing) = split_axis(first.shape(), axis);

    let out = DataTensor::zeroed_bytes_in(dtype, shape, arena).map_err(wrap)?;
    let row = shape[axis] * trailing * cell;
    for o in 0..outer {
        let mut at = o * row;
        for t in parts {
            let chunk = t.shape()[axis] * trailing * cell;
            let src = o * chunk;
            out[at..at + chunk].copy_from_slice(&t.data()[src..src + chunk]);
            at += chunk;
        }
    }
    finish_bytes(dtype, shape, out, arena)
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

    let dtype = t.dtype();
    let cell = dtype.size_of();
    let (outer, n, inner) = split_axis(t.shape(), axis);
    let shape = shape_without_axis(t.shape(), axis, arena);

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
    finish_bytes(t.dtype(), shape, t.data(), arena)
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
    let out = DataTensor::zeroed_bytes_in(dtype, shape, arena).map_err(wrap)?;

    // Walking the output in row-major order, output axis `i` steps through
    // the input by the stride of the axis `perm[i]` names.
    let in_strides = strides_of(t.shape(), arena);
    let mut src_strides = bvec::<usize>(arena, rank);
    src_strides.extend(perm.iter().map(|&ax| in_strides[ax]));
    copy_block(
        t.data(),
        Origin::new(0, &src_strides),
        out,
        Origin::new(0, strides_of(shape, arena)),
        shape,
        dtype.size_of(),
        arena,
    );
    finish_bytes(dtype, shape, out, arena)
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
        if pattern.iter().any(|b| *b != 0) && !out.is_empty() {
            // Seed one cell, then double: log2(numel) copies instead of
            // one per element.
            out[..cell].copy_from_slice(pattern);
            let mut filled = cell;
            while filled < out.len() {
                let n = filled.min(out.len() - filled);
                out.copy_within(..n, filled);
                filled += n;
            }
        }
    }

    // The input lands at `before` inside the padded output.
    let out_strides = strides_of(shape, arena);
    let origin = before.iter().zip(out_strides).map(|(o, s)| o * s).sum();
    copy_block(
        t.data(),
        Origin::new(0, strides_of(t.shape(), arena)),
        out,
        Origin::new(origin, out_strides),
        t.shape(),
        cell,
        arena,
    );
    finish_bytes(dtype, shape, out, arena)
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
    let out = DataTensor::zeroed_bytes_in(dtype, shape, arena).map_err(wrap)?;

    // The window starts at `offset` inside the input.
    let in_strides = strides_of(t.shape(), arena);
    let origin = offset.iter().zip(in_strides).map(|(o, s)| o * s).sum();
    copy_block(
        t.data(),
        Origin::new(origin, in_strides),
        out,
        Origin::new(0, strides_of(shape, arena)),
        shape,
        dtype.size_of(),
        arena,
    );
    finish_bytes(dtype, shape, out, arena)
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

    let (outer, extent, inner) = split_axis(t.shape(), axis);
    let mut resolved = bvec::<usize>(arena, indices.len());
    for &i in indices {
        // Negative indices count from the end, as they do everywhere else
        // an axis position is named in this family.
        resolved.push(resolve_index(i, extent).ok_or_else(|| bad("gather: index out of range"))?);
    }
    let resolved = resolved.into_bump_slice();

    let mut shape = bvec::<usize>(arena, t.ndim());
    shape.extend_from_slice(t.shape());
    shape[axis] = resolved.len();
    let shape = shape.into_bump_slice();
    charge(ctx, cost(t.numel(), numel_of(shape)?))?;

    let dtype = t.dtype();
    let cell = dtype.size_of();
    let out = DataTensor::zeroed_bytes_in(dtype, shape, arena).map_err(wrap)?;
    for o in 0..outer {
        for (k, &src_k) in resolved.iter().enumerate() {
            let dst = ((o * resolved.len()) + k) * inner * cell;
            let src = ((o * extent) + src_k) * inner * cell;
            out[dst..dst + inner * cell].copy_from_slice(&t.data()[src..src + inner * cell]);
        }
    }
    finish_bytes(dtype, shape, out, arena)
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Where a block starts inside a row-major buffer, and how far one step
/// along each axis moves. Both in elements.
struct Origin<'s> {
    base: usize,
    strides: &'s [usize],
}

impl<'s> Origin<'s> {
    fn new(base: usize, strides: &'s [usize]) -> Self {
        Self { base, strides }
    }

    /// Element offset of the row that starts at `idx` (one entry per outer
    /// axis; the innermost axis is walked by the caller).
    fn row(&self, idx: &[usize]) -> usize {
        self.base
            + idx
                .iter()
                .zip(self.strides)
                .map(|(i, s)| i * s)
                .sum::<usize>()
    }

    /// Stride of the innermost axis; 1 for a 0-d block.
    fn step(&self) -> usize {
        self.strides.last().copied().unwrap_or(1)
    }
}

/// Copy a `window`-shaped block from `src` at `from` to `dst` at `to`.
///
/// One `copy_from_slice` per innermost row when both sides are contiguous
/// along that axis — every `crop` and `pad`, and any `transpose` that
/// keeps the last axis — and one per cell otherwise. `crop`, `pad` and
/// `transpose` are all this function with different origins.
fn copy_block(
    src: &[u8],
    from: Origin<'_>,
    dst: &mut [u8],
    to: Origin<'_>,
    window: &[usize],
    cell: usize,
    arena: &Bump,
) {
    if window.contains(&0) {
        return;
    }
    // A 0-d window is one element: a single row of length 1.
    let (rows, n) = window
        .split_last()
        .map_or((&[][..], 1), |(n, rows)| (rows, *n));
    let (src_step, dst_step) = (from.step(), to.step());

    let mut idx = bvec::<usize>(arena, rows.len());
    idx.resize(rows.len(), 0);
    loop {
        let (s, d) = (from.row(&idx), to.row(&idx));
        if src_step == 1 && dst_step == 1 {
            dst[d * cell..(d + n) * cell].copy_from_slice(&src[s * cell..(s + n) * cell]);
        } else {
            for k in 0..n {
                let (sk, dk) = ((s + k * src_step) * cell, (d + k * dst_step) * cell);
                dst[dk..dk + cell].copy_from_slice(&src[sk..sk + cell]);
            }
        }
        if !advance(&mut idx, rows) {
            break;
        }
    }
}

/// The byte pattern of one element of `dtype` holding `v`. Built by making
/// a one-element tensor and reading its payload, which keeps every dtype's
/// encoding in datavalue rather than duplicating it here.
fn scalar_bytes<'a>(dtype: DType, v: &DataValue<'_>, arena: &'a Bump) -> Result<&'a [u8]> {
    by_dtype!(dtype, scalar_bytes_impl, v, arena)
}

fn scalar_bytes_impl<'a, T: Scalar>(v: &DataValue<'_>, arena: &'a Bump) -> Result<&'a [u8]> {
    let x = T::from_value(v).ok_or_else(|| element_error(0, T::DTYPE))?;
    let one = arena.alloc_slice_copy(&[x]);
    let shape = arena.alloc_slice_copy(&[1usize]);
    Ok(DataTensor::from_slice(shape, one).map_err(wrap)?.data())
}
