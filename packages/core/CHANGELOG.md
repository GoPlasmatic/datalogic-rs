# Changelog

All notable changes to `datalogic-rs` are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [5.0.0] - Pre-release

v5 is a significant rework of the public API and the evaluation engine.
Compiled rules now flow through a single arena-based dispatch path; the
crate no longer pulls in `serde_json` by default; and the public surface
has been narrowed to a small, documented set of types. A `compat` feature
provides one-release-cycle shims for the most common 4.x entry points.

### Breaking Changes

- **`DataLogic` → `Engine`.** The top-level type is now `Engine`. The 4.x
  inherent constructors (`new`, `with_preserve_structure`, `with_config`,
  `with_config_and_structure`) collapse into a single fluent
  `Engine::builder()` (`EngineBuilder`). Old constructors live on as
  deprecated extension methods in `compat::LegacyApi`.
- **`serde_json` is no longer a default dependency.** Default features are
  empty. Enable the `compat` feature (`features = ["compat"]`) to keep the
  4.x `serde_json::Value` boundary (`Engine::evaluate_serde`,
  `compile_serde_value`, the `LegacyApi` trait, etc.).
- **`CustomOperator` trait** (renamed from the 4.x `Operator`): the
  evaluate method is now just `evaluate` (was `evaluate_arena`), receives
  pre-evaluated `&'a DataValue<'a>` args, and returns
  `Result<&'a DataValue<'a>>` allocated in a `bumpalo::Bump`. The legacy
  `ArenaOperator` trait remains as a deprecated bridge for one release.
- **Operator registration is builder-only.** `Engine` itself no longer has
  `add_operator`; it lives on `EngineBuilder` (also `add_operator_box`
  for pre-boxed `Box<dyn CustomOperator>`). Once `build()` returns, the
  operator set is frozen. The 4.x `remove_operator` is gone — registration
  is single-direction; rebuild the builder if you need a different set.
- **Internal types are no longer public.** `CompiledNode`, `OpCode`,
  `MetadataHint`, `PathSegment`, `ReduceHint` were public in 4.x and are
  now `pub(crate)`. They were never reachable through any documented
  workflow; if you were importing them, file an issue with your use case.
- **The `preserve` *operator* is removed.** Literal scalars and arrays
  pass through inline, so the operator served no purpose.
  Object templating moves to `Engine::builder().preserve_structure(true)`
  (the `preserve` *mode*, gated by `feature = "preserve"`).
- **`evaluate*` verbs unified.** What 4.x called `evaluate_value` is now
  `evaluate_serde` (compat-only). The new entry points are
  `Engine::evaluate_str` (one-shot, JSON in / JSON out) and
  `Engine::evaluate` (arena-aware, zero-copy result).
- **`Error::wrap` preserves the source chain.** Wrapping an arbitrary
  `std::error::Error` produces an `ErrorKind::Custom` whose
  `std::error::Error::source()` returns the original — the 4.x
  Display-only wrap is gone.
- **`Error::operator` and `Error::path` are private fields** with public
  accessors `operator()` / `path()`. Construct via
  `Error::with_operator(...).with_path(...)`.

### Added

- **`EngineBuilder`** — fluent construction of `Engine` with config,
  preserve-structure mode, and pre-registered custom operators.
- **`Session`** — handle that owns a reusable `bumpalo::Bump` and resets
  it between calls. Returns owned (`OwnedDataValue` / `String` /
  `serde_json::Value`) results so callers don't manage arena lifetimes.
- **`Engine::evaluate`** — arena-aware hot path. Returns a borrowed
  `&'a DataValue<'a>` allocated in the caller-owned `Bump` for zero-copy
  read-through paths.
- **`EvalInput` trait** — accepts `&str`, `&OwnedDataValue`, an owned
  `DataValue<'a>`, an existing `&'a DataValue<'a>`, or `&serde_json::Value`
  (with the `compat` feature) without per-shape overloads.
- **`Error::wrap<E: std::error::Error + Send + Sync + 'static>`** —
  ergonomic `?`-friendly wrapper that preserves the source chain;
  no-ops when called on an existing `Error`.
- **`Error::thrown_value()`** — accessor for the `ErrorKind::Thrown`
  payload without manually pattern-matching the kind.
- **`Error::resolved_path(&Logic) -> Vec<PathStep>`** — translates the
  failure breadcrumb into structured `PathStep`s (root-to-leaf).
- **`Engine::with_trace()`** (gated by `feature = "trace"`) — opens a
  `TracedSession` that collects per-node execution steps. The one-shot
  `TracedSession::evaluate_str` compiles with optimisation disabled so
  every operator surfaces a trace step.
- **`#![forbid(unsafe_code)]`** — the crate is now unsafe-free, enforced
  at build time.

### Deprecated

- The entire `compat` module (`compat::LegacyApi`, `ArenaValue`,
  `ArenaContextStack`, `ArenaOperator`, `Engine::evaluate_arc_value`,
  `evaluate_owned`, `evaluate_json`, `evaluate_json_with_trace`, etc.).
  All entries carry `#[deprecated(since = "5.0.0", note = "…")]` with
  the v5 replacement; the module will be removed in 5.1.

### Removed

- The 4.x `preserve` operator (see breaking changes above).
- The 4.x value-mode evaluation path. Every evaluation now flows through
  the arena dispatcher; `serde_json::Value` is converted at the boundary
  only when the `compat` feature is enabled.

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

- `compat` — `serde_json::Value` interop and `LegacyApi` shims for 4.x
  callers.
- `preserve` — structure-preservation (templating) mode on the builder.
- `datetime` — `now`, `parse_date`, `format_date`, datetime arithmetic.
- `trace` — execution tracing (transitively requires `compat`).
- `error-handling` — `try` / `throw` operators.
- `ext-string`, `ext-array`, `ext-control`, `ext-math` — opt-in operator
  groups beyond the JSONLogic baseline.
- `wasm` — convenience bundle for the `@goplasmatic/datalogic` WASM
  package (enables `datetime`, `trace`, `preserve`).

See `examples/migrating_from_v4.rs` for a side-by-side migration walkthrough.

[Unreleased]: https://github.com/GoPlasmatic/datalogic-rs/compare/v5.0.0...HEAD
[5.0.0]: https://github.com/GoPlasmatic/datalogic-rs/releases/tag/v5.0.0
