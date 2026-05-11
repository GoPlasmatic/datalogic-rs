# Changelog

All notable changes to `datalogic-rs` are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

_No changes pending._

## [5.0.0] - 2026-05-09

v5 is a significant rework of the public API and the evaluation engine.
Compiled rules flow through a single arena-based dispatch path; the
crate no longer pulls in `serde_json` by default; and the public surface
has been narrowed to a small, documented set of types organised into
three categories with non-overlapping purposes (`datalogic::`, `Engine`,
`Session`). v5 is a **hard cliff** — there is no `compat` feature and no
in-crate deprecated shims; v4 callers follow `MIGRATION.md` at the repo
root.

### Breaking Changes

- **`DataLogic` → `Engine`.** The top-level type is now `Engine`. The 4.x
  inherent constructors (`new`, `with_preserve_structure`, `with_config`,
  `with_config_and_structure`) collapse into a single fluent
  `Engine::builder()` (`EngineBuilder`). v4 constructors are removed.
- **`serde_json` is no longer a default dependency.** Default features
  are empty. Enable the `serde_json` feature (`features = ["serde_json"]`)
  to take and return `&serde_json::Value` (via `IntoLogic` / `EvalInput`)
  and to use the typed `eval_into::<T>` family (`T: DeserializeOwned`).
- **`CustomOperator` trait** (renamed from the 4.x `Operator`): the
  evaluate method is `evaluate` (was `evaluate_arena`), receives
  pre-evaluated `&'a DataValue<'a>` args, takes `&mut EvalContext<'_, 'a>`
  (was `&mut ContextStack<'a>`), and returns `Result<&'a DataValue<'a>>`
  allocated in a `bumpalo::Bump`. The 4.x `ArenaOperator` trait is
  removed.
- **Operator registration is builder-only.** `Engine` itself no longer
  has `add_operator`; it lives on `EngineBuilder`. The single
  `EngineBuilder::add_operator(name, op)` accepts both typed
  (`T: CustomOperator + 'static`) and pre-boxed
  (`Box<dyn CustomOperator>`) operators — the box itself implements
  `CustomOperator` by delegating, so one entry point covers both
  shapes. Once `build()` returns, the operator set is frozen. The 4.x
  `remove_operator` is gone — registration is single-direction; rebuild
  the builder if you need a different set.
- **Internal types are no longer public.** `CompiledNode`, `OpCode`,
  `MetadataHint`, `PathSegment`, `ReduceHint` were public in 4.x and are
  now `pub(crate)`. They were never reachable through any documented
  workflow; if you were importing them, file an issue with your use case.
- **The `preserve` *operator* is removed.** Literal scalars and arrays
  pass through inline, so the operator served no purpose.
  Object templating moves to `Engine::builder().with_templating(true)`
  (templating mode, gated by `feature = "templating"`).
- **`evaluate*` verbs split into two roles.** Ergonomic one-shot calls
  use the `eval*` family (`eval`, `eval_str`, `eval_into::<T>`) — the
  engine owns the per-call arena and returns `OwnedDataValue` / `String`
  / `T`. The raw `Engine::evaluate(&Logic, data, &Bump)` keeps its
  longer name as the marker for the caller-arena, borrowed-result
  power tier. v4 method names (`evaluate_json`, `evaluate_owned`,
  `evaluate_ref`, `evaluate_arc_value`, `evaluate_json_value`,
  `evaluate_structured`, `evaluate_json_structured`,
  `evaluate_json_with_trace`) are removed; see `MIGRATION.md`.
- **`Error::wrap` preserves the source chain.** Wrapping an arbitrary
  `std::error::Error` produces an `ErrorKind::Custom` whose
  `std::error::Error::source()` returns the original — the 4.x
  Display-only wrap is gone.
- **`Error::operator` and `Error::node_ids` are private fields** with
  public accessors `operator()` / `node_ids()`. Construct via
  `Error::with_operator(...).with_node_ids(...)`.
- **`Error` no longer implements `UnwindSafe` / `RefUnwindSafe`.**
  Caused by the new `ErrorKind::Custom(Arc<dyn std::error::Error + Send + Sync>)`
  variant — the trait object is not unwind-safe by default. Downstream
  code that wraps `Engine::eval*` in `std::panic::catch_unwind` will
  need to use `AssertUnwindSafe` (or restructure to avoid the catch).
- **`TracedResult` removed.** The trace surface returns
  `TracedRun<R>` directly: success and failure share one
  `result: Result<R, Error>` field instead of the v4 split between
  `result: Value` + `error: Option<String>` + `structured_error: Option<Error>`.
- **`EvaluationConfig::with_nan_handling` renamed** to
  `with_arithmetic_nan_handling`. The fluent setters now mirror the field
  names exactly; pair with the new `with_division_by_zero`,
  `with_loose_equality_errors`, `with_truthy_evaluator`,
  `with_numeric_coercion`, and `with_max_recursion_depth`.
- **`PathStep` is `#[non_exhaustive]`.** The fields are output-only (every
  `PathStep` is produced by `Logic::resolve_node_ids` / `Error::resolve_path`),
  so locking it now means future field adds in 5.x are non-breaking.
  Now derives `Deserialize` alongside the existing `Serialize`, so
  external tooling can JSON-roundtrip resolved paths.
- **`EvaluationConfig` and `NumericCoercionConfig` are `#[non_exhaustive]`.**
  External callers can no longer use struct-expression construction —
  including struct-update syntax (`Config { ..Default::default() }`).
  Use `Config::default()` (or the presets `EvaluationConfig::strict()` /
  `safe_arithmetic()`) followed by the `with_*` fluent setters. Fields
  remain `pub` for direct mutation: `let mut c = Config::default(); c.foo = bar;`
  also works. This is the v5 lock-in moment — adding fields to either
  struct in 5.x will be non-breaking for downstream from now on.

### Added

- **Three-category public API.** `datalogic::` (zero-config one-shot),
  `Engine` (configured / raw-arena power), `Session` (compile-once hot
  loop). Each category has one purpose; the choice between them is
  unambiguous.
- **`datalogic::` module-level helpers.** `eval`, `eval_str`,
  `eval_into::<T>` (gated on `serde_json`), `compile` — backed by a
  `OnceLock<Engine>` of `Engine::default()`. No engine construction
  required for the no-config path.
- **`EngineBuilder`** — fluent construction of `Engine` with config,
  templating mode, constant-folding toggle, and pre-registered custom
  operators. Includes `with_constant_folding(bool)` to disable the
  optimiser pass for callers that walk the compiled tree.
- **`Engine::eval` / `eval_str` / `eval_into`** — one-shot evaluation
  with engine-owned per-call arena, returning `OwnedDataValue` /
  `String` / `T: DeserializeOwned` respectively.
- **`Engine::compile_arc`** — convenience wrapper that returns
  `Arc<Logic>` for cross-thread sharing in one call.
- **`Session`** — handle that owns a reusable `bumpalo::Bump` and lets
  callers `reset()` it between batches. Methods: `eval`, `eval_str`,
  `eval_into::<T>`, `eval_borrowed` (zero-copy borrowed result).
- **`Engine::evaluate`** — power-tier arena method. Returns a borrowed
  `&'a DataValue<'a>` allocated in a caller-owned `Bump`.
- **`IntoLogic` trait** (sealed) — accepts `&str`, `&String`,
  `&OwnedDataValue`, `OwnedDataValue`, or `&serde_json::Value` (gated
  on `serde_json`) for `Engine::compile`.
- **`OwnedInput` trait** (sealed) — same shapes as `IntoLogic`,
  consumed by the one-shot `eval*` methods. Doesn't carry an arena
  lifetime, so it composes cleanly with the engine-owned per-call
  arena.
- **`EvalInput` trait** (sealed) — accepts `&str`, `&String`,
  `&OwnedDataValue`, `DataValue<'a>`, `&'a DataValue<'a>`, or
  `&serde_json::Value` (gated on `serde_json`) for the borrowed-result
  paths (`Engine::evaluate`, `Session::eval_borrowed`).
- **`FromDataValue` trait** (sealed) — output side. `OwnedDataValue` /
  `String` always available; `serde_json::Value` and typed
  `T: DeserializeOwned` gated on `serde_json`. Powers the suffix
  vocabulary (`_str`, `_into::<T>`).
- **`TracedSession`** mirrors `Session` 1:1. Methods (`eval`,
  `eval_str`, `eval_into::<T>`, `eval_borrowed`) all return
  `TracedRun<R>`. The one-shot `eval_str` / `eval_into` compile with
  optimisation disabled so every operator surfaces a trace step.
- **`Error::wrap<E: std::error::Error + Send + Sync + 'static>`** —
  ergonomic `?`-friendly wrapper that preserves the source chain;
  no-ops when called on an existing `Error`.
- **`Error::thrown_value()`** — accessor for the `ErrorKind::Thrown`
  payload without manually pattern-matching the kind.
- **`Error::resolve_path(&Logic) -> Vec<PathStep>`** — translates the
  failure breadcrumb into structured `PathStep`s (root-to-leaf).
- **`#![forbid(unsafe_code)]`** — the crate is now unsafe-free, enforced
  at build time.
- **`pub use bumpalo;`** — `bumpalo` is now re-exported at the crate root.
  Use `datalogic_rs::bumpalo::Bump` instead of pulling in `bumpalo` as
  a separate dependency, so `Engine::evaluate` / `CustomOperator::evaluate`
  arena lifetimes resolve against the same major version that
  `datalogic-rs` itself uses.
- **`PathStep: Deserialize`.** Pair with the existing `Serialize` to
  JSON-roundtrip resolved paths.
- **`with_*` setters on `NumericCoercionConfig`.** Mirrors the
  `EvaluationConfig` fluent API: `with_empty_string_to_zero`,
  `with_null_to_zero`, `with_bool_to_number`, `with_reject_non_numeric`,
  `with_undefined_to_zero`. All `#[must_use]`, all return `Self`.

### Removed

- The entire `compat` module — `LegacyApi` trait, `ArenaValue` /
  `ArenaContextStack` / `ArenaOperator` deprecated aliases, v4 method
  shims (`compile_serde_value`, `evaluate_arc_value`, `evaluate_owned`,
  `evaluate_ref`, `evaluate_json`, `evaluate_structured`,
  `evaluate_json_structured`, `evaluate_json_with_trace`,
  `evaluate_json_with_trace_structured`), and the v4 inherent
  constructors (`with_preserve_structure`, `with_config`,
  `with_config_and_structure`). v4 callers migrate via `MIGRATION.md`.
- The `migrating_from_v4.rs` example (its translation table moved to
  `MIGRATION.md`).
- The 4.x `preserve` operator (see breaking changes above).
- The 4.x value-mode evaluation path. Every evaluation flows through
  the arena dispatcher; `serde_json::Value` is converted at the boundary
  only when the `serde_json` feature is enabled.
- `TracedResult` (replaced by `TracedRun<R>`; see breaking changes).
- `ContextStack` re-export (replaced by `EvalContext`).

### Performance

- **OpCode dispatch** — built-in operators dispatch through an enum
  rather than string lookup.
- **Cached predicate / iter-input shape** on every compiled node, so
  `filter` / `map` / quantifiers skip the per-iteration pattern match.
- **Cached root operator name** on `Logic` — error attachment no longer
  walks the tree.
- **Lazy result-buffer allocation** in `missing` / `missing_some` /
  `merge` so empty paths cost zero allocation.
- **`Cow<'static, str>`** in `ErrorKind` variants and `Error.operator` —
  built-in error names attach with no heap allocation.

### Feature flags

- `serde_json` — `&serde_json::Value` interop and the typed
  `eval_into::<T>` family (`T: DeserializeOwned`). Replaces the v4
  `compat` feature.
- `templating` — templating mode on the builder (multi-key objects compile
  to output-shaping templates; unknown operator keys pass through).
- `datetime` — `now`, `parse_date`, `format_date`, datetime arithmetic.
- `trace` — execution tracing (transitively requires `serde_json`).
- `error-handling` — `try` / `throw` operators.
- `ext-string`, `ext-array`, `ext-control`, `ext-math` — opt-in operator
  groups beyond the JSONLogic baseline.
- `wasm` — convenience bundle for the `@goplasmatic/datalogic` WASM
  package (enables `datetime`, `trace`, `templating`).

See `MIGRATION.md` at the repo root for the v4 → v5 cookbook.

[Unreleased]: https://github.com/GoPlasmatic/datalogic-rs/compare/v5.0.0...HEAD
[5.0.0]: https://github.com/GoPlasmatic/datalogic-rs/releases/tag/v5.0.0
