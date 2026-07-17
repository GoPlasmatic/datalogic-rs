# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Per-binding versions track the core crate's version. The repository ships
under a single coordinated tag (`vX.Y.Z`), driven by `.github/workflows/release.yml`.

## [5.1.0] - 2026-07-17

### Added

- **Common-subexpression elimination (core).** JSONLogic has no `let`
  bindings, so rules repeat pure aggregate subexpressions verbatim. A
  new whole-tree compile pass detects structurally identical pure
  subtrees and shares one memoized evaluation per rule execution
  instead of recomputing each occurrence. The pass is invisible in
  every public observable — `to_json()`, trace trees, and error
  breadcrumbs are byte-identical to a non-CSE compile — and subtrees
  containing custom operators, `throw`/`try`, `now`, `fractional`, or
  `sem_ver` are never memoized. `Logic` gains a public
  `cse_slot_count()` accessor reporting how many memo slots the
  compiler assigned. The `macro/checkout-40` benchmark (which
  recomputes its subtotal map+reduce in 8 places) drops ~2.9x.
- **Python: PEP 561 type stubs in every wheel.** `datalogic-py` ships
  `datalogic_py/__init__.pyi` plus the `py.typed` marker covering the
  full surface (`apply`, `Engine`, `Rule`, `Session`, `DataHandle`,
  batch errors, the exception hierarchy, `__version__`), guarded
  against drift by `mypy.stubtest` in CI and a wheel-content check in
  the release matrix. PyPI listing gains the `Typing :: Typed`
  classifier.

### Performance

- `reduce(map(...))` pipelines fuse into a single pass: the fold runs
  directly over the map's input instead of materializing the
  intermediate array in the arena. Results are bit-identical (the
  fused loop composes the same representation-choice primitives as the
  unfused pipeline); non-numeric shapes bail to the general flow.
  Together with CSE, the `macro/checkout-40` cross-library matrix row
  improved 2.8x over the 5.0.1 capture (9.5 µs → 3.3 µs) with per-eval
  arena usage down from 31.9 KB to under 2 KB.
- The reduce arithmetic fold and the strict-eq filter fast path adopt
  the hinted `FieldCursor` field-lookup pattern from the map fast
  paths.
- datavalue 0.2.3's buffered, heap-free number emit: parse-eval-
  serialize round trips improve ~3–8% on serialize-heavy workloads;
  evaluation-only paths are flat.

### Fixed

- Whole floats outside i64's exactly-representable range stringify via
  shortest round-trip formatting — `1e300` now prints as `"1e300"`
  instead of a saturated `"9223372036854775807.0"` (matching
  serde_json). The `datavalue` dependency floor moves to 0.2.3.
- Removed the unsound numeric-string precoercion optimizer pass:
  folded and unfolded evaluation previously disagreed on arithmetic
  over numeric strings with values beyond 2^53 (a string operand keeps
  arithmetic in f64 space while a rewritten number literal takes the
  exact-integer paths). Rules with fully-static numeric-string
  arithmetic still constant-fold — through the real engine evaluator.
- `reduce` arithmetic fast path honors operand order: fold bodies of
  the form `{"-": [current, accumulator]}` returned sign-flipped
  results versus the general path (add/multiply were unaffected by
  commutativity).
- JVM: `jackson-databind` bumped to 2.22.1 (CVE-2026-54515).

## [5.0.1] - 2026-07-07

### Changed

- **BREAKING (C ABI — in-tree consumers only): ABI v2.** `bindings/c`
  replaces the v1 contract wholesale: `(pointer, length)` UTF-8 inputs
  (no NUL terminators anywhere), status-code returns with an optional
  `datalogic_error **` out-param (the thread-local last-error block is
  deleted, and with it Go's per-call `LockOSThread`), borrowed
  session results, owned `datalogic_buf` one-shot results, and a
  custom-operator callback protocol with no cross-boundary allocator
  handoff. Wrappers assert `datalogic_abi_version() == 2` at load. The
  ABI was never published as a standalone artifact; all four in-tree
  consumers (Go, JVM, .NET, PHP) migrated in lockstep with their public
  APIs unchanged. Migration table in
  [`MIGRATION.md`](./MIGRATION.md#500--501-c-abi-v2-bindings-internal).
- **BREAKING (JVM environment): FFM replaces JNA; JDK 22+ required.**
  The Java binding now reaches the C ABI through `java.lang.foreign`
  (add `--enable-native-access=ALL-UNNAMED` on JDK 24+); the JNA
  dependency is deleted and with it the microseconds of reflective
  dispatch per call. The Java-visible API is unchanged, and the JAR's
  native-resource layout is identical. Also fixes by construction the
  latent non-ASCII corruption: JNA marshalled *argument* strings with
  the JVM default charset (results were already forced UTF-8).
- **BREAKING (WASM): errors are real
  `Error` objects.** Through 5.0.0 every `@goplasmatic/datalogic-wasm`
  API rejected with a plain JSON *string*, so `e instanceof Error` was
  `false`. APIs now throw a proper `Error` whose `name` is the error
  kind (for example `"ParseError"`), with the structured fields
  (`type`, `operator`, `node_ids`, variant extras) attached as own
  properties and the old JSON string preserved verbatim on
  `e.detailJson`. Migration snippets in
  [`bindings/wasm/README.md`](./bindings/wasm/README.md#error-handling).

### Performance

- C-family bindings: session results serialize into a reusable
  session-owned buffer and cross the boundary as borrowed bytes — the
  per-result malloc and the `datalogic_string_free` crossing are gone;
  session-less one-shots run over a pooled thread-local arena, so naive
  callers get session-grade allocation behaviour.
- Node: the object-typed entry points (`Rule.evaluate`, `Engine.eval`)
  now route JSON-string data straight into the arena parser instead of
  building an intermediate `serde_json::Value` tree (mirroring what
  `evaluateStr` and the Session methods already did).
- Python: wheels now build with fat LTO + a single codegen unit — the
  binding's standalone workspace previously shipped with no release
  profile at all, losing cross-crate inlining into the core.
- Python: dict inputs and results convert via a direct walk between
  Python objects and arena values instead of the pythonize double tree
  (pythonize retained only as the exotic-shape fallback), with the
  pre-change semantics pinned by a 549-case equivalence corpus:
  2.5-3.4x faster at every payload size, and the 8 KB dict path drops
  from ~82 µs to ~24 µs — now ~3x faster than a `json.dumps` /
  `json.loads` round-trip. (The same direct-converter approach was
  built, measured, and deliberately reverted for Node: 23-31% faster
  than its serde bridge but still structurally slower than V8's
  `JSON.stringify` + one string crossing; the string path remains
  Node's fast lane, and the equivalence test stays in-tree as the gate
  for future attempts.)
- Wide-object key lookup uses an optimistic ordered probe.
- Identically-shaped ISO datetime strings compare on a byte-compare
  fast path.
- `try`/`catch` propagates thrown values through a borrowed channel and
  interns the NaN payload.
- Composite literals are pre-converted at compile time via
  self-referential cells.
- Strings render directly into the evaluation arena (no intermediate
  heap `String`); contiguous `slice` (step == 1) is zero-copy; hot
  numeric/flagd paths and cold error/output paths drop throwaway
  allocations; constant-fold passes early-bail before cloning arg trees.

### Fixed

- Docs honesty: the Python README's dict-conversion claim is qualified
  by payload size (the `pythonize` path wins below roughly 1 KB and
  reverses above), and the WASM README no longer claims zero-copy
  strings across the JS↔WASM boundary (both directions copy; cost
  scales with payload size).
- WASM binding: the `now` operator trapped with "time not implemented on
  this platform" (and leaked that call's arena) in every JS host, because
  the v5 rewrite dropped the v4 `wasm` opt-in for `chrono/wasmbind`. The
  binding now enables the new `wasm-clock` feature, and a
  `wasm-bindgen` regression test covers the operator.
- Benchmark harness: `datalogic-bench` was missing the `flagd` feature,
  so `flagd/*.json` suites reported ERR for the native engine column
  while the WASM column ran them.
- Four conformance suites that existed on disk but were missing from
  `tests/suites/index.json` now run in the conformance runner;
  `type.json` array cases corrected.
- Benchmark harness `suites_root` repointed at `crates/datalogic-rs`
  after the repository reorganization.
- Python binding: pyo3 / pythonize bumped to 0.29 (security advisories).

### Added

- **`ParsedData` (core)** — self-contained parse-once data handle,
  accepted by every arena-lifetime evaluation entry point at zero
  per-call conversion cost. Parsing dominates the string contract
  (70-90% of a parse-eval-serialize round trip), and this factors it
  out. **`Engine::truthy`** exposes the engine's configured truthiness
  coercion for binding use.
- **Data handles, typed results, and batch evaluation across the
  C-family bindings** (C, Go, JVM, .NET, PHP): parse a payload once
  (`datalogic_data_parse` / `DataHandle`) and evaluate many rules
  against it; typed scalar evaluations (`bool` / `i64` / `f64` /
  truthiness); and one-crossing batch shapes — one rule × N payloads
  (`evaluate_batch`) and N rules × one payload (`evaluate_many`, the
  rule-set/feature-flag shape) — with per-item error reporting that
  never fails the whole call.
- **PHP FFI preload support** — an `FFI::load`-compatible header and
  `preload.php` for `opcache.preload` + `ffi.enable=preload`
  deployments; `FFI::cdef` remains the zero-config fallback.
- **In-tree boundary benchmark harness**
  (`tools/benchmark/boundary/`) — one runner per runtime (Rust core, C,
  Go, JVM, .NET, PHP, Python, Node, WASM) reproducing the
  BINDINGS-OVERHEAD methodology with byte-stable checked-in workloads,
  a driver script, and a table renderer, so the per-binding overhead
  numbers are reproducible with one command instead of living outside
  the repo.
- **ABI v2 mirrors for Node, Python, and WASM** — the direct-core
  bindings gain the same tiers natively: `DataHandle` parse-once
  handles, typed session evaluations (`evaluateBool` / number / truthy;
  Python adds `evaluate_int`), and `Promise.allSettled`-shaped
  `evaluateBatch` / `evaluateMany` with per-item errors that never fail
  the call. The WASM handle keeps the payload resident in linear
  memory, so the per-call JS↔WASM copy + parse disappears (the 8 KB
  session path drops ~7.7x).
- **Node async tier** — `Rule.evaluateStrAsync(dataJson)` evaluates on
  the libuv thread pool and returns a `Promise<string>`; rejections
  carry the same structured fields as synchronous throws. Not faster
  per call — it exists for event-loop hygiene on large payloads.
- **WASM speed-profile opt-in** — `WASM_PROFILE=speed ./build.sh`
  builds `opt-level = 3` + `wasm-opt -O3`: measured 1.13-1.85x faster
  across tiers at +8.1% raw size and 1.4% *smaller* gzipped. The
  published default stays the size-optimized build.
- **`wasm-clock` feature** — opt-in JS-host clock for the `now` operator
  on `wasm32-unknown-unknown` (forwards to `chrono/wasmbind`; successor
  to the v4 `wasm` feature). Off by default so non-JS wasm runtimes
  (wasmtime, wazero, Chicory) keep loading the module — the constraint
  from [#47](https://github.com/GoPlasmatic/datalogic-rs/issues/47) —
  with a CI guard asserting the default wasm32 dependency graph stays
  free of `wasm-bindgen`/`js-sys`.
- **`Logic::is_constant`** — reports whether compilation constant-folded
  the entire rule to a literal. Complements `Logic::is_static`
  (`is_static` asks whether a rule *could* be evaluated without a data
  context; `is_constant` reports whether the compiler actually *did*
  reduce it — folding can fail, e.g. `{"/": [1, 0]}` stays an operator
  node so the error surfaces at evaluation time). The benchmark harness
  uses it to time folded and non-folded rules separately.
- **`EvaluationConfig::from_json_str`** (requires `serde_json`) — build a
  configuration from a JSON object: an optional `"preset"` key
  (`"default"` / `"safe_arithmetic"` / `"strict"`) plus per-field
  overrides. This is the wire format the language bindings use to pass
  engine configuration across FFI boundaries through one shared parser.
  Unknown keys and enum strings are rejected loudly.
- cargo-fuzz target over `eval_str`.
- flagd `fractional` testbed scenarios (flagd v3.1.0–v3.5.0) ported into
  the conformance suites.
- Release platform matrix evened out: Intel-mac Python wheels and Node
  prebuilds, aarch64-musl Node prebuilds.
- Runnable `examples/` for every language binding (C, Node, WASM,
  Python, Go, JVM, .NET, PHP): the same three programs — `getting-started`,
  `compile-once-evaluate-many`, `custom-operator` — with the same rule
  and data in each language, executed in CI so they cannot rot.
- `scripts/conformance-count.sh` — generates the canonical
  "N suites / M cases" statistic quoted in READMEs and release notes.
- **Signal Board redesign of the React visual debugger**
  (`@goplasmatic/datalogic-ui`): nodes are typed and coloured by the
  *return-type signal* they produce (boolean, number, string, array, and
  so on) rather than by operator category, exposed through overridable
  `--sig-*` CSS tokens scoped to `.logic-editor`. Boolean operators render
  as SVG logic-gate silhouettes (AND / OR / NOT), edges carry an explicit
  left-to-right flow direction, and the depth axis packs by real node
  widths so wide nodes no longer overlap. The React component's public
  props are unchanged.

### Removed

- **`NumericCoercionConfig::undefined_to_zero`** and its
  `with_undefined_to_zero` setter. The flag was documented as reserved
  and was never read: JSONLogic does not distinguish a missing key from
  an explicit `null`, and a missing var already coerces to `0` under the
  default `null_to_zero = true`. Removing an inert public field is
  technically breaking for code that merely named it; delete the field
  access or setter call — nothing changes behaviourally.

## [5.0.0] - 2026-05-14

v5 is a coordinated major release across the Rust core crate and every
language binding — WASM, Node, Python, C, Go, JVM, .NET, and PHP. For
step-by-step v4→v5 migration, see [MIGRATION.md](./MIGRATION.md).

### Added

- **Node-native binding** (`@goplasmatic/datalogic-node`) via napi-rs,
  shipping per-platform `.node` prebuilds. WASM is now positioned for
  browser/edge; Node services should prefer the native binding.
- **Python binding** (`datalogic-py`) via pyo3 + maturin, with abi3-py310
  wheels across Linux (gnu/musl, x86_64/aarch64), macOS, and Windows.
- **C ABI crate** (`bindings/c`) via cbindgen, exposed as a static and
  shared library consumed in-tree by the Go / JVM / .NET / PHP bindings.
- **Go binding** (`datalogic-go`) over the C ABI, with a synthetic
  `bindings/go/v*` tag published by the release pipeline.
- **JVM binding** (`io.github.goplasmatic:datalogic`) via JNA over the
  shared C cdylib, packaged for Maven Central. *(Correction 2026-07-03:
  the Maven Central publish leg did not run for 5.0.0 — the group had no
  published artifacts. The first Maven Central release ships with the
  next tag; until then, build from source per `bindings/jvm/README.md`.)*
- **.NET binding** (`Goplasmatic.Datalogic`) via P/Invoke over the
  shared C cdylib, published to NuGet.
- **PHP binding** (`goplasmatic/datalogic`) via PHP FFI over the shared
  C cdylib; ships via a subtree split to `GoPlasmatic/datalogic-php`
  (Packagist resolves from tags). *(Correction 2026-07-03: the subtree
  split ran, but the package was not registered on packagist.org until
  2026-07-03, so `composer require` did not resolve before that date.)*
- **`flagd` Cargo feature** — opt-in OpenFeature flagd-compatible operators
  ([spec](https://flagd.dev/reference/custom-operations/)):
  - `fractional` — deterministic murmurhash3-x86-32 percentage bucketing,
    matching the canonical Go evaluator's `(hash * total_weight) >> 32`
    integer distribution. Hash implementation vendored inline (~30 LOC,
    no external dep) for portability across every target.
  - `sem_ver` — semantic-version comparison with the spec's four input
    normalizations (strip `v`/`V` prefix, pad partial versions, coerce
    numeric input, drop build metadata). Backed by the optional
    [`semver`](https://docs.rs/semver) crate.

  Both return `null` on malformed input; conformance test suites under
  `crates/datalogic-rs/tests/suites/flagd/` mirror the upstream
  [`fractional_test.go`](https://github.com/open-feature/flagd/blob/main/core/pkg/evaluator/fractional_test.go)
  and [`semver_test.go`](https://github.com/open-feature/flagd/blob/main/core/pkg/evaluator/semver_test.go).
- **Custom operator registration across every language binding** — WASM,
  Node, Python, C ABI, Go, JVM, .NET, and PHP now expose a way to
  register host-language callbacks as JSONLogic operators, with a
  uniform JSON-string in/out contract. See
  [`bindings/BINDINGS.md`](./bindings/BINDINGS.md#custom-operator-support).
- **Module-level helpers**: `datalogic_rs::eval`, `eval_str`, `eval_into`,
  and `compile` — backed by a default engine, no construction required.
- **`engine.eval_into::<T>(...)`** for typed deserialization of results.
- **`engine.compile_arc(...)`** for the cross-thread sharing pattern.
- **`with_constant_folding(false)`** builder flag for tree walkers
  (debuggers, alternate evaluators).
- **`TracedSession`** mirrors `Session` 1:1 — every `eval*` returns
  `TracedRun<R>`. The C ABI surfaces a parallel
  `datalogic_traced_session_*` family so JVM / .NET / PHP / Go share
  the same session-with-trace contract.
- **`ArenaExt` trait** for ergonomic `CustomOperator` return values, plus
  a public `bumpalo` re-export.
- **`IntoLogic`** and **`FromDataValue`** traits for boundary conversion.
- Public docs site (mdBook) at `docs/`, deployed via `.github/workflows/docs.yml`.
- Cross-library benchmark matrix under `tools/benchmark/` (datalogic-rs
  vs. json-logic-* and WASM peers).
- Arena-mode evaluation dispatch: every operator now has a native
  arena variant (no legacy bridge fallbacks), structured-error
  breadcrumbs carry a node-id path, and the trace pipeline reuses
  `CompiledNode::id` directly instead of a side-table HashMap.

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

## [4.0.21] - 2026-04-11

### Fixed

- UI type declarations regenerated to match the published library
  surface; resolves consumer TypeScript build errors against
  `@goplasmatic/datalogic-ui`.

## [4.0.20] - 2026-04-11

### Added

- First CI and release workflows for the v4 line
  (`.github/workflows/ci.yml`, `.github/workflows/release.yml`).

### Changed

- Reduced code duplication across operator implementations and
  trimmed unused dependencies in the core crate.

### Fixed

- UI: resolved edge-crossing artefacts and trace-matching mismatches
  in the visual debugger.
- UI: dropped `vite-plugin-top-level-await` for Vite 8 compatibility.
- Clippy + TypeScript lint cleanups across the workspace.

### Security

- UI dev deps: patched `picomatch` and `brace-expansion` advisories.

## [4.0.19] - 2026-03-12

### Added

- Compilation pipeline restructured into a modular, multi-pass
  optimisation flow (constant-folding etc.) on top of the
  `CompiledNode` IR.

### Changed

- Removed the unused `Optimized(OptimizedNode)` variant from the
  compiled-node enum and trimmed the surrounding match arms.
- Evaluation hot-path tuning: more aggressive `#[inline]` placement
  and enum-size reductions for cache-line wins.

### Fixed

- WASM target: dropped the unconditional `chrono`/`wasmbind` dep so
  consumers compiling for non-browser wasm32 targets build cleanly
  (PR [#48](https://github.com/GoPlasmatic/datalogic-rs/pull/48),
  thanks @aepfli).
- UI: trace child matching now uses deep equality, fixing mismatches
  when `BTreeMap` key ordering diverged between runs.
- UI: CSS imports moved into components so the library build's
  tree-shaker doesn't drop them.

### Security

- Bumped UI dev dependencies to clear `npm audit` findings.

## [4.0.18] - 2026-02-06

### Added

- **`switch` / `match` operator** for pattern-matching style control
  flow (replaces deeply nested `if` chains).
- UI: visual support for rendering `switch`/`match` nodes in the
  debugger.

### Changed

- Compile-time specialisation for hot operators plus fast paths for
  quantifiers (`all`/`some`/`none`), `reduce`, `map`, `try`/`throw`,
  and datetime parsing.
- New invariant-evaluation helper used by `slice`, `cat`, `length`,
  and `min`/`max`.
- Eliminated redundant datetime / duration parsing in comparisons;
  improved comparison heuristics.
- Removed an unnecessary `LazyLock` from NaN error construction.

## [4.0.15] - 2026-02-04

### Added

- Dedicated `CompiledVar` and `CompiledExists` node variants with
  matching evaluation + tracing paths (faster than the generic
  operator dispatch they replace).

### Changed

- **Eval hot path is ~23% faster** in the bundled benchmarks via
  fewer clones, dedicated context-frame fields for `reduce`, and
  `Cow`-based intermediate values.
- Replaced the `BTreeMap`-backed reduce context frame with explicit
  fields (`accumulator`, `current`).
- Removed the `SmallVec` dependency — array nodes use `Vec` directly.
- Operator modules consolidated; duplicated comparison logic
  deduplicated.
- Moved `val` datetime / duration property access out of the val
  fast path and optimised val compilation.
- Bumped `regex` to 1.12.

### Fixed

- Numeric and string comparison fast paths corrected for edge cases
  around mixed types.

## [4.0.14] - 2026-02-02

### Added

- UI: operator catalog panel with category icons + colour coding.
- UI: URL sharing for rules + data ("share a debug session").
- UI: visual editor mode with properties panel + context menus, plus
  per-argument type selection.
- UI: error visualisation in the debugger trace.
- UI: mobile-friendly responsive layout (iPad + phone).
- UI: `componentMode` prop to toggle the mode selector visibility.
- UI: namespaced CSS classes with a `dl-` prefix and a theme system
  (v4.0.13 internal cut).

### Changed

- UI: rebranded from "DataLogic Debugger" to **DataLogic Studio**.
- UI: modularised debugger context, trace utilities, and editor
  architecture; unified node components.
- Core: simplified `throw` / `try` operator implementations.

### Fixed

- UI: focus loss during edits, expression sync on deletion, desktop
  accordion regression, mobile properties-panel positioning,
  toolbar/menu issues on iPad, `if`/`else` trace matching and
  structure-node collapse, filter example using `val`.
- UI: read-only mode no longer mounts `EditorProvider`.

### Removed

- UI: deprecated props, unused CSS, stale public assets and manifest
  reference.

## [4.0.9] - 2026-01-24

### Added

- Crate packaging excludes the `ui/` tree and npm files so cargo
  package payloads stay lean.

### Fixed

- UI: debugger and structure-node edge regressions.
- Docs: corrected datetime operator examples.

## [4.0.8] - 2026-01-24

### Added

- **React visual debugger** (`@goplasmatic/datalogic-ui`) and an
  initial monorepo layout housing the UI alongside the core crate.
- UI: human-readable operator titles in the logic editor.

### Changed

- Renamed the `datalogic-wasm/` directory to `wasm/` and tightened
  the WASM build profile.
- Docs modularised into JS / React sections, plus a link to the
  full-page visual debugger from the playground.

### Fixed

- WASM: `now` datetime operator wired through to the JS surface.
- Playground URLs updated from the legacy `datalogic-ui` repo.
- Docs workflow: corrected the `rust-toolchain` action name.
- pnpm version removed from CI so the `packageManager` field in
  `package.json` is authoritative.

## [4.0.7] - 2026-01-23

### Added

- **Execution tracing** for step-by-step debugging — exposed both in
  the Rust API and the WASM surface.
- WASM published to **npm** with CDN-friendly loading paths.
- WASM `preserve_structure` parameter on the JS entry points.

### Changed

- Playground updated to consume WASM 4.0.7 with the new
  `preserve_structure` flag.

### Removed

- Dropped the "execution-trace proposal" draft now that the feature
  has landed.

## [4.0.5] - 2026-01-09

### Added

- **WebAssembly bindings** as a first-class binding target, with
  optimised dependency tree.
- **mdBook documentation** at `docs/` with GitHub Pages deployment.
- **Custom operators** support that interoperates with
  `preserve_structure` mode (PR
  [#44](https://github.com/GoPlasmatic/datalogic-rs/pull/44),
  thanks @ngerakines).
- Comprehensive documentation set + worked examples.

### Changed

- Context stack simplified; operator implementations trimmed.
- Bumped `regex` to 1.12.2.

## [4.0.4] - 2025-10-03

### Added

- **Comprehensive `EvaluationConfig`** for tuning evaluator behaviour
  (numeric coercion, undefined handling, etc.).

### Changed

- Context-metadata keys and value-access paths optimised.
- `access_path_ref` refactored to use let-chains for cleaner nested
  matching.

## [4.0.3] - 2025-09-18

### Fixed

- `val` operator: numeric indices with level access (e.g. nested
  index lookups in scoped contexts) now resolve correctly.

## [4.0.2] - 2025-09-18

### Fixed

- `reduce` operator: nested properties whose parent key was a numeric
  string no longer mis-resolve.

## [4.0.1] - 2025-09-14

### Changed

- Dependency-version maintenance bump.

## [4.0.0] - 2025-09-14

Major architecture overhaul ("v4 redesign"). The evaluator is now
built around a pre-compiled `CompiledNode` IR with an `OpCode` enum
dispatch, replacing the v3 walk-the-`Value` evaluator. See
[MIGRATION.md](./MIGRATION.md) for v3→v4 movement (and the v4→v5
section for the subsequent migration).

### Added

- **Pre-compilation pipeline**: `OpCode` enum + `CompiledNode` IR,
  static logic pre-compilation, and an inline-function dispatch
  layer.
- **Operator surface — comparison, arithmetic, type, string,
  datetime, duration, control-flow** all rebuilt on the new IR with
  comprehensive coverage and overflow-safe semantics.
- **`exists` operator** plus fixes to `array` / `val` operators
  around it.
- **`length` operator** for strings and arrays.
- **`sort` and `slice` operators**.
- **`try` / `throw` operators** for error handling.
- **`now` operator** returning the current datetime.
- **Comprehensive thread-safety story** with `Arc`-backed root data
  on the context stack.

### Changed

- Eliminated `node_to_value` conversions — everything operates on
  `CompiledNode` end-to-end.
- Consolidated common operator logic into shared helper modules and
  deduplicated comparison code.
- Removed the v3 hash-caching system after profiling showed it was
  no longer load-bearing.
- Datetime overflow protection switched to saturation semantics for
  arithmetic; overflow protection extended to all numeric operators.
- Structured-object handling in the fast evaluator hardened.
- Duration checks reordered ahead of generic object checks in
  comparison operators (fixes mis-typed comparisons).

### Fixed

- `merge` operator: null values handled correctly.
- Numerous compile-time and doc-test regressions surfaced by the
  rewrite.

### Removed

- Arena-allocation layer from the v3 design (the v4 evaluator no
  longer needed it; arena evaluation was reintroduced as an
  optional dispatch mode in v5).
- Hash-caching layer (see above).

[5.1.0]: https://github.com/GoPlasmatic/datalogic-rs/compare/v5.0.1...v5.1.0
[5.0.1]: https://github.com/GoPlasmatic/datalogic-rs/compare/v5.0.0...v5.0.1
[5.0.0]: https://github.com/GoPlasmatic/datalogic-rs/compare/v4.0.21...v5.0.0
[4.0.21]: https://github.com/GoPlasmatic/datalogic-rs/releases/tag/v4.0.21
[4.0.20]: https://github.com/GoPlasmatic/datalogic-rs/releases/tag/v4.0.20
[4.0.19]: https://github.com/GoPlasmatic/datalogic-rs/releases/tag/v4.0.19
[4.0.18]: https://github.com/GoPlasmatic/datalogic-rs/releases/tag/v4.0.18
[4.0.15]: https://github.com/GoPlasmatic/datalogic-rs/releases/tag/v4.0.15
[4.0.14]: https://github.com/GoPlasmatic/datalogic-rs/releases/tag/v4.0.14
[4.0.9]: https://github.com/GoPlasmatic/datalogic-rs/releases/tag/v4.0.9
[4.0.8]: https://github.com/GoPlasmatic/datalogic-rs/releases/tag/v4.0.8
[4.0.7]: https://github.com/GoPlasmatic/datalogic-rs/releases/tag/v4.0.7
[4.0.5]: https://github.com/GoPlasmatic/datalogic-rs/releases/tag/v4.0.5
[4.0.4]: https://github.com/GoPlasmatic/datalogic-rs/releases/tag/v4.0.4
[4.0.3]: https://github.com/GoPlasmatic/datalogic-rs/releases/tag/v4.0.3
[4.0.2]: https://github.com/GoPlasmatic/datalogic-rs/releases/tag/v4.0.2
[4.0.1]: https://github.com/GoPlasmatic/datalogic-rs/releases/tag/v4.0.1
[4.0.0]: https://github.com/GoPlasmatic/datalogic-rs/releases/tag/v4.0.0
