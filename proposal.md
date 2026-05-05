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

### Batch 3 — Module reshape  *(internal-only, low risk)*

Modules are `pub(crate)` after P0.4/P0.5, so renames don't break external users.

- **B3.1** — **Delete `src/datetime.rs`.** Single-line re-export shim now subsumed by `pub use datavalue;` (P0.3). *(`datetime.rs:8`, `lib.rs`)*
- **B3.2** — **Fold `constants.rs`.** Two strings + two error helpers — module is misnamed. Move into `error.rs`; tighten `pub` → `pub(crate)`. *(`constants.rs`)*
- **B3.3** — **Three `helpers.rs` files.** Rename `operators/helpers.rs` → `truthy.rs`, `operators/arithmetic/helpers.rs` → `arith_fold.rs`, `operators/array/helpers.rs` → `iter_resolve.rs`. Greppable, tab-completion-friendly. *(do all three together)*
- **B3.4** — **`compile::builder` rename.** Conceptually collides with the crate-root `EngineBuilder`. Rename to `compile::walker` or `compile::dispatch`. *(`compile/builder.rs`)*
- **B3.5** — **`compile/path_parser.rs` rename.** Shares the word "path" with the unrelated root `path.rs`. Rename to `compile/path_segments.rs`. *(`compile/path_parser.rs`)*
- **B3.6** — **Root `value/` rename or fold.** `arena/value/` and root `value/` both imply "values." Root is `OwnedDataValue ↔ serde_json::Value` compat helpers — fold into `compat.rs` (datetime sentinel goes with it; combine with **B6.4**). *(`value/mod.rs`)*
- **B3.7** — **`try_op` / `type_op` rename.** `_op` suffix is keyword-collision avoidance only. Rename to `error_handling.rs` and `inspect.rs`, or fold into a sibling. *(`operators/try_op.rs`, `operators/type_op.rs`)*
- **B3.8** — **Tighten `compile/optimize` modifiers.** `pub mod` / `pub fn` are dead — parent is `mod compile;`, so external code can't reach them. Drop to `pub(super)` / `pub(crate)`. *(`compile/mod.rs:12`, `compile/optimize/mod.rs:14-17,46`)*
- **B3.9** — **Move `bvec` out of `arena/mod.rs`.** Module-declaration noise. Move to `arena/util.rs`. *(`arena/mod.rs:33`)*
- **B3.10** — **Split `arena/pool.rs`.** Singletons (`pool.rs:47-167`) and test-only `BumpGuard` (`pool.rs:179-246`) share a file under "pool" but are unrelated. Split into `arena/singletons.rs` and `arena/bump_pool.rs`; consider deleting `BumpGuard` if no production caller remains.

### Batch 4 — Doc + dead-code cleanup

Cosmetic but cheap. Single-commit batch.

- **B4.1** — **`OpCode::evaluate_direct` documented but doesn't exist.** Stale doc shipping fictional API. Replace with a pointer to `engine/dispatch.rs::dispatch_node_inner`. *(`opcode.rs:12`)*
- **B4.2** — **Duplicate intro doc** on `lib.rs` and `Engine`. Keep crate-level only. *(`lib.rs:1-84`, `engine/mod.rs:13-37`)*
- **B4.3** — **`Engine::preserve_structure()` returns false off-feature.** Misleading without the feature. Gate the *method* on `#[cfg(feature = "preserve")]`, not just the body. *(`engine/mod.rs:169-178`)*
- **B4.4** — **`Logic`'s `static_arena` `#[allow(dead_code)]` + four-line justification.** Rename field to `_static_arena` and drop the allow. *(`node.rs:701-704`)*
- **B4.5** — **`MetadataHint` / `ReduceHint` `pub` enums** while only `pub(crate) use`-d at the root. Change definitions to `pub(crate) enum`. *(`node.rs:166-189`)*

### Batch 5 — Internal cleanup

Maintainer-visible hygiene.

- **B5.1** — **`Engine::from_builder_parts` hedged visibility.** `pub(crate)` AND `#[doc(hidden)] pub`. Pick one. *(`engine/mod.rs:117-129`)*
- **B5.2** — **Inline `Engine::new_inner`.** Indirection to avoid two `Self { ... }` blocks under cfg. With `from_builder_parts` already there, this is a dead seam. *(`engine/mod.rs:131-143`)*
- **B5.3** — **`run_to_value` duplicates `evaluate_serde`.** Both build the same arena, run `value_to_data`, evaluate, convert back. Have the compat shim call `evaluate_serde` directly. *(`engine/mod.rs:380,394`)*
- **B5.4** — **`invalid_args_marker` sentinel.** Encodes errors as `OwnedDataValue::Object(vec![("__invalid_args__", true)…])` literals — stringly-typed detection at runtime. Introduce a real `CompiledNode::InvalidArgs { opcode, original }` variant. *(`compile/builder.rs:209-227`)*
- **B5.5** — **`compile_builtin_args` pass-through.** One-line wrapper around `compile_args` taking an unused `_opcode`. Delete and inline. *(`compile/builder.rs:161-169`)*
- **B5.6** — **`TraceCollector` raw-pointer escape hatch on `ContextStack`.** `tracer: Option<NonNull<TraceCollector>>` with manual `unsafe { ptr.as_ptr().as_mut() }`. Sound today, fragile against re-entrant futures. Introduce a lifetime `'tr` (paid only when `trace` is on) or move tracer state into a side-buffer keyed off a session id. *(`arena/context.rs:135,219,237,247`)*
- **B5.7** — **`evaluate_throw` formats type with `{:?}`.** `Debug` output is not API-stable. Use `OwnedDataValue::type_name()`. *(`operators/throw.rs:28`)*
- **B5.8** — **Doc-comment `in`-on-array vs `in`-on-string asymmetry.** `compare_equals` for the array case, byte `contains` for the string case. Fine asymmetry — just doc-comment it. *(`operators/string.rs:131-143`)*

### Batch 6 — Operator dedup

Smaller ROI than the P0 dedup work but worth doing once you're already in the file.

- **B6.1** — **Three near-clone integer-fast-path-with-f64-fallback loops.** `arithmetic/basic.rs:404-458` (`one_arg_array_fold`), `arithmetic/basic.rs:285-339` (`subtract_variadic`), `arithmetic/helpers.rs:143-193` (`variadic_fold`). Thread `op.identity_int()` and `op.combine_int / combine_f` into `variadic_fold` and call from all three sites. ~150 lines saved.
- **B6.2** — **`extract_datetime` / `is_datetime_object` / `extract_duration` / `is_duration_object` overlap.** Hoist the `pairs.iter().any(|(k, _)| *k == "datetime")` test next to `extract_datetime`. *(`operators/helpers.rs:41,65`, `operators/datetime.rs:64,70`)*
- **B6.3** — **Last 2 `unsafe` pair-value reborrows.** `arena/value/lookup.rs:90,111`. Factor into `pub(crate) unsafe fn arena::reborrow_pair_value(...) -> &'a DataValue<'a>` shared with the helper that already exists in `array/helpers.rs::for_each_iter_object` (P0.6 moved 3 sites there). *(after this, the `unsafe` reborrow pattern lives in exactly one place crate-wide)*
- **B6.4** — **Datetime sentinel walk duplicated.** `value/mod.rs::owned_to_serde` and `arena/value/conversion.rs::data_to_value` both hand-recurse to wrap the datetime sentinel object. Extract `datetime_sentinel(name, payload) -> Value` once. *(natural fit for B3.6 — fold `value/mod.rs` into `compat.rs` while extracting this helper)*

### Batch 7 — Micro-cleanup (P2; opportunistic)

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
