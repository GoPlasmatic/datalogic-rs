# datalogic-rs v5 — Outstanding Review Items

Original review scope: 9 P0 + ~25 P1 + ~15 P2 items, organised into 7 batches.
**All seven batches landed.** A post-Batch-7 investigation (see commit
`ea0d577`) further triaged the 10 originally-deferred items: 5 landed
either fully or as minimal-viable improvements, 5 remain genuinely deferred
(behaviour-preservation cost too high, premise depends on upstream changes,
or pure-perf without profile data).

> **Compat boundary.** v5 is in draft. v5-internal API can change freely
> between drafts. The only back-compat surface that must be preserved is
> the v4 wrapper layer in `src/compat.rs`.

> **Line-number anchors drift** as commits land — re-find the symbol before editing.

---

## Done in the deferred-item follow-up (`ea0d577`)

| Id | Item | Outcome |
|---|---|---|
| D1-min | `TraceCollector` unsafe blocks | Three `unsafe` sites → one `with_tracer(\|c\| ...)` helper; switched to `NonNull::as_mut`. Full lifetime-parameter refactor explicitly rejected (would propagate `'tr` through 225 sites + the `CustomOperator` public trait). |
| D2 | Arithmetic fold loops | `FoldState` + `step` + `finalize` extracted. Each caller keeps its own coercion strategy (strict vs permissive). –30 LoC, zero behavior change. |
| D4 | `IntoOperatorBox` scaffold | Removed. `add_operator<T: CustomOperator + 'static>` direct + new `add_operator_boxed` for the rare pre-boxed path. |
| D6 | substr literal-fast-path | `substr_arg_i64` helper for the two `string.rs` sites. Slice's `extract_opt_i64_arena` *not* folded — different error semantics. |
| D7 | `slice_chars` ↔ `slice_indices` | String slicing now consumes `slice_indices`. –50 LoC, incidentally fixes a latent `e < -len` boundary asymmetry. |

---

## Still deferred

### D3 — `scope_level: u32` ↔ `get_at_level(isize)` type mismatch

`arena/context.rs::get_at_level` takes `isize` and immediately calls
`level.unsigned_abs()`. The negative-handling is load-bearing — every
isize caller relies on `abs` semantics rather than getting a `u32`-cast
overflow. Fix needs a `Level` newtype that exposes the abs semantics
in its constructor; ~50–100 LoC for stylistic gain. Not urgent.

### D5 — `if-return` waterfalls

Investigation concluded these aren't really waterfalls. `comparison.rs`
has single guards; `variable.rs` is already half-staged with extracted
helpers (`resolve_metadata_hint` / `resolve_reduce_hint`); the
`string.rs` "waterfall" is the substr literal-fast-path that D6
already collapsed. Reshape would add indirection without removing
branches. Recommend **closing as won't-fix** unless a fresh hot-spot
emerges.

### D8 — `evaluate_format_date` chained `.replace()`

`operators/datetime.rs::jsonlogic_to_chrono_format` chains six
`String::replace` calls per call. Cold path (per-rule, not per-row).
Pure perf optimisation; defer to a profiler-driven cleanup with
real workload data.

### D9 — `evaluate_array_literal` per-child allocation

`engine/dispatch.rs:evaluate_array_literal` / `evaluate_structured_object`
build a `bumpalo::Vec<DataValue>` and copy children by value.
Investigation confirmed the proposal's hint of an `Array(&[&DataValue])`
shape is *not* implementable in-tree — `datavalue::DataValue::Array`
holds `&[DataValue<'a>]`, not `&[&DataValue<'a>]`. Would require an
upstream `datavalue` API change. Defer unless that lands.

### D10 — `OPCODE_NAMES` ↔ `OpCode::as_str` duplicate source-of-truth

`opcode.rs:168-273` (table) and `:301-398` (match) list every variant
twice. The recent perf commit `a702b65` deliberately replaced the
table-scan in `as_str` with a direct `match` for jump-table codegen;
a macro that generates both arms must preserve that — adds opacity for
new-operator contributors. The round-trip test at `:412` is a
sufficient guard for v5.0; revisit when the OpCode list churns.

---

## Anti-findings — preserve in v5

Things the codebase already does well; resist drive-by changes.

- **Two-tier eval design** (`Engine::evaluate` arena-mode + `Session` owned-mode) — clean separation of power-user and convenience paths.
- **`EvalInput` sealed trait** — five impls, parse-fallibility threaded through one method, no leaks.
- **`Error::resolved_path`** — cheap path-of-ids on the hot error path, lazy resolve to structured `PathStep` only when consumed.
- **`Logic` `Sync` impl with the documented `_static_arena` invariant** — exactly the right level of unsafe-block hygiene.
- **`kind_tag()` + `#[non_exhaustive] ErrorKind`** — forward-compatible serialization.
- **Outlined `literal_fallback`** (`engine/mod.rs` `#[cold] #[inline(never)]`) — thoughtful hot-path optimisation.
- **`OpCode` discriminants are stable `#[repr(u8)]` with feature-gated holes** (`opcode.rs:51-160`) — keeps numeric IDs stable across feature toggles.
- **Operator dispatch macro split into `simple` / `iter` / `with_kind` / `other`** (`engine/dispatch.rs`).
- **The line drawn between `&'a [CompiledNode]` args (most ops) and pre-parsed structs (`Var` / `Exists` / `Missing` / `MissingSome` / `InvalidArgs`)** — clean and intentional.
- **Free-function operator style + `CustomOperator` trait only for user-supplied ops** — keep. Trait dispatch for built-ins would lose the dispatch macro's codegen.
- **The optimizer's pass split + fixpoint loop** (`compile/optimize/mod.rs`).
- **`OpCode::FromStr` linear scan** — call site is correctly identified as cold.
- **`compile::missing` ↔ `operators::missing` split** (compile-time literal pre-parse vs runtime evaluation).
- **`OpCode::Concat` ↔ `cat` operator string asymmetry** — opcode named for in-dispatcher readability; the operator string follows the canonical JSONLogic spec. Two surfaces, two audiences. Don't unify.
- **`for_each_iter_array` / `for_each_iter_object` helpers + `ControlFlow`-based short-circuit** (P0.6) — natural callback shape for filter/map/quantifiers without forcing reduce into the same mould.
- **Single SAFETY-noted `arena::value::reborrow_arena_value`** (B6.3) — every `unsafe` `&'a` lifetime extension routes through one audited point.
- **`ContextStack::with_tracer` helper** (D1-min) — single SAFETY site for the tracer raw-pointer dereference; the raw pointer is the *right* design for a `ContextStack` that must stay free of an extra lifetime parameter.
- **`FoldState` accumulator + per-caller coercion strategy** (D2) — int/float/all_int state machine shared across `+`/`-`/`*` variadic forms while keeping each operator's coercion rules distinct.
