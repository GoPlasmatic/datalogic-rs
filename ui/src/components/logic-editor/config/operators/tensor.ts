/**
 * Tensor Operators
 *
 * Marshalling between JSON and typed n-dimensional buffers, for rules that
 * feed a model and read its output back. Deliberately arithmetic-free:
 * these operators move data, they never compute over it.
 *
 * A tensor crosses the JSON boundary as the tagged
 * `{"tensor": {"dtype", "shape", "data"}}` form, with `data` little-endian
 * base64 — which is why most examples below wrap the result in `to_list`
 * or `shape` rather than showing the payload.
 */

import type { Operator } from '../operators.types';

const DTYPES =
  'bool, i8, u8, i16, u16, i32, u32, i64, u64, f16, bf16, f32, f64';

export const tensorOperators: Record<string, Operator> = {
  tensor: {
    name: 'tensor',
    label: 'Tensor',
    category: 'tensor',
    description: 'Build a tensor, or pass one through',
    arity: {
      type: 'range',
      min: 1,
      max: 2,
      args: [
        { name: 'value', label: 'Value', type: 'any', required: true },
        { name: 'dtype', label: 'Dtype', type: 'string', required: false },
      ],
    },
    help: {
      summary: 'The family entry point: nested arrays, the wire form, or a tensor',
      details:
        `Nested JSON arrays need a declared dtype (${DTYPES}) and infer their shape from the nesting. ` +
        'The tagged `{"tensor": {...}}` wire form decodes from its base64, and an existing tensor passes through. ' +
        'An element that will not fit the dtype is an error, never a truncation.',
      returnType: 'any',
      examples: [
        {
          title: 'From nested arrays',
          rule: { to_list: [{ tensor: [[[1, 2], [3, 4]], 'f32'] }] },
          result: [[1, 2], [3, 4]],
        },
        {
          title: 'Read a tensor out of the data',
          rule: { shape: [{ tensor: [{ var: 'prediction' }] }] },
          data: { prediction: { tensor: { dtype: 'u8', shape: [3], data: 'AQID' } } },
          result: [3],
        },
      ],
      notes: [
        'A flat list plus a shape is `reshape` over this',
        'A bare scalar makes a 0-d tensor',
      ],
      seeAlso: ['to_list', 'reshape', 'zeros'],
    },
    ui: { icon: 'layers', shortLabel: 'tensor', nodeType: 'operator' },
  },

  zeros: {
    name: 'zeros',
    label: 'Zeros',
    category: 'tensor',
    description: 'An all-zero tensor of a given shape',
    arity: {
      type: 'binary',
      args: [
        { name: 'shape', label: 'Shape', type: 'array', required: true },
        { name: 'dtype', label: 'Dtype', type: 'string', required: true },
      ],
    },
    help: {
      summary: 'Allocate a zero-filled tensor',
      details:
        'Needs no element support, so this works on every dtype including f16 / bf16 without the `tensor-half` feature.',
      returnType: 'any',
      examples: [
        {
          title: 'A 2x3 grid of zeros',
          rule: { to_list: [{ zeros: [[2, 3], 'i32'] }] },
          result: [[0, 0, 0], [0, 0, 0]],
        },
      ],
      seeAlso: ['full', 'tensor'],
    },
    ui: { icon: 'boxes', shortLabel: 'zeros', nodeType: 'operator' },
  },

  full: {
    name: 'full',
    label: 'Full',
    category: 'tensor',
    description: 'A tensor with every element set to one value',
    arity: {
      type: 'ternary',
      args: [
        { name: 'shape', label: 'Shape', type: 'array', required: true },
        { name: 'dtype', label: 'Dtype', type: 'string', required: true },
        { name: 'value', label: 'Value', type: 'any', required: true },
      ],
    },
    help: {
      summary: 'Allocate a tensor filled with a constant',
      returnType: 'any',
      examples: [
        {
          title: 'A 2x2 of 1.5',
          rule: { to_list: [{ full: [[2, 2], 'f32', 1.5] }] },
          result: [[1.5, 1.5], [1.5, 1.5]],
        },
      ],
      seeAlso: ['zeros', 'scatter'],
    },
    ui: { icon: 'boxes', shortLabel: 'full', nodeType: 'operator' },
  },

  scatter: {
    name: 'scatter',
    label: 'Scatter',
    category: 'tensor',
    description: 'Sparse writes into an otherwise-zero tensor',
    arity: {
      type: 'range',
      min: 3,
      max: 4,
      args: [
        { name: 'points', label: 'Points', type: 'array', required: true },
        { name: 'shape', label: 'Shape', type: 'array', required: true },
        { name: 'dtype', label: 'Dtype', type: 'string', required: true },
        { name: 'value', label: 'Value', type: 'any', required: false },
      ],
    },
    help: {
      summary: 'Write values at listed coordinates, zero everywhere else',
      details:
        'A point of length `rank` writes the default value (1 unless given); one extra trailing entry is the value to write. ' +
        'Out-of-range points are dropped rather than rejected, because the usual producer is a detector emitting coordinates that may fall outside the grid.',
      returnType: 'any',
      examples: [
        {
          title: 'Two points on a 2x2 grid',
          rule: { to_list: [{ scatter: [[[0, 1], [1, 0]], [2, 2], 'u8'] }] },
          result: [[0, 1], [1, 0]],
        },
      ],
      notes: ['Out-of-range and negative coordinates are dropped'],
      seeAlso: ['rle_expand', 'one_hot', 'full'],
    },
    ui: { icon: 'hash', shortLabel: 'scatter', nodeType: 'operator' },
  },

  rle_expand: {
    name: 'rle_expand',
    label: 'RLE Expand',
    category: 'tensor',
    description: 'Run-length decode into a tensor',
    arity: {
      type: 'ternary',
      args: [
        { name: 'runs', label: 'Runs', type: 'array', required: true },
        { name: 'shape', label: 'Shape', type: 'array', required: true },
        { name: 'dtype', label: 'Dtype', type: 'string', required: true },
      ],
    },
    help: {
      summary: 'Expand a flat [value, count, ...] list row-major',
      details:
        'The run lengths must sum to exactly the shape element count: a mask that decodes to the wrong size is a producer bug, and zero-filling the remainder would hide it.',
      returnType: 'any',
      examples: [
        {
          title: 'Two runs into a 2x2',
          rule: { to_list: [{ rle_expand: [[0, 2, 1, 2], [2, 2], 'u8'] }] },
          result: [[0, 0], [1, 1]],
        },
      ],
      seeAlso: ['scatter', 'one_hot'],
    },
    ui: { icon: 'repeat', shortLabel: 'rle', nodeType: 'operator' },
  },

  one_hot: {
    name: 'one_hot',
    label: 'One-Hot',
    category: 'tensor',
    description: 'Build a [len, depth] indicator matrix',
    arity: {
      type: 'ternary',
      args: [
        { name: 'indices', label: 'Indices', type: 'array', required: true },
        { name: 'depth', label: 'Depth', type: 'number', required: true },
        { name: 'dtype', label: 'Dtype', type: 'string', required: true },
      ],
    },
    help: {
      summary: 'Encode category indices as indicator rows',
      returnType: 'any',
      examples: [
        {
          title: 'Two categories out of three',
          rule: { to_list: [{ one_hot: [[0, 2], 3, 'u8'] }] },
          result: [[1, 0, 0], [0, 0, 1]],
        },
      ],
      notes: ['An index outside 0..depth leaves its row all-zero'],
      seeAlso: ['scatter', 'argmax'],
    },
    ui: { icon: 'binary', shortLabel: 'one_hot', nodeType: 'operator' },
  },

  stack: {
    name: 'stack',
    label: 'Stack',
    category: 'tensor',
    description: 'Join equal-shaped tensors along a new axis',
    arity: {
      type: 'binary',
      args: [
        { name: 'tensors', label: 'Tensors', type: 'array', required: true },
        { name: 'axis', label: 'Axis', type: 'number', required: true },
      ],
    },
    help: {
      summary: 'Join along a new axis, raising the rank by one',
      returnType: 'any',
      examples: [
        {
          title: 'Two vectors into a matrix',
          rule: {
            to_list: [
              { stack: [[{ tensor: [[1, 2], 'u8'] }, { tensor: [[3, 4], 'u8'] }], 0] },
            ],
          },
          result: [[1, 2], [3, 4]],
        },
      ],
      notes: ['Every tensor must share a shape and a dtype'],
      seeAlso: ['concat', 'unstack'],
    },
    ui: { icon: 'layers', shortLabel: 'stack', nodeType: 'operator' },
  },

  concat: {
    name: 'concat',
    label: 'Concat',
    category: 'tensor',
    description: 'Join tensors along an existing axis',
    arity: {
      type: 'binary',
      args: [
        { name: 'tensors', label: 'Tensors', type: 'array', required: true },
        { name: 'axis', label: 'Axis', type: 'number', required: true },
      ],
    },
    help: {
      summary: 'Join along an existing axis, keeping the rank',
      returnType: 'any',
      examples: [
        {
          title: 'Append along axis 0',
          rule: {
            to_list: [
              { concat: [[{ tensor: [[1, 2], 'u8'] }, { tensor: [[3], 'u8'] }], 0] },
            ],
          },
          result: [1, 2, 3],
        },
      ],
      notes: ['Shapes must match on every axis but the joined one'],
      seeAlso: ['stack', 'unstack'],
    },
    ui: { icon: 'git-merge', shortLabel: 'concat', nodeType: 'operator' },
  },

  unstack: {
    name: 'unstack',
    label: 'Unstack',
    category: 'tensor',
    description: 'Split along an axis into an array of tensors',
    arity: {
      type: 'binary',
      args: [
        { name: 'tensor', label: 'Tensor', type: 'any', required: true },
        { name: 'axis', label: 'Axis', type: 'number', required: true },
      ],
    },
    help: {
      summary: 'The inverse of stack: split and drop the axis',
      returnType: 'array',
      examples: [
        {
          title: 'Rows of a matrix',
          rule: {
            map: [
              { unstack: [{ tensor: [[[1, 2], [3, 4]], 'u8'] }, 0] },
              { to_list: [{ val: [] }] },
            ],
          },
          result: [[1, 2], [3, 4]],
        },
      ],
      seeAlso: ['stack', 'gather'],
    },
    ui: { icon: 'git-branch', shortLabel: 'unstack', nodeType: 'operator' },
  },

  reshape: {
    name: 'reshape',
    label: 'Reshape',
    category: 'tensor',
    description: 'Reinterpret the same bytes under a new shape',
    arity: {
      type: 'binary',
      args: [
        { name: 'tensor', label: 'Tensor', type: 'any', required: true },
        { name: 'shape', label: 'Shape', type: 'array', required: true },
      ],
    },
    help: {
      summary: 'A new header over the same payload — the one zero-copy operator',
      details:
        'The element count must match exactly; there is no inferred -1 dimension.',
      returnType: 'any',
      examples: [
        {
          title: 'A flat list into a matrix',
          rule: { to_list: [{ reshape: [{ tensor: [[1, 2, 3, 4], 'u8'] }, [2, 2]] }] },
          result: [[1, 2], [3, 4]],
        },
      ],
      seeAlso: ['transpose', 'tensor'],
    },
    ui: { icon: 'box', shortLabel: 'reshape', nodeType: 'operator' },
  },

  transpose: {
    name: 'transpose',
    label: 'Transpose',
    category: 'tensor',
    description: 'Permute the axes',
    arity: {
      type: 'range',
      min: 1,
      max: 2,
      args: [
        { name: 'tensor', label: 'Tensor', type: 'any', required: true },
        { name: 'perm', label: 'Permutation', type: 'array', required: false },
      ],
    },
    help: {
      summary: 'Reorder the axes; defaults to a full reversal',
      returnType: 'any',
      examples: [
        {
          title: 'Transpose a 2x2',
          rule: { to_list: [{ transpose: [{ tensor: [[[1, 2], [3, 4]], 'u8'] }] }] },
          result: [[1, 3], [2, 4]],
        },
      ],
      seeAlso: ['reshape', 'gather'],
    },
    ui: { icon: 'arrow-up', shortLabel: 'transpose', nodeType: 'operator' },
  },

  pad: {
    name: 'pad',
    label: 'Pad',
    category: 'tensor',
    description: 'Grow every axis by a leading and trailing margin',
    arity: {
      type: 'range',
      min: 3,
      max: 4,
      args: [
        { name: 'tensor', label: 'Tensor', type: 'any', required: true },
        { name: 'before', label: 'Before', type: 'array', required: true },
        { name: 'after', label: 'After', type: 'array', required: true },
        { name: 'value', label: 'Fill', type: 'any', required: false },
      ],
    },
    help: {
      summary: 'Add a margin around the tensor, filled with `value` (0 by default)',
      returnType: 'any',
      examples: [
        {
          title: 'One before, two after',
          rule: { to_list: [{ pad: [{ tensor: [[1, 2], 'u8'] }, [1], [2]] }] },
          result: [0, 1, 2, 0, 0],
        },
      ],
      notes: ['`before` and `after` take one entry per axis'],
      seeAlso: ['crop'],
    },
    ui: { icon: 'box', shortLabel: 'pad', nodeType: 'operator' },
  },

  crop: {
    name: 'crop',
    label: 'Crop',
    category: 'tensor',
    description: 'Cut a sub-block out of a tensor',
    arity: {
      type: 'ternary',
      args: [
        { name: 'tensor', label: 'Tensor', type: 'any', required: true },
        { name: 'offset', label: 'Offset', type: 'array', required: true },
        { name: 'shape', label: 'Shape', type: 'array', required: true },
      ],
    },
    help: {
      summary: 'The inverse of pad: take a window, which must lie inside the input',
      returnType: 'any',
      examples: [
        {
          title: 'The middle two of four',
          rule: { to_list: [{ crop: [{ tensor: [[1, 2, 3, 4], 'u8'] }, [1], [2]] }] },
          result: [2, 3],
        },
      ],
      seeAlso: ['pad', 'gather'],
    },
    ui: { icon: 'box', shortLabel: 'crop', nodeType: 'operator' },
  },

  cast: {
    name: 'cast',
    label: 'Cast',
    category: 'tensor',
    description: 'Convert a tensor to another dtype',
    arity: {
      type: 'binary',
      args: [
        { name: 'tensor', label: 'Tensor', type: 'any', required: true },
        { name: 'dtype', label: 'Dtype', type: 'string', required: true },
      ],
    },
    help: {
      summary: 'The one cross-dtype conversion, and deliberately lossy',
      details:
        'Narrowing saturates rather than wrapping (300 to u8 is 255, not 44) and NaN becomes 0. ' +
        'Values pivot through f64 internally, so i64 / u64 elements above 2^53 lose their low bits.',
      returnType: 'any',
      examples: [
        {
          title: 'Saturating narrowing',
          rule: { to_list: [{ cast: [{ tensor: [[1.7, 300, -5], 'f64'] }, 'u8'] }] },
          result: [1, 255, 0],
        },
      ],
      seeAlso: ['normalize', 'dtype'],
    },
    ui: { icon: 'type', shortLabel: 'cast', nodeType: 'operator' },
  },

  normalize: {
    name: 'normalize',
    label: 'Normalize',
    category: 'tensor',
    description: '(x - mean) * scale, producing f32',
    arity: {
      type: 'range',
      min: 2,
      max: 3,
      args: [
        { name: 'tensor', label: 'Tensor', type: 'any', required: true },
        { name: 'mean', label: 'Mean', type: 'number', required: true },
        { name: 'scale', label: 'Scale', type: 'number', required: false },
      ],
    },
    help: {
      summary: 'Shift and scale into the float range a model expects',
      details:
        'The output dtype is always f32. `scale` defaults to 1, so the two-argument form is a plain mean subtraction. ' +
        'Per-channel normalization is unstack + normalize + stack.',
      returnType: 'any',
      examples: [
        {
          title: 'Pixel range to roughly -64..64',
          rule: { to_list: [{ normalize: [{ tensor: [[0, 255], 'u8'] }, 127.5, 0.5] }] },
          result: [-63.75, 63.75],
        },
      ],
      seeAlso: ['cast', 'unstack'],
    },
    ui: { icon: 'calculator', shortLabel: 'normalize', nodeType: 'operator' },
  },

  argmax: {
    name: 'argmax',
    label: 'Argmax',
    category: 'tensor',
    description: 'Index of the largest element along an axis',
    arity: {
      type: 'binary',
      args: [
        { name: 'tensor', label: 'Tensor', type: 'any', required: true },
        { name: 'axis', label: 'Axis', type: 'number', required: true },
      ],
    },
    help: {
      summary: 'Reduce an axis to the index of its maximum, as plain JSON',
      details:
        'Returns nested arrays of indices, or a bare number when the input is 1-d — an index is something a rule goes on to compare, so it comes back as ordinary JSON rather than a tensor. Ties go to the first occurrence, and NaN never wins.',
      returnType: 'any',
      examples: [
        {
          title: 'Winning class per row',
          rule: { argmax: [{ tensor: [[[1, 9], [7, 3]], 'f32'] }, 1] },
          result: [1, 0],
        },
      ],
      seeAlso: ['to_list', 'one_hot'],
    },
    ui: { icon: 'search', shortLabel: 'argmax', nodeType: 'operator' },
  },

  gather: {
    name: 'gather',
    label: 'Gather',
    category: 'tensor',
    description: 'Select slices along an axis, in the order given',
    arity: {
      type: 'range',
      min: 2,
      max: 3,
      args: [
        { name: 'tensor', label: 'Tensor', type: 'any', required: true },
        { name: 'indices', label: 'Indices', type: 'array', required: true },
        { name: 'axis', label: 'Axis', type: 'number', required: false },
      ],
    },
    help: {
      summary: 'Reorder and resample along one axis; axis defaults to 0',
      returnType: 'any',
      examples: [
        {
          title: 'Pick and repeat',
          rule: { to_list: [{ gather: [{ tensor: [[10, 20, 30], 'u8'] }, [2, 0, 2]] }] },
          result: [30, 10, 30],
        },
      ],
      notes: [
        'Negative indices count from the end',
        'Every index must be in range — unlike scatter, dropping one would change the output shape',
      ],
      seeAlso: ['crop', 'unstack'],
    },
    ui: { icon: 'list', shortLabel: 'gather', nodeType: 'operator' },
  },

  to_list: {
    name: 'to_list',
    label: 'To List',
    category: 'tensor',
    description: 'Expand a tensor into nested JSON arrays',
    arity: {
      type: 'unary',
      args: [{ name: 'tensor', label: 'Tensor', type: 'any', required: true }],
    },
    help: {
      summary: 'The general escape hatch, and expensive by design',
      details:
        'The one operator that turns a compact buffer back into one JSON node per element. Reach for argmax, shape or a comparison first.',
      returnType: 'any',
      examples: [
        {
          title: 'Back to plain arrays',
          rule: { to_list: [{ tensor: [[[1, 2], [3, 4]], 'i32'] }] },
          result: [[1, 2], [3, 4]],
        },
      ],
      seeAlso: ['tensor', 'argmax', 'shape'],
    },
    ui: { icon: 'list', shortLabel: 'to_list', nodeType: 'operator' },
  },

  shape: {
    name: 'shape',
    label: 'Shape',
    category: 'tensor',
    description: "A tensor's dimensions",
    arity: {
      type: 'unary',
      args: [{ name: 'tensor', label: 'Tensor', type: 'any', required: true }],
    },
    help: {
      summary: 'Read the shape as an array of numbers',
      returnType: 'array',
      examples: [
        {
          title: 'Dimensions of a 1x3',
          rule: { shape: [{ tensor: [[[1, 2, 3]], 'u8'] }] },
          result: [1, 3],
        },
      ],
      seeAlso: ['dtype', 'reshape'],
    },
    ui: { icon: 'tag', shortLabel: 'shape', nodeType: 'operator' },
  },

  dtype: {
    name: 'dtype',
    label: 'Dtype',
    category: 'tensor',
    description: "A tensor's element type",
    arity: {
      type: 'unary',
      args: [{ name: 'tensor', label: 'Tensor', type: 'any', required: true }],
    },
    help: {
      summary: 'Read the dtype as its wire name',
      details: `One of: ${DTYPES}. Exactly what tensor, zeros, full and cast accept back.`,
      returnType: 'string',
      examples: [
        {
          title: 'The dtype of an f32 tensor',
          rule: { dtype: [{ tensor: [[1, 2], 'f32'] }] },
          result: 'f32',
        },
      ],
      seeAlso: ['shape', 'cast'],
    },
    ui: { icon: 'type', shortLabel: 'dtype', nodeType: 'operator' },
  },
};
