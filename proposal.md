# datalogic-rs v5 — Outstanding Review Items

Original review scope: 9 P0 + ~25 P1 + ~15 P2 items, organised into 7 batches.
**All seven batches landed.** What's left below is the small set of items that
were *deliberately deferred* (not skipped) — each needs care that didn't fit
into a "no-behavior-change" batch and warrants a focused follow-up.

> **Compat boundary.** v5 is in draft. v5-internal API can change freely
> between drafts. The only back-compat surface that must be preserved is
> the v4 wrapper layer in `src/compat.rs`.

> **Line-number anchors drift** as commits land — re-find the symbol before editing.

---

## Deferred items

### D1 — `TraceCollector` raw-pointer escape hatch on `ContextStack`

`tracer: Option<NonNull<TraceCollector>>` with manual
`unsafe { ptr.as_ptr().as_mut() }` (`arena/context.rs:135,219,237,247`).
Sound today — tracer reads happen synchronously within one `evaluate()`
call — but fragile against re-entrant futures.

**Resolution paths:**
- (a) Add a `'tr` lifetime parameter to `ContextStack<'a, 'tr>`, paid only
  when the `trace` feature is enabled. Propagation cost across every
  call site that touches `ContextStack`.
- (b) Move tracer state into a side-buffer keyed off a session id. Extra
  map lookup per traced step.

Was originally **B5.6**.

---

### D2 — Three near-clone arithmetic fold loops

`variadic_fold` (`operators/arithmetic/helpers.rs`),
`subtract_variadic` (`operators/arithmetic/basic.rs`),
`one_arg_array_fold` (`operators/arithmetic/basic.rs`).

The three share an int-fast-path-with-f64-fallback pattern but differ in
their int-coercion strategy:

- `variadic_fold` uses strict `as_i64()` (native int only).
- `subtract_variadic` uses `as_i64() OR try_coerce_to_integer_cfg`.
- `one_arg_array_fold` uses `try_coerce_to_integer_cfg` only.

Unifying on a single coercion path risks behavior changes around overflow
boundaries (e.g. `{"+": [1, "9223372036854775808"]}`) that the test
suite may not cover. Needs equivalence tests at each coercion boundary
before unification.

Was originally **B6.1**. Estimated savings: ~150 LoC.

---

### D3 — `scope_level: u32` ↔ `get_at_level(isize)` type mismatch

`arena/context.rs::get_at_level` accepts `isize` and immediately calls
`level.unsigned_abs()`. The negative-handling is load-bearing — every
isize caller relies on `abs` semantics rather than getting a `u32`-cast
overflow.

**Resolution path:** introduce a `Level` newtype that exposes the same
`abs` semantics in its constructor. Update both `u32` and `isize` callers
to construct `Level::new(...)`.

Was originally **B7 / scope_level**.

---

### D4 — `IntoOperatorBox` sealed-trait scaffold

30 lines of sealed scaffolding (`lib.rs:197-226`) for two impls:
`T: CustomOperator` and `Box<dyn CustomOperator>`.

**Tradeoff:** dropping the `Box<dyn CustomOperator>` impl simplifies the
sealed scaffold but breaks callers who construct boxes upfront (rare
pattern but observable). The fix is `add_operator_boxed` for the rare
path. Mark as a v5.0 → v5.1 break or accept the current scaffold as the
cost of the convenience.

Was originally **B7 / IntoOperatorBox**.

---

### D5 — `if … return; if … return;` waterfalls

`comparison.rs:412-484`, `variable.rs:127-153`, `string.rs:55-77`. Linear
sequence of guard-and-return arms that read better as `match` arms or
extracted stage functions.

`variable.rs` is already half-moved with `resolve_metadata_hint` /
`resolve_reduce_hint` (P0.7) — finish the same pattern for the other two
files. Need careful diff review to confirm fall-through equivalence
since the guards' order matters.

Was originally **B7 / if-return waterfalls**.

---

### D6 — `extract_opt_i64_arena` literal-fast-path dedup

The `if literal { fast } else { dispatch + coerce }` pattern repeats
across `operators/slice.rs:94-115` and
`operators/string.rs:55-77,69-77`. A small helper would unify the
coercion logic.

Risk: each site's coercion semantics may differ subtly (similar to D2).
Needs per-site equivalence checks.

Was originally **B7 / extract_opt_i64_arena**.

---

### D7 — `slice_chars` index-list duplication

`operators/array/slice.rs:120-150,152-203` open-codes the same forward /
backward index-list construction twice (once for arrays, once for
strings). Build the index list once and have both consume it.

Algorithm change — needs care around the `saturating_add` backstops on
the string path.

Was originally **B7 / slice_chars**.

---

### D8 — `evaluate_format_date` chained `.replace()`

`operators/datetime.rs:141-149` chains six `String::replace` calls to
convert JSONLogic format spec to chrono format. Compile-time format-
string transform (or arena-allocate the result string) would skip the
six allocations per call.

Pure perf optimisation — defer to a profiler-driven cleanup.

Was originally **B7 / evaluate_format_date**.

---

### D9 — `evaluate_array_literal` / `evaluate_structured_object` per-child allocation

`engine/dispatch.rs:333-358` allocates every child as a fresh `DataValue`
for nested literals. For deeply-nested literal trees this is N allocations
where 1 would do (if a `DataValue::Array(&[&Dv])` shape existed).

Defer until either (a) `datavalue` adds the slice-of-refs shape, or
(b) profiling shows the dominant pattern in real workloads.

Was originally **B7 / evaluate_array_literal**.

---

### D10 — `OPCODE_NAMES` ↔ `OpCode::as_str` duplicate source-of-truth

`opcode.rs:168-273` (table) and `:301-398` (match) list every variant
twice. Round-trip test (`:412`) catches divergence today, but a future
maintainer adding a variant has to remember both arms.

**Resolution path:** macro-generate both arms from a single list. Bigger
refactor — touches every variant declaration. The round-trip test is a
sufficient guard for v5.0; revisit when the OpCode list churns.

Was originally **B7 / OPCODE_NAMES**.

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
