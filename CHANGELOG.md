# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Per-binding versions track the core crate's version. The repository ships
under a single coordinated tag (`vX.Y.Z`), driven by `.github/workflows/release.yml`.

## [Unreleased]

## [5.0.0] - TBD

v5 is a coordinated major release across the Rust core crate and every
binding (WASM, Node, Python, C, Go). For step-by-step v4→v5 migration,
see [MIGRATION.md](./MIGRATION.md).

### Added

- **Node-native binding** (`@goplasmatic/datalogic-node`) via napi-rs,
  shipping per-platform `.node` prebuilds. WASM is now positioned for
  browser/edge; Node services should prefer the native binding.
- **Python binding** (`datalogic-py`) via pyo3 + maturin, with abi3-py310
  wheels across Linux (gnu/musl, x86_64/aarch64), macOS, and Windows.
- **C ABI crate** (`bindings/c`) via cbindgen, exposed as a static library
  consumed in-tree by the Go binding.
- **Go binding** (`datalogic-go`) over the C ABI, with a synthetic
  `bindings/go/v*` tag published by the release pipeline.
- **Module-level helpers**: `datalogic_rs::eval`, `eval_str`, `eval_into`,
  and `compile` — backed by a default engine, no construction required.
- **`engine.eval_into::<T>(...)`** for typed deserialization of results.
- **`engine.compile_arc(...)`** for the cross-thread sharing pattern.
- **`with_constant_folding(false)`** builder flag for tree walkers
  (debuggers, alternate evaluators).
- **`TracedSession`** mirrors `Session` 1:1 — every `eval*` returns
  `TracedRun<R>`.
- **`ArenaExt` trait** for ergonomic `CustomOperator` return values, plus
  a public `bumpalo` re-export.
- **`IntoLogic`** and **`FromDataValue`** traits for boundary conversion.
- Public docs site (mdBook) at `docs/`, deployed via `.github/workflows/docs.yml`.
- Cross-library benchmark matrix under `tools/benchmark/` (datalogic-rs
  vs. json-logic-* and WASM peers).

### Changed

- **Breaking — Cargo feature rename**: `compat` → `serde_json`.
- **Breaking — Engine construction is builder-only.** Replace
  `Engine::with_config(c)` with `Engine::builder().with_config(c).build()`,
  and `Engine::with_preserve_structure()` with
  `Engine::builder().with_templating(true).build()`.
- **Breaking — feature rename**: `preserve_structure` →
  `templating` (semantics unchanged).
- **Breaking — one-shot evaluation API.** `engine.evaluate_json(rule, data) -> Value`
  is replaced by `engine.eval_str(rule, data) -> String` (JSON in/out)
  or `engine.eval_into::<T>(rule, data)` (typed).
- **Breaking — value-boundary evaluation.** `engine.evaluate_owned(&logic, value)` →
  `engine.eval_into::<serde_json::Value, _, _>(rule, &value)`.
- **Breaking — compile from `&Value`.** `engine.compile_serde_value(&v)` →
  `engine.compile(&v)` via the `IntoLogic` trait (requires `serde_json` feature).
- **Breaking — trace API.** `engine.evaluate_json_with_trace(...)` →
  `engine.trace().eval_str(...)`, returning `TracedRun<R>`.
- **Breaking — custom operator surface.** `ArenaOperator` →
  `CustomOperator`; context type `&mut ContextStack<'a>` →
  `&mut EvalContext<'_, 'a>`.
- **Breaking — npm package rename**: WASM is now published as
  `@goplasmatic/datalogic-wasm` (was `@goplasmatic/datalogic`). Node
  consumers should switch to `@goplasmatic/datalogic-node`.
- Errors surface structured `operator` / `node_ids` / `kind` getters;
  `resolve_path(&compiled)` returns root→leaf `PathStep`s.
- `EvaluationConfig` and `NumericCoercionConfig` are now `#[non_exhaustive]`.
- `PathStep` is `#[non_exhaustive]` and implements `Deserialize`.
- MSRV: Rust 1.85 (edition 2024).
- Monorepo layout flattened to `crates/` (Rust core), `bindings/` (one
  folder per language wrapper), `ui/` (React debugger), and `tools/`
  (dev-only). See [ARCHITECTURE.md](./ARCHITECTURE.md).
- Release pipeline split into an orchestrator (`release.yml`) plus
  per-binding `workflow_call` files; coordinated by a single `v*` tag
  with strict pre-publish version-drift validation.

### Removed

- **Breaking — `compat` feature and the `LegacyApi` trait.** No
  deprecated v4 shims remain in the v5 crate; rewrites are mechanical
  per [MIGRATION.md](./MIGRATION.md).
- **Breaking — `data_to_json_string` helper.** Use `datavalue::Display`
  (`.to_string()`) instead.
- **Breaking — `EvaluationConfig::new()`** constructor (use the fluent
  setters / `Default`).

### Migration

See [MIGRATION.md](./MIGRATION.md) for the authoritative v4→v5 cookbook,
including a 60-second checklist, method-by-method translations,
side-by-side patterns, and structural-error consumer recipes.

[Unreleased]: https://github.com/GoPlasmatic/datalogic-rs/compare/v5.0.0...HEAD
[5.0.0]: https://github.com/GoPlasmatic/datalogic-rs/releases/tag/v5.0.0
