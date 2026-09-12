//! Tensor constructors: `tensor`, `zeros`, `full`, `scatter`,
//! `rle_expand`, `one_hot`.
//!
//! Every one of these can be asked to produce far more data than the rule
//! that names it, which is why each charges its declared element count
//! before it allocates rather than after it has built the buffer.
//!
//! The element-wise paths collect into a `bumpalo::collections::Vec<T>`
//! and hand the finished slice to `DataTensor::from_slice`, which wraps it
//! with no copy: the vector's own allocation *is* the tensor's payload.

use super::{
    Scalar, arg, as_dtype, as_i64, as_shape, as_usize, at_most, bad, by_dtype, charge, cost,
    finish, numel_of, opt_arg, strides_of, wrap,
};
use crate::arena::{ContextStack, DataValue, bvec};
use crate::{CompiledNode, Engine, Result};
use bumpalo::Bump;
use datavalue::{DType, DataTensor, TensorError};

/// Is this the tagged `{"tensor": {...}}` wire form? A single-key object
/// under datavalue's tag; anything else is a nested-array input.
fn is_tagged(v: &DataValue<'_>) -> bool {
    matches!(v, DataValue::Object([(k, _)]) if *k == DataTensor::JSON_TAG)
}

/// The tagged form's *body*, arriving without its wrapper.
///
/// Two callers produce this. A rule containing the emitter's output
/// (`{"tensor": {"dtype": .., "shape": .., "data": ..}}`) hands the body
/// over as the operator's argument, the tag having been consumed as the
/// operator name; and a caller who stored just the body in their data
/// reasonably expects it to decode.
fn is_wire_body(v: &DataValue<'_>) -> bool {
    matches!(v, DataValue::Object(pairs)
        if pairs.iter().any(|(k, _)| *k == "dtype")
            && pairs.iter().all(|(k, _)| matches!(*k, "dtype" | "shape" | "data")))
}

/// Put the tag back on so datavalue's boundary decoder — which is strict
/// about the wrapper, and about unknown and repeated inner keys — can read
/// it. One pair allocation, no payload copy.
fn decode_body<'a>(v: &DataValue<'a>, arena: &'a Bump) -> Result<DataTensor<'a>> {
    let tagged = arena.alloc([(DataTensor::JSON_TAG, *v)]);
    DataTensor::from_json_value_in(&DataValue::Object(&tagged[..]), arena).map_err(wrap)
}

/// `tensor: [value, dtype?]` — the family's entry point and its sentinel.
///
/// Three inputs, mirroring `datetime`'s "the operator name is also the
/// wire tag" shape, so what the emitter writes evaluates back to the value
/// it came from:
///
/// - a tensor passes through untouched,
/// - the tagged `{"tensor": {...}}` object decodes from its base64,
/// - nested JSON arrays (or a bare scalar, for a 0-d tensor) decode into
///   the declared `dtype`, with the shape inferred from the nesting.
///
/// A flat list plus an explicit shape is `reshape` over this.
pub(crate) fn evaluate_tensor<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    at_most(args, 2)?;
    let v = arg(args, 0, ctx, engine, arena)?;

    // Unlike the shape-declaring constructors below, both paths here are
    // bounded by an input value that is already materialised in the
    // arena, so there is nothing to refuse up front: the charge is the
    // size of what we were handed. Hence charging after the decode.
    let t = if let DataValue::Tensor(t) = v {
        **t
    } else if is_tagged(v) {
        DataTensor::from_json_value_in(v, arena).map_err(wrap)?
    } else if is_wire_body(v) {
        decode_body(v, arena)?
    } else {
        let dtype = as_dtype(opt_arg(args, 1, ctx, engine, arena)?.ok_or_else(|| {
            bad("tensor: nested-array input needs a dtype, e.g. {\"tensor\": [[[1,2]], \"f32\"]}")
        })?)?;
        DataTensor::from_nested_in(v, dtype, arena).map_err(wrap)?
    };

    charge(ctx, t.numel() as u64)?;
    finish(t, arena)
}

/// `zeros: [shape, dtype]` — an all-zero tensor.
///
/// The one element-wise-looking constructor that needs no `Element` impl:
/// an all-zero buffer is a valid value of every dtype, `f16` / `bf16`
/// included, so this works without `tensor-half` where `full` does not.
pub(crate) fn evaluate_zeros<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    at_most(args, 2)?;
    let shape = as_shape(arg(args, 0, ctx, engine, arena)?, arena)?;
    let dtype = as_dtype(arg(args, 1, ctx, engine, arena)?)?;

    charge(ctx, numel_of(shape)? as u64)?;
    let buf = DataTensor::zeroed_bytes_in(dtype, shape, arena).map_err(wrap)?;
    finish(
        DataTensor::from_bytes(dtype, shape, buf).map_err(wrap)?,
        arena,
    )
}

/// `full: [shape, dtype, value]` — every element set to `value`.
pub(crate) fn evaluate_full<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    at_most(args, 3)?;
    let shape = as_shape(arg(args, 0, ctx, engine, arena)?, arena)?;
    let dtype = as_dtype(arg(args, 1, ctx, engine, arena)?)?;
    let value = arg(args, 2, ctx, engine, arena)?;

    charge(ctx, numel_of(shape)? as u64)?;
    by_dtype!(dtype, full_impl, shape, value, arena)
}

fn full_impl<'a, T: Scalar>(
    shape: &'a [usize],
    value: &DataValue<'_>,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    let fill = T::from_value(value).ok_or_else(|| element_error(0, T::DTYPE))?;
    let mut data = bvec::<T>(arena, numel_of(shape)?);
    data.resize(numel_of(shape)?, fill);
    finish(
        DataTensor::from_slice(shape, data.into_bump_slice()).map_err(wrap)?,
        arena,
    )
}

/// `scatter: [points, shape, dtype, value?]` — a sparse write into an
/// otherwise-zero tensor.
///
/// A point is a coordinate array: `[i, j]` writes `value` (1 by default),
/// `[i, j, v]` writes `v`. Out-of-range points are dropped rather than
/// rejected, because the usual producer is a detector emitting boxes in
/// source coordinates that may fall outside the target grid, and dropping
/// them is what the consumer would otherwise write by hand.
pub(crate) fn evaluate_scatter<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    at_most(args, 4)?;
    let points = arg(args, 0, ctx, engine, arena)?;
    let shape = as_shape(arg(args, 1, ctx, engine, arena)?, arena)?;
    let dtype = as_dtype(arg(args, 2, ctx, engine, arena)?)?;
    let value = opt_arg(args, 3, ctx, engine, arena)?;

    let DataValue::Array(points) = points else {
        return Err(bad("scatter: points must be an array of coordinate arrays"));
    };
    charge(ctx, cost(points.len(), numel_of(shape)?))?;
    by_dtype!(dtype, scatter_impl, points, shape, value, arena)
}

fn scatter_impl<'a, T: Scalar>(
    points: &[DataValue<'_>],
    shape: &'a [usize],
    value: Option<&DataValue<'_>>,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    let default = match value {
        None => T::ONE,
        Some(v) => T::from_value(v).ok_or_else(|| element_error(0, T::DTYPE))?,
    };
    let rank = shape.len();
    let strides = strides_of(shape, arena);
    let numel = numel_of(shape)?;

    let mut data = bvec::<T>(arena, numel);
    data.resize(numel, T::ZERO);

    for (n, point) in points.iter().enumerate() {
        let DataValue::Array(coords) = point else {
            return Err(bad("scatter: each point must be an array"));
        };
        // `rank` coordinates use the default value; one extra trailing
        // entry is the value to write at that coordinate.
        let (coords, written) = match coords.len() {
            l if l == rank => (&coords[..], default),
            l if l == rank + 1 => (
                &coords[..rank],
                T::from_value(&coords[rank]).ok_or_else(|| element_error(n, T::DTYPE))?,
            ),
            _ => {
                return Err(bad(
                    "scatter: point length must match the rank, or rank + 1",
                ));
            }
        };

        let mut offset = 0usize;
        let mut inside = true;
        for (ax, c) in coords.iter().enumerate() {
            let c = as_i64(c)?;
            // Negative indices are out of range here rather than
            // wrapping: a detector emitting -1 means "no box", and
            // silently writing to the last row would be worse than
            // dropping it.
            if c < 0 || c as usize >= shape[ax] {
                inside = false;
                break;
            }
            offset += c as usize * strides[ax];
        }
        if inside {
            data[offset] = written;
        }
    }

    finish(
        DataTensor::from_slice(shape, data.into_bump_slice()).map_err(wrap)?,
        arena,
    )
}

/// `rle_expand: [runs, shape, dtype]` — run-length decode into a tensor.
///
/// `runs` is the flat `[v0, n0, v1, n1, …]` pairing, filled row-major. The
/// run lengths must sum to exactly the shape's element count: a mask that
/// decodes to the wrong size is a bug in the producer, and filling the
/// remainder with zeros would hide it.
pub(crate) fn evaluate_rle_expand<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    at_most(args, 3)?;
    let runs = arg(args, 0, ctx, engine, arena)?;
    let shape = as_shape(arg(args, 1, ctx, engine, arena)?, arena)?;
    let dtype = as_dtype(arg(args, 2, ctx, engine, arena)?)?;

    let DataValue::Array(runs) = runs else {
        return Err(bad(
            "rle_expand: runs must be a flat [value, count, …] array",
        ));
    };
    if !runs.len().is_multiple_of(2) {
        return Err(bad("rle_expand: runs must have an even length"));
    }
    charge(ctx, cost(runs.len(), numel_of(shape)?))?;
    by_dtype!(dtype, rle_impl, runs, shape, arena)
}

fn rle_impl<'a, T: Scalar>(
    runs: &[DataValue<'_>],
    shape: &'a [usize],
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    let numel = numel_of(shape)?;
    let mut data = bvec::<T>(arena, numel);

    for (pair, chunk) in runs.as_chunks::<2>().0.iter().enumerate() {
        let value = T::from_value(&chunk[0]).ok_or_else(|| element_error(pair, T::DTYPE))?;
        let count = as_usize(&chunk[1])?;
        if data.len() + count > numel {
            return Err(bad(
                "rle_expand: runs decode to more elements than the shape holds",
            ));
        }
        data.resize(data.len() + count, value);
    }
    if data.len() != numel {
        return Err(bad(
            "rle_expand: runs decode to fewer elements than the shape holds",
        ));
    }

    finish(
        DataTensor::from_slice(shape, data.into_bump_slice()).map_err(wrap)?,
        arena,
    )
}

/// `one_hot: [indices, depth, dtype]` — a `[len, depth]` indicator matrix.
///
/// An index outside `0..depth` leaves its row all-zero, which is what
/// every framework's `one_hot` does and what a caller encoding an
/// "unknown category" sentinel expects.
pub(crate) fn evaluate_one_hot<'a>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    at_most(args, 3)?;
    let indices = arg(args, 0, ctx, engine, arena)?;
    let depth = as_usize(arg(args, 1, ctx, engine, arena)?)?;
    let dtype = as_dtype(arg(args, 2, ctx, engine, arena)?)?;

    let DataValue::Array(indices) = indices else {
        return Err(bad("one_hot: indices must be an array"));
    };
    let shape = arena.alloc_slice_copy(&[indices.len(), depth]);
    charge(ctx, numel_of(shape)? as u64)?;
    by_dtype!(dtype, one_hot_impl, indices, shape, arena)
}

fn one_hot_impl<'a, T: Scalar>(
    indices: &[DataValue<'_>],
    shape: &'a [usize],
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    let depth = shape[1];
    let numel = numel_of(shape)?;
    let mut data = bvec::<T>(arena, numel);
    data.resize(numel, T::ZERO);

    for (row, idx) in indices.iter().enumerate() {
        let i = as_i64(idx)?;
        if i >= 0 && (i as usize) < depth {
            data[row * depth + i as usize] = T::ONE;
        }
    }

    finish(
        DataTensor::from_slice(shape, data.into_bump_slice()).map_err(wrap)?,
        arena,
    )
}

/// The same error datavalue raises when a nested leaf will not fit its
/// dtype, so both decode paths report an unrepresentable element the same
/// way.
fn element_error(index: usize, expected: DType) -> crate::Error {
    wrap(TensorError::Element { index, expected })
}
