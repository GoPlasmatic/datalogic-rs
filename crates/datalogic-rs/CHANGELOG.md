# Changelog

All notable changes to `datalogic-rs` — the core crate and every language
binding — are documented in the repository-root
[CHANGELOG.md](https://github.com/GoPlasmatic/datalogic-rs/blob/main/CHANGELOG.md),
which is the single canonical changelog. Release automation validates
that file (and only that file) before a tag can ship, so it is always
current.

This stub exists because the crates.io package is built from this
directory; it intentionally carries no released version entries (only
the `[Unreleased]` notes below, mirrored from the root file) so the two
files cannot drift.

## [Unreleased]

### Added

- **WASM: `builtinOperatorNames()`, `Engine.evaluateWithTrace`,
  `Engine.customOperatorNames()`.** The module-level
  `builtinOperatorNames(): string[]` mirrors
  `Engine::builtin_operator_names()`; `Engine.evaluateWithTrace(logic,
  data): string` returns the same envelope as the top-level
  `evaluateWithTrace` while honouring the engine's templating mode,
  config, and custom operators; `Engine.customOperatorNames(): string[]`
  lists the registered custom operators.
- **Node: `builtinOperatorNames()` and `Engine.customOperatorNames()`.**
  Same shapes as the WASM additions, so authoring tooling can read the
  full operator vocabulary from either JS package.

### Changed

- **`date_diff` rejects unknown units.** An unrecognised unit now raises
  `InvalidArguments` (`date_diff: unknown unit ...`) instead of silently
  returning `0`. Accepted units are `days`, `hours`, `minutes`, `seconds`,
  and `milliseconds`; `milliseconds` is now documented alongside the
  others.
- **Conformance battery is now 58 suites / 1,658 cases**, after the
  regression suites below landed.

### Fixed

- **`and` / `or` constant folding dropped dynamic arguments.** With a
  literal in trailing position, folding could discard the dynamic
  arguments before it or strip a trailing identity literal.
  `{"or": [{"var": "a"}, "fallback"]}` now returns the variable when it
  is truthy, and `{"and": [{"var": "a"}, "x"]}` returns `"x"` when `a`
  is truthy, matching unoptimised evaluation. Regression cases live in
  `control/and.json` and `control/or.json`.
- **`try` now hands engine errors to the catch arm.** Previously only a
  `throw` payload reached the catch arm; an engine-raised error left the
  original data in scope. The catch arm now sees `{"type": ...}` for
  every error: `"Unknown Operator"` for an unknown operator, and the
  error's message text otherwise (the canonical `"Invalid Arguments"`
  for argument-shape errors; an operator-specific message such as
  `date_diff: unknown unit ...` where the operator reports one). A
  missing variable is still not an error (`var` returns `null`), so
  `try` does not fall back on absent data.
- **WASM packaging.** The wasm-bindgen start stub is no longer exported
  as `init`, and `pkg/bundler` no longer receives a `commonjs`
  `package.json` override (the bundler target is ESM).
- **React debugger (`@goplasmatic/datalogic-ui`)**: correctness and feature
  work across the operator registry, serialization round trips, editing
  operations and the trace view. Not part of the crates.io package; the
  entries are in the root
  [CHANGELOG.md](https://github.com/GoPlasmatic/datalogic-rs/blob/main/CHANGELOG.md).
