# datalogic-rs v5 — Code Review Plan

Original review: 9 P0 items, ~25 P1 items, ~15 P2 items. All P0 done; this doc lays out batches for the remainder.

> **Compat boundary.** v5 is in draft. v5-internal API can change freely between drafts. The only back-compat surface that must be preserved is the v4 wrapper layer in `src/compat.rs` (the `LegacyApi` trait, the deprecated `ArenaOperator` / `ArenaValue` / `ArenaContextStack` aliases, and the deprecated `with_*` constructors).

> **Line-number anchors drift** as commits land. Each item points to the right region of the file, but a maintainer should re-find the symbol before editing.

---

## Done

| # | Item | Commit |
|---|---|---|
| P0.1 | `evaluate_value` → `evaluate_serde` (return-type symmetry on `Engine` & `Scratch`) | `949d7ae` |
| P0.2 | `Scratch::eval*` → `evaluate*` (verb unification) | `949d7ae` |
| P0.3 | datavalue re-export — per-type `pub use` → `pub use datavalue;` (one explicit hop) | `164c77b` |
| P0.4 | `pub mod arena` → `mod arena`; `data_to_json_string` re-exported at root | `164c77b` |
| P0.5 | `operators::*` submodules + `truthy_owned` / `check_invalid_args_marker` / `FastPredicate` → `pub(crate)` | `164c77b` |
| P0.6 | iter helpers — `IterGuard+step_*+run_iter_body` factored into `for_each_iter_array/object` | `8e3599e` |
| P0.7 | `evaluate_val` 193-line monolith → 22-line dispatcher + 3 helpers; `array_get` unsafe reborrow eliminated | `3a62e64` |
| P0.8 | `compare_equals_primitive` collapsed into `compare_equals` (–70 LoC) | `774636c` |
| P0.9 | trait `Operator` → `CustomOperator` (resolves `Operator`/`OpCode` confusion) | `133aa0a` |

**Side-effects on the original P1/P2 list:**
- "unsafe reborrow appears in 5 places" — 3 sites (filter/map/quantifiers) consolidated into `for_each_iter_object`'s single SAFETY-noted reborrow; 1 site (`variable.rs::array_get`) eliminated. **2 left** at `arena/value/lookup.rs:90,111` (covered by **B6.3** below).
- "`pub use FastPredicate` / `IterArgKind` leak" — closed by P0.5.
- Items below using language like "Engine::evaluate_value" / "Operator trait" / "Scratch::eval" have been retitled to the post-rename names.

---

## Remaining work — seven batches

Sized so each lands as one PR. Within a batch, items are independent and can be cherry-picked.

### Batch 1 — Public API hygiene  *(do before v5 release)* — ✅ resolved (20c4e47)

Visible in v5 docs. Foot-guns and friction. All items landed in one commit.

- **B1.1** ✅ — Dropped `Default for Error`. *(was `error.rs:310-314`)*
- **B1.2** ✅ — `Error::wrap` no-op when input is already `Error`. Uses `Option<E>` + `Any::downcast_mut` (no unsafe). Regression test in `error::tests::wrap_of_existing_error_is_noop`. *(`error.rs:181-211`)*
- **B1.3** ✅ — Dropped `PathStep #[non_exhaustive]`. *(`path.rs:15`)*
- **B1.4** ✅ — `TraceCollector` → `pub(crate)`, removed from crate-root re-exports. `TracedResult` re-export gated behind `cfg(feature = "compat")`; `TracedRun` is the v5 shape. *(`lib.rs:122-125`, `trace.rs:351`)*
- **B1.5** ✅ — Dropped `EvaluationConfig::with_*` setters. Tests/examples migrated to `EvaluationConfig { field: x, ..Default::default() }`. *(`config.rs:123-133`)*
- **B1.6** ✅ — Dropped `Engine::compile_arc`. Doctest in `node.rs` updated to `Arc::new(engine.compile(...))`. *(was `engine/mod.rs:253`)*
- **B1.7** ✅ — Rewrote `Engine::builder` doc — positions builder for non-default cases, `new` for stock engines. *(`engine/mod.rs:78`)*

### Batch 2 — Final v5 renames  *(do before v5 release)* — ✅ resolved (df04a18)

All updates flow through `compat.rs` deprecation `note` strings only — the v4 wrappers themselves are unchanged.

- **B2.1** ✅ — `Scratch` → `Session`. Module `scratch.rs` → `session.rs`; `Engine::scratch()` → `Engine::session()`.
- **B2.2** ✅ — `IntoEvalData` → `EvalInput`. Method `into_eval_data` → `into_arena_value`. Module `eval_data.rs` → `eval_input.rs`.
- **B2.3** ✅ — `ContextStack` moved to `operator::ContextStack` (kept the name per user direction; only the path changed). New `src/operator.rs` module re-exports from `arena`.
- **B2.4** ✅ — `TruthyEvaluator::Custom` ungated; callback signature is now `Fn(&OwnedDataValue) -> bool` (was `&serde_json::Value`).
- ~~**B2.5**~~ — SKIPPED (intentional). Promoted to anti-findings: the opcode `Concat` is named for in-dispatcher readability; the operator string `cat` follows the JSONLogic spec.
- **B2.6** ✅ — `evaluate_compiled_var` / `evaluate_compiled_exists` → `evaluate_val_compiled` / `evaluate_exists_compiled`.

### Batch 3 — Module reshape  *(internal-only, low risk)* — ✅ resolved (1882247)

- **B3.1** ✅ — Deleted `src/datetime.rs`; 13 internal `crate::datetime::*` callers switched to `datavalue::*`.
- **B3.2** ✅ — Folded `constants.rs` into `error.rs`. `nan_error()` / `invalid_args()` are now associated `Error::nan()` / `Error::invalid_args()`. Strings stay as `pub(crate) const` referenced as `crate::error::INVALID_ARGS` / `NAN_ERROR`.
- **B3.3** — DEFERRED. Three `helpers.rs` files contain genuinely diverse contents (truthy + datetime extraction + sentinel check etc.). Renaming requires a content split first, scheduled with Batch 6.
- **B3.4** ✅ — `compile/builder.rs` → `compile/walker.rs`.
- **B3.5** ✅ — `compile/path_parser.rs` → `compile/path_segments.rs`.
- **B3.6** — DEFERRED, pairs with B6.4.
- **B3.7** ✅ — Folded `throw.rs` + `try_op.rs` → `error_handling.rs`; `type_op.rs` → `inspect.rs`.
- **B3.8** ✅ — `compile/optimize` declarations dropped to `pub(super)`; `pub mod optimize` → `mod optimize`. All were dead modifiers.
- **B3.9** ✅ — `bvec` moved to `arena/util.rs`.
- **B3.10** ✅ — `arena/pool.rs` simplified to `arena/singletons.rs` (the test-only `BumpGuard` slot-pool had no production callers and went with it; 3 BumpGuard unit tests deleted with the struct).

### Batch 4 — Doc + dead-code cleanup — ✅ resolved (70ebc61)

- **B4.1** ✅ — Replaced stale `OpCode::evaluate_direct` mention with a pointer to `engine::dispatch::dispatch_node_inner`. *(`opcode.rs:12`)*
- **B4.2** ✅ — Trimmed duplicate intro doc on `Engine`; one-line pointer to crate-level docs. *(`engine/mod.rs:13`)*
- **B4.3** ⚠️ — REVERTED. Gating `preserve_structure()` on the feature broke an unconditional caller in `compile/mod.rs`. Kept the method always-available; doc tightened to state explicitly that it returns `false` off-feature.
- **B4.4** ✅ — `Logic::static_arena` → `_static_arena`; dropped `#[allow(dead_code)]`. The underscore prefix carries the same intent natively.
- **B4.5** ✅ — `MetadataHint`, `ReduceHint`, `PathSegment`, `CompiledNode` and 6 sibling structs in `node.rs` demoted to `pub(crate)`. `Logic::root` field, `Logic::new` constructor, and `ExpressionNode::build_from_compiled` likewise tightened to satisfy the visibility-mismatch lint.

**`#[allow(...)]` audit (in addition to B4 items):**
- Removed `#[allow(unused_imports)]` from `lib.rs:131` — the imports were genuinely unused; trimmed the re-export.
- Removed `#[allow(dead_code)]` from `Logic::_static_arena` (B4.4).
- Kept `#![allow(deprecated)]` in `compat.rs` (module-wide — the module *defines* deprecated v4 names) and in 7 test files that exercise `LegacyApi`. All load-bearing.

### Batch 5 — Internal cleanup — ✅ resolved (5ca69a2)

- **B5.1** ✅ — Dropped `#[doc(hidden)]` from `Engine::from_builder_parts`; `pub(crate)` already enforces non-external use.
- **B5.2** ✅ — Inlined `Engine::new_inner`. `Engine::new` calls `from_builder_parts` directly.
- **B5.3** ✅ — `evaluate_serde` compiles + delegates to `run_to_value`. One canonical post-compile body.
- **B5.4** ✅ — `__invalid_args__` sentinel replaced with `CompiledNode::InvalidArgs { id }` variant; dispatcher routes to `Error::invalid_args()`. `check_invalid_args_marker` helper and call sites in `control.rs`/`logical.rs` deleted.
- **B5.5** ✅ — Deleted `compile_builtin_args` pass-through.
- **B5.6** — **DEFERRED.** `TraceCollector` raw-pointer refactor needs either a `'tr` lifetime parameter on `ContextStack` or a session-id-keyed side-buffer; substantive enough for a focused follow-up commit. The current `unsafe { ptr.as_ptr().as_mut() }` is sound — tracer reads happen synchronously within one `evaluate()` call.
- **B5.7** ✅ — `evaluate_throw` switched from `format!("{:?}", ...)` to a stable `value_type_name()` helper returning canonical `"null"`/`"boolean"`/`"number"`/etc.
- **B5.8** ✅ — Doc-commented the intentional string-substring vs array-strict-equality asymmetry on `evaluate_in`.

### Batch 6 — Operator dedup + module fold — ✅ resolved (770e054)

- **B6.1** — **DEFERRED.** Three arithmetic fold loops (`variadic_fold`, `subtract_variadic`, `one_arg_array_fold`) differ in int-coercion strategies — `as_i64()` strict vs `try_coerce_to_integer_cfg` permissive. Unifying risks behavior changes around overflow boundaries the test suite may not cover. Slated for a focused follow-up.
- **B6.2** — SKIPPED. The `is_datetime_object` / `is_duration_object` private helpers in `operators/datetime.rs` are 1-line key probes only used in this file; not worth a helper.
- **B6.3** ✅ — Single `arena::value::reborrow_arena_value` SAFETY-noted helper replaces 5 open-coded `unsafe { &*(v as *const ...) }` sites (lookup.rs ×2, traversal.rs's local helper, array/helpers.rs::for_each_iter_object, array/reduce.rs::reduce_arena_bridge).
- **B6.4 + B3.6** ✅ — `src/value/mod.rs` folded into `src/compat.rs` (the only callers were compat-feature-gated anyway). Shared `datetime_sentinel(key, payload) -> Value` helper is now used by both `compat::owned_to_serde` and `arena/value/conversion.rs::data_to_value`. `crate::value::NumberValue` references rewritten to `datavalue::NumberValue` directly.
- **B3.3** ✅ — `operators/helpers.rs` → `operators/truthy.rs`. Datetime extract helpers (`extract_datetime` / `extract_duration`) moved into `operators/datetime.rs` (datetime-feature-gated together). The `truthy.rs` name now matches its single-purpose content.

### Batch 7 — Micro-cleanup (P2; opportunistic) — partially resolved (1e00d35)

Two items landed (no-behavior-change subset). Eight deferred — see commit `1e00d35` for the breakdown of *why* each was skipped/deferred (perf-only changes, public API breaks, or behavior-change risks).

**Done in 1e00d35:**
- ✅ Dropped unused `_arena` param from `traverse_segments`, `access_path_str_ref`, `apply_path_element`, `evaluate_exists_compiled`, `FastPredicate::evaluate`, `FastPredicate::resolve_value` (~25 call sites).
- ✅ `dispatch.rs` `with_kind:` macro arm collapses Divide/Modulo + Abs/Ceil/Floor into 11 lines of table entries (was 64 lines of boilerplate).

Touch when you're already in the file. None individually worth a PR.

- `IntoOperatorBox` 30 lines of sealed-trait scaffold for two impls. Take `T: CustomOperator + 'static` directly; offer `add_operator_boxed` for the rare pre-boxed path. *(`lib.rs:197-226`)*
- `evaluate_format_date` allocates the chrono format via chained `replace().replace()…`. Compile-time format-string transform, or arena-allocate the result string. *(`operators/datetime.rs:141-149`)*
- `dispatch.rs` arm for `Divide`/`Modulo`/`Abs`/`Ceil`/`Floor` repeats 60 lines for 5 ops sharing a discriminator-wrapping shape. Add a third macro arm with a literal `kind` parameter. *(`engine/dispatch.rs:130-189`)*
- `evaluate_array_literal` / `evaluate_structured_object` allocate every child as a fresh `DataValue` for nested literals. *(`engine/dispatch.rs:333-358`)*
- `if … return; if … return;` waterfalls in `comparison.rs:412-484`, `variable.rs:127-153`, `string.rs:55-77`. Reshape as `match` arms or extracted "stage" functions. (`variable.rs` already half-moved with `resolve_metadata_hint` / `resolve_reduce_hint` — finish the job.)
- `extract_opt_i64_arena` + literal-fast-path-then-dispatch repeated. *(`operators/slice.rs:94-115`, `operators/string.rs:55-77,69-77`)*
- `slice_chars` open-codes index-list construction twice. Build once, consume from both array & string sites. *(`operators/array/slice.rs:120-150,152-203`)*
- `scope_level: u32` while `get_at_level` accepts `isize` and immediately calls `unsigned_abs()`. Unify on `u32`. *(`arena/context.rs:276`)*
- `access_path_str_ref` / `path_exists_str` accept `_arena: &'a Bump` they never use. Drop the parameter. *(`arena/value/traversal.rs:64,123,141`)*
- `OPCODE_NAMES` table + `OpCode::as_str` match are duplicated source-of-truth. Macro-generate both arms from one list. *(`opcode.rs:168-273` vs `:301-398`)*

---

## Anti-findings — preserve in v5

Things the codebase already does well; resist drive-by changes.

- **Two-tier eval design** (`Engine::evaluate` arena-mode + `Session` owned-mode) — clean separation of power-user and convenience paths.
- **`EvalInput` sealed trait** — five impls, parse-fallibility threaded through one method, no leaks.
- **`Error::resolved_path`** — cheap path-of-ids on the hot error path, lazy resolve to structured `PathStep` only when consumed.
- **`Logic` `Sync` impl with the documented `static_arena` invariant** — exactly the right level of unsafe-block hygiene.
- **`kind_tag()` + `#[non_exhaustive] ErrorKind`** — forward-compatible serialization.
- **Outlined `literal_fallback`** (`engine/mod.rs:54-69` `#[cold] #[inline(never)]`) — thoughtful hot-path optimisation.
- **`OpCode` discriminants are stable `#[repr(u8)]` with feature-gated holes** (`opcode.rs:51-160`) — keeps numeric IDs stable across feature toggles.
- **Operator dispatch macro split into `simple` / `iter` / `other`** (`engine/dispatch.rs:27-58`).
- **The line drawn between `&'a [CompiledNode]` args (most ops) and pre-parsed structs (`Var` / `Exists` / `Missing` / `MissingSome`)** — clean and intentional.
- **Free-function operator style + `CustomOperator` trait only for user-supplied ops** — keep. Trait dispatch for built-ins would lose the dispatch macro's codegen.
- **The optimizer's pass split + fixpoint loop** (`compile/optimize/mod.rs:46-68`).
- **`OpCode::FromStr` linear scan** — call site is correctly identified as cold.
- **`compile::missing` ↔ `operators::missing` split** (compile-time literal pre-parse vs runtime evaluation).
- **`OpCode::Concat` ↔ `cat` operator string asymmetry** — the opcode discriminant is named for in-dispatcher readability; the operator string follows the canonical JSONLogic spec. Two different surfaces with two different audiences. Don't unify.
- **`for_each_iter_array` / `for_each_iter_object` helpers + `ControlFlow`-based short-circuit** (P0.6) — natural callback shape for filter/map/quantifiers without forcing reduce into the same mould.

---

## Suggested order

The batches are listed in landing order. Strict ordering only matters for **B1 + B2** — they bind the public API surface; do them before release. **B3** onwards can be interleaved or deferred.

| Batch | Items | Risk | Approx LoC |
|---|---|---|---|
| B1 — Public API hygiene | 7 | low | ~50 |
| B2 — v5 renames | 6 | low (mechanical) | ~200 churn |
| B3 — Module reshape | 10 | low (file moves) | minimal semantic |
| B4 — Doc cleanup | 5 | trivial | ~30 |
| B5 — Internal cleanup | 8 | low | ~100 |
| B6 — Operator dedup | 4 | medium (needs care) | net –200 to –300 |
| B7 — Micro-cleanup | ~10 | trivial each | as you go |

**Recommended landing path:** B1 → B2 → (B4 + B5 in parallel as one chore-PR) → B3 → B6. B7 is opportunistic.
