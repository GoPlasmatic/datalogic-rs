# Tensor Operators

JSON has no tensor. Anything that marshals JSON into a model's inputs and
its outputs back into JSON — an ONNX Runtime session, an edge request
encoder, an RL observation buffer — needs one value that is neither a
scalar nor a JSON array. The `tensor` feature adds it: an opaque
n-dimensional typed buffer (a dtype, a shape, and one row-major
contiguous byte payload), plus twenty operators that make, reshape, and
read one back.

> **Feature flags (Rust crate).** All tensor operators require the
> `tensor` feature. `tensor-half` additionally enables `f16` / `bf16`
> elements. Every language binding enables `tensor`. See the
> [feature table](overview.md#which-operators-need-which-cargo-feature).

## What this is not

There is **no arithmetic** here — no matmul, no convolution, no
element-wise add. The family marshals data and nothing else. Every
operator's cost is proportional to the data it moves, which is what keeps
it honest to price; an operator whose work is not proportional to its data
(a matmul reads 2n² elements and does n³ multiplies) would break that
property. Model execution belongs in the runtime you are feeding, not in
the rules engine.

## The wire form

A tensor crosses the JSON boundary as a single-key tagged object:

```json
{ "tensor": { "dtype": "f32", "shape": [2, 2], "data": "AACAPwAAAEAAAEBAAACAQA==" } }
```

`data` is the raw little-endian payload in standard base64. This is
simultaneously the operator call, the form the engine emits, and the form
the decoder accepts — so a serialized tensor pasted back into a rule
evaluates to the tensor it came from, and a tensor stored in your input
data decodes with `{"tensor": [{"val": "..."}]}`.

Because the text-returning bindings (WASM, Node, C and everything built on
it) hand back JSON, tensors flow through them unchanged with no FFI
change.

## dtypes

`bool`, `i8`, `u8`, `i16`, `u16`, `i32`, `u32`, `i64`, `u64`, `f16`,
`bf16`, `f32`, `f64`. Names are accepted case-insensitively, so
safetensors-style `"F32"` works.

The **shape operators** (`stack`, `concat`, `unstack`, `reshape`,
`transpose`, `pad`, `crop`, `gather`) move raw cells and never interpret
one, so they work on every dtype including `f16` / `bf16` with no extra
feature. `zeros` does too — an all-zero buffer is a valid value of every
dtype. The **element operators** need `tensor-half` before they will
touch `f16` / `bf16`, and answer an unsupported-dtype error otherwise.

---

## tensor

Build a tensor, or pass one through. The family's entry point.

**Syntax:**
```json
{ "tensor": [value, dtype] }
{ "tensor": [value] }
```

**Arguments:**
- `value` — nested JSON arrays (or a bare scalar for a 0-d tensor), the
  tagged wire form, or an existing tensor
- `dtype` — required for the nested-array form; the shape is inferred
  from the nesting

**Returns:** A tensor.

An element that does not fit the declared dtype is an error, never a
truncation and never a manufactured infinity.

**Examples:**

```json
{ "tensor": [[[1, 2], [3, 4]], "f32"] }
// Result: a 2x2 f32 tensor

{ "tensor": [{ "val": "prediction" }] }
// Decodes a tagged tensor out of the input data

{ "reshape": [{ "tensor": [[1, 2, 3, 4], "u8"] }, [2, 2]] }
// A flat list plus a shape
```

---

## zeros / full

Build a tensor of a declared shape.

**Syntax:**
```json
{ "zeros": [shape, dtype] }
{ "full": [shape, dtype, value] }
```

**Examples:**

```json
{ "zeros": [[2, 3], "i32"] }        // 2x3 of 0
{ "full": [[2, 2], "f32", 1.5] }    // 2x2 of 1.5
```

---

## scatter

Sparse writes into an otherwise-zero tensor.

**Syntax:**
```json
{ "scatter": [points, shape, dtype, value] }
```

**Arguments:**
- `points` — array of coordinate arrays. `[i, j]` writes `value`
  (1 by default); `[i, j, v]` writes `v`
- `value` — optional default written value

Out-of-range points are **dropped**, not rejected: the usual producer is a
detector emitting boxes in source coordinates that may fall outside the
target grid.

**Examples:**

```json
{ "scatter": [[[0, 1], [1, 0]], [2, 2], "u8"] }
// [[0, 1], [1, 0]]
```

---

## rle_expand

Run-length decode into a tensor, row-major.

**Syntax:**
```json
{ "rle_expand": [runs, shape, dtype] }
```

`runs` is the flat `[v0, n0, v1, n1, …]` pairing. The run lengths must sum
to exactly the shape's element count — a mask that decodes to the wrong
size is a producer bug, and zero-filling the remainder would hide it.

**Examples:**

```json
{ "rle_expand": [[0, 2, 1, 2], [2, 2], "u8"] }
// [[0, 0], [1, 1]]
```

---

## one_hot

Build a `[len, depth]` indicator matrix.

**Syntax:**
```json
{ "one_hot": [indices, depth, dtype] }
```

An index outside `0..depth` leaves its row all-zero.

**Examples:**

```json
{ "one_hot": [[0, 2], 3, "u8"] }
// [[1, 0, 0], [0, 0, 1]]
```

---

## stack / concat / unstack

Join and split along an axis. `stack` introduces a **new** axis (so the
result is one rank higher); `concat` joins along an **existing** one.
`unstack` is the inverse of `stack`. Negative axes count from the end.

**Syntax:**
```json
{ "stack": [tensors, axis] }
{ "concat": [tensors, axis] }
{ "unstack": [tensor, axis] }
```

`stack` requires identical shapes; `concat` requires them to agree on
every axis but the joined one. Both require a common dtype. `unstack`
returns an array of tensors.

**Examples:**

```json
{ "stack": [[{"tensor": [[1, 2], "u8"]}, {"tensor": [[3, 4], "u8"]}], 0] }
// A 2x2 tensor: [[1, 2], [3, 4]]
```

---

## reshape

Reinterpret the same bytes under a new shape. The only operator here that
copies nothing — the payload is shared with the input.

**Syntax:**
```json
{ "reshape": [tensor, shape] }
```

The element count must match exactly; there is no inferred `-1`
dimension.

---

## transpose

Permute the axes. `perm` defaults to a full reversal, so a 2-d transpose
needs no second argument.

**Syntax:**
```json
{ "transpose": [tensor] }
{ "transpose": [tensor, perm] }
```

---

## pad / crop

Grow or cut every axis. `crop` is the inverse of `pad`.

**Syntax:**
```json
{ "pad": [tensor, before, after, value] }
{ "crop": [tensor, offset, shape] }
```

`before` and `after` take one entry per axis. `value` defaults to 0.
A crop window must lie inside the input.

**Examples:**

```json
{ "pad": [{ "tensor": [[1, 2], "u8"] }, [1], [2]] }
// [0, 1, 2, 0, 0]
```

---

## gather

Select slices along an axis, in the order given — so it both reorders and
resamples.

**Syntax:**
```json
{ "gather": [tensor, indices, axis] }
```

`axis` defaults to 0. Negative indices count from the end. Every index
must be in range: unlike `scatter`, dropping one would silently change the
output shape.

---

## cast

Convert to another dtype. The family's one cross-dtype conversion, and
deliberately lossy.

**Syntax:**
```json
{ "cast": [tensor, dtype] }
```

Narrowing **saturates** rather than wrapping (`300` to `u8` is `255`, not
`44`), and `NaN` becomes 0. Values are widened through `f64` internally,
so `i64` / `u64` elements above 2^53 lose their low bits; casting to the
dtype a tensor already has is a no-op and does not round-trip.

---

## normalize

`(x − mean) × scale`, always producing `f32`.

**Syntax:**
```json
{ "normalize": [tensor, mean, scale] }
```

`scale` defaults to 1, so the two-argument form is a plain mean
subtraction. The output dtype is fixed because that is what the operation
is for: turning integer sensor or pixel data into the float range a model
expects. `mean` and `scale` are scalars — per-channel normalization is
`unstack` + `normalize` + `stack`.

**Examples:**

```json
{ "normalize": [{ "tensor": [[0, 255], "u8"] }, 127.5, 0.5] }
// [-63.75, 63.75]
```

---

## argmax

Index of the largest element along an axis, as plain JSON.

**Syntax:**
```json
{ "argmax": [tensor, axis] }
```

**Returns:** Nested JSON arrays of indices, or a bare number when the
input is 1-d — an index is something a rule goes on to compare and branch
on, so it comes back as ordinary JSON rather than a tensor.

Ties go to the first occurrence, and `NaN` never wins.

**Examples:**

```json
{ "argmax": [{ "tensor": [[[1, 9], [7, 3]], "f32"] }, 1] }
// [1, 0]
```

---

## to_list

Expand into plain nested JSON arrays.

**Syntax:**
```json
{ "to_list": [tensor] }
```

The general escape hatch, and expensive by design: this is the one
operator that turns a compact buffer back into one JSON node per element.
Reach for `argmax`, `shape` or a comparison first.

---

## shape / dtype

Read the header.

**Syntax:**
```json
{ "shape": [tensor] }
{ "dtype": [tensor] }
```

`shape` returns an array of numbers; `dtype` returns the wire name, which
is exactly what `tensor`, `zeros`, `full` and `cast` accept back.

---

## How a tensor behaves as a value

| Context | Behavior |
|---|---|
| `type` | `"tensor"` |
| Truthiness | `true` unless the element count is 0, matching the empty-array rule. A 0-d tensor holds one element, so it is truthy |
| `==` / `!=` | Structural between two tensors: dtype, shape, and payload. Against any other type it is incompatible, which follows the `loose_equality_errors` config exactly as an object does |
| `===` | Structural, with no coercion at all |
| Numeric coercion | None. A one-element tensor does **not** coerce the way a one-element array does — `to_list` is the explicit way out |
| `sort` | Tensors rank after objects; two tensors order by dtype, then shape, then payload bytes |
| `map`, array elements, object fields | Passed through untouched |

Tensor operators are never constant-folded and never memoized by the CSE
pass, even though they are pure functions of their arguments: folding
`{"zeros": [[128, 128], "f32"]}` would bake a 64 KB literal into the
compiled rule.
