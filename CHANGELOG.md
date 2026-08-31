# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

Per-binding versions track the core crate's version. The repository ships
under a single coordinated tag (`vX.Y.Z`), driven by `.github/workflows/release.yml`.

## [Unreleased]

### Added

- **Templating: opt-in `$`-prefix escape for object keys.**
  `Engine::builder().with_template_key_escape('$')` makes exactly one
  leading prefix strip from every template key, and stops an escaped key
  from resolving as an operator — so `{"$type": ...}` emits the key `type`
  instead of running the `type` operator, and `$$type` emits a literal
  `$type`. Recovers the ~60 built-in names (plus any registered custom
  operator) as output keys. The prefix is a `char` rather than a fixed `$`,
  since `$` already begins real keys in MongoDB and JSON Schema payloads.
  Off by default: without it, `$`-prefixed keys pass through verbatim as
  before. Requires `feature = "templating"` and templating mode.
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
- **Dependency refresh across every ecosystem.** `cargo update` over all
  six Rust workspaces, `npm` over the three JS packages, plus `composer`,
  Maven and NuGet. Requirement bumps worth calling out: the Python
  binding's `self_cell` floor moves 1.2.2 → 1.3.0 (matching the core
  crate), JVM `jackson-databind` → 2.22.2 and JUnit 5.14.4 → 6.1.3, .NET
  `Microsoft.NET.Test.Sdk` → 18.9.0 / `xunit.runner.visualstudio` → 4.0.0
  / `Microsoft.SourceLink.GitHub` → 10.0.400, and the release workflow's
  maturin pin 1.13.3 → 1.15.0. `cargo audit`, `npm audit` and
  `composer audit` are clean. The one surviving `cargo audit` warning
  (RUSTSEC-2026-0097, `rand` 0.7.3 via `jsonlogic-rs` → `phf` 0.8) is
  reachable only through the dev-only benchmark crate's opt-in
  `subject-jsonlogic-rs` feature and ships in nothing; `jsonlogic-rs`
  0.5.0 is its own latest release, so there is no upgrade path.
- **.NET builds on the 10.0.x SDK.** `dotnet-version` moves from `8.0.x`
  to `10.0.x` in CI and both release workflows, which the .NET 10-wave
  test toolchain needs. `TargetFramework` stays `net8.0` — only the build
  SDK moved, so consumers on .NET 8 are unaffected.
- **TypeScript held at `~6.0.3`.** TypeScript 7.0.2 is npm `latest`, but
  typescript-eslint 8.68.0 still declares `typescript >=4.8.4 <6.1.0`, so
  taking 7.x would break `npm run lint`. 6.0.3 is the top of the 6.x
  line; the hold lifts when typescript-eslint supports TypeScript 7.
- **`wasip2` stays at 1.0.1** under the MSRV-aware resolver: 1.0.4
  requires Rust 1.87 and the crate's floor is 1.85. Likewise `smallvec`
  stays on the 1.x line, since 2.0 has only alpha releases.

### Fixed

- **`to_json` round-trips a misused `and`/`or`/`if` again.** An operator
  in that family given a non-array argument compiles to a deferred
  `InvalidArgs` marker, which serialised to the placeholder
  `{"<invalid args>": null}`. That is not JSONLogic the engine reads
  back: in templating mode it re-parsed as an ordinary output field, so
  an erroring rule round-tripped into a *successful* one returning
  `{"<invalid args>": null}` as data; outside templating it re-parsed as
  an unknown operator, losing which op actually failed. The marker now
  serialises as the offending rule verbatim, `{"<op>":
  <args>}`, which recompiles to the same node and raises the same error.
  The marker retains its raw arguments to make that possible, which also
  covers `format_date` / `parse_date` rejected for a bad literal
  timezone, where the arguments are a well-formed array. `trace.rs` no
  longer carries its own copy of the rendering, so the debugger's
  expression tree shows the real sub-expression and agrees with its
  steps. `CompiledNode` stays at 48 bytes.
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

### Added (UI)

- **Engine settings and custom operators in the editor.** `DataLogicEditor`
  accepts `config` (preset, NaN and division-by-zero handling, truthiness,
  numeric coercion, recursion cap) and `customOperators`; both apply to the
  result and the trace, and the toolbar summarises non-default settings. The
  Studio exposes them in an Engine settings panel and carries them in share
  links.
- **Insert menu and shortcuts.** An Insert toolbar button (Cmd/Ctrl+K) adds an
  argument to the selection, wraps it, or targets the root; Cmd/Ctrl+D
  duplicates the selected node.
- **Step timeline and failure reporting.** The debugger lists every step with
  its node, iteration, context and result, with click-to-jump. The node on the
  engine's failure breadcrumb is highlighted with its error, and a rule that
  fails to compile reports the error in a banner above the diagram.
- **flagd operators in the palette.** `fractional` and `sem_ver` join the
  registry under a new "Feature Flags" category, so every operator the engine
  accepts is documented and insertable.
- **Per-operator documentation links** in the properties panel, pointing at
  that operator's page on the docs site.
- **Test suite (about 1,100 vitest cases).** Every operator help example is
  evaluated against the bundled engine, a corpus covering every operator
  round-trips through the node graph, real trace envelopes are asserted to map
  onto the diagram, and every shipped sample is checked against its expected
  result.

### Changed (UI)

- **Samples cover every operator family**, including `switch`, `??`,
  `try`/`throw`, `type`, `keys`/`values`/`entries`, `group_by`, `distinct`,
  `sort` with a key extractor, `slice` with a step, `sem_ver`, `fractional`,
  IANA timezones and iteration metadata. Templating samples switch the mode on
  automatically when loaded.
- **`data` accepts any JSON value** (object, array or scalar) in the component,
  the Studio and the embed, matching the engine.
- **One CSS import.** React Flow's base styles were already vendored into
  `styles.css`; the redundant second import is gone from the embed, the
  examples and the docs.
- **Dependency cleanup.** Playground-only packages (`@msgpack/msgpack`,
  `fflate`, `@fontsource/*`) and the WASM package moved to devDependencies, so
  consumers no longer install them; the release workflow now rewrites the
  devDependency pin.
- **Dead code removed**: the legacy `LogicEditor` component, `AddArgumentMenu`,
  and the unused per-node evaluation hook.
- **Vite 8.2 deprecations cleared.** The four Vite/Vitest configs use
  `import.meta.dirname` instead of `__dirname`, ahead of the native config
  loader becoming the default, and the embed build declares
  `codeSplitting: false` in place of the deprecated `inlineDynamicImports`.
  The embed bundle is byte-identical across the change.

### Fixed (UI)

- **Serialization round trips.** Editing a rule no longer corrupts it:
  if/else-if chains kept duplicated operands, `switch`/`match` dropped a case
  whose value was `null` (and every case after it), multi-segment `val` paths
  lost segments, `var` defaults were dropped on selection, `exists` paths were
  emitted as one dotted key, and templating structures lost inline values and
  nesting.
- **Editing operations.** Fixed argument reindexing after a deletion,
  duplicate/paste/wrap on structure and fixed-slot parents, argument add and
  remove on `if`, `switch` and `exists`, and clone id collisions.
- **Iteration metadata.** The editor emitted `{"val": "index"}`, which the
  engine reads as a plain key lookup; it now emits `{"val": [[1], "index"]}`.
- **Operator help matched to the engine.** Wrong arities (`sort`, `??`,
  `parse_date`/`format_date`, `reduce`), wrong results (`all` on an empty
  array, `!`/`!!` on empty collections, `timestamp` normalisation), and
  capabilities the engine does not have (datetime property access on `val`,
  dot-notation paths for `exists`) were corrected.
- **Trace mapping.** Steps now match their nodes through the engine's
  canonicalised expressions (`val`/`var` forms, `?:` and `match` aliases,
  nested `switch` cases, templating structures), so the diagram no longer
  falls back to synthetic nodes.
- **`ExecutionStep` type** matches the engine: `step_id` (not `id`), with
  nullable `result` and `error`.
- **Structured errors** in the debug panel show the type, failing operator,
  node breadcrumb and thrown payload, and are parsed from the error object's
  own properties rather than its message text.
- **Menus.** The Object category was missing from the canvas menu and the
  per-category cap silently hid `distinct`; every operator is now reachable.
- **Embed.** Canvas edits now reach the host through `onChange`, templating is
  supported (toolbar toggle and `data-templating`), and structured errors are
  displayed.
- **Duplicate edge ids** from structure nodes and `var` defaults, which made
  React Flow drop one of the two links.

## [5.3.0] - 2026-08-25

### Added

- **`Engine::builtin_operator_names()`**
  ([#65](https://github.com/GoPlasmatic/datalogic-rs/issues/65)).
  Iterator over every built-in operator key this build resolves: the
  JSONLogic baseline plus whichever extension families were compiled
  in, including the input aliases `var`, `?:` and `match`. Derived from
  the compiler's own lookup table, so it cannot drift from dispatch.
  Complements `custom_operator_names()` (since 5.0); the union of the two
  is the engine's full vocabulary, which authoring-side tooling needs
  under templating mode, where an unknown key is not an error but
  echoes back as data. Downstream consumers (dataflow-rs, Orion) can
  drop their hand-kept mirrors of the operator list.

## [5.2.0] - 2026-08-19

### Added

- **`group_by` and `distinct` operators** (`ext-array` feature)
  ([#63](https://github.com/GoPlasmatic/datalogic-rs/issues/63)).
  `{"group_by": [array, key_expr]}` collapses an array into
  `{key, items}` rows on a per-element key expression, insertion-ordered
  by first key occurrence; keys keep their evaluated type and group by
  strict deep equality. `{"distinct": [array]}` dedups by value,
  `{"distinct": [array, key_expr]}` by computed key — first occurrence
  wins in both forms. `group_by` and keyed `distinct` join the
  iteration class (never constant-folded, key expressions excluded from
  CSE); unkeyed `distinct` is pure and fold-eligible.
- **`keys` / `values` / `entries` operators behind a new `ext-object`
  feature** ([#63](https://github.com/GoPlasmatic/datalogic-rs/issues/63)).
  The object take-apart family: `entries` yields `[{key, value}]` rows
  the array vocabulary can iterate. `null` input yields `[]`; other
  non-object input errors. All four Rust-side bindings (and the C ABI
  consumers) enable the feature; pure Rust consumers opt in.
- **Optional trailing IANA timezone argument on `format_date` and
  `parse_date`**, backed by `chrono-tz` inside the `datetime` feature
  ([#63](https://github.com/GoPlasmatic/datalogic-rs/issues/63)).
  `format_date(ts, fmt, "Asia/Kolkata")` renders the instant as
  wall-clock time in the zone (the `"z"` token then reports the target
  zone's offset at that instant); `parse_date(s, fmt, zone)` reads a
  naive input as zone wall-clock and resolves it to the UTC instant.
  DST policy: ambiguous local times resolve to the earlier instant,
  nonexistent ones (spring-forward gap) error. Zone offsets come from
  chrono-tz's compiled-in table — no tzdata I/O, so `format_date` with
  a literal zone stays fold-eligible, and a literal *unknown* zone
  name is rejected at compile time through the operator-specialisation
  stage. Two-argument behavior is byte-for-byte unchanged.
- **Month- and weekday-name format tokens**: `MMM` → abbreviated month,
  `MMMM` → full month, `EEE` → abbreviated weekday, `EEEE` → full
  weekday, alongside the existing `yyyy MM dd HH mm ss` table and raw
  chrono `%` passthrough.

### Fixed

- **`jsonlogic_to_chrono_format` is now a single-pass longest-match
  scanner** instead of sequential `String::replace`. Previously `"MMM"`
  corrupted to `"%mM"` (rendering as a month number followed by a
  literal `M`); token families now disambiguate by length and a
  replacement can never be re-matched by a later token.

## [5.1.2] - 2026-08-10

### Added

- **Root `Makefile` with repo-wide targets.** `make lint`, `make fmt`,
  `make clippy`, `make clean` and `make clean-all` fan out over every
  Cargo manifest in the tree — root-level `cargo fmt --all` / `cargo
  clippy --workspace` / `cargo clean` silently skip the four bindings
  and the fuzz crate, which are excluded workspaces. `make clippy`
  lints `bindings/wasm` against `wasm32-unknown-unknown` (so its
  `#![cfg(target_arch = "wasm32")]` test module is actually checked)
  and reports every crate's failures in one pass. See
  [DEVELOPMENT.md](./DEVELOPMENT.md#repo-wide-commands).

### Changed

- **CI and release validation lint through one shared composite action**
  (`.github/actions/rust-lint`) running the `make` targets above, so the
  clippy/fmt gate now covers all six Cargo manifests (previously the
  root workspace only) and release validation is structurally identical
  to PR CI instead of a mirrored copy that could drift.
- Dependency floors raised to current: `serde` 1.0.229, `serde_json`
  1.0.151, `smallvec` 1.15.2, `self_cell` 1.3.0, and dev-dependencies
  `tokio` 1.53 / `futures` 0.3.33.

### Fixed

- **Three high-severity dev-dependency advisories patched.** In the UI
  package: `brace-expansion` 5.0.8 → 5.0.9
  ([GHSA-rgw5-rvv9-x895](https://github.com/advisories/GHSA-rgw5-rvv9-x895),
  DoS via unbounded intermediate arrays — the 5.0.8 that 5.1.1 landed as
  a fix turned out to be inside this advisory's range) and `nanoid`
  3.3.16 → 3.3.18
  ([GHSA-2v37-7h3g-55p8](https://github.com/advisories/GHSA-2v37-7h3g-55p8),
  custom generators can loop indefinitely). In the Node binding:
  `js-yaml` 4.3.0 → 4.3.1
  ([GHSA-5p4m-2wfm-xmqj](https://github.com/advisories/GHSA-5p4m-2wfm-xmqj),
  CVE-2026-59870, quadratic CPU in `!!omap` resolution). All three are
  transitive and build-time only (via `eslint`, `vite`, and
  `@napi-rs/cli` respectively), so none ship to package consumers.
  `npm audit` reports zero vulnerabilities in both packages again.

## [5.1.1] - 2026-07-25

### Fixed

- **`reduce` fast paths disagreed with general dispatch above 2^53**
  ([#61](https://github.com/GoPlasmatic/datalogic-rs/issues/61)). Both
  the arithmetic fast path and the `reduce(map(...))` fused loop carried
  the accumulator as a raw `f64` across a two/three-mode state machine.
  General dispatch instead rebuilds a number every step, and the
  binary operators' overflow helper collapses a whole,
  exactly-`i64`-representable result back to an integer, which flips the
  *next* step from float math into exact integer math. Below 2^53 the two
  agree; above it they do not. Folding `[-9591485970090907; 6]` with
  `{"-": [current, accumulator]}` from `0.25` returned `0` on the fast
  paths and `1` through general dispatch. Both now fold through the same
  integer/float promotion the `+` / `-` / `*` operators use, so the
  representation decision is shared rather than mirrored. Found by the
  `fused_reduce_map_agrees` property test; the counterexample is pinned
  in `property_test.proptest-regressions`.

- **`TruthyEvaluator::Python` now behaves as documented.** All three
  dispatch sites matched `JavaScript | Python` and routed to the same
  implementation, so the variant was an alias rather than a distinct
  mode; the only test covering it asserted its `Debug` string. The one
  place the two languages genuinely differ is `NaN`, which Python
  treats as truthy and JavaScript treats as falsy. Both the evaluation
  path and the constant-folding path now implement that, and a test
  pins them in lockstep by running the same rule with folding on and
  off. Default truthiness is `JavaScript`, so JSONLogic conformance is
  unaffected; only engines explicitly configured with
  `TruthyEvaluator::Python` see a behaviour change, and only on values
  arithmetic produced (`NaN` has no JSON literal).

### Security

- **Third-party GitHub Actions are pinned to commit SHAs.** Every
  non-GitHub action resolved through a mutable tag or branch
  (`dtolnay/rust-toolchain@stable`,
  `pypa/gh-action-pypi-publish@release/v1`, `taiki-e/install-action@v2`,
  `shivammathur/setup-php@v2`, `PyO3/maturin-action@v1`,
  `peaceiris/actions-mdbook@v2`). The release workflow holds publish
  credentials for nine registries, so a retagged upstream action could
  have exfiltrated them. Human-readable refs are retained as trailing
  comments.
- **Workflow tokens scoped to least privilege.** `release.yml` defaulted
  to `contents: write` for every job, including the whole build matrix
  and every registry publisher. It now defaults to `contents: read`,
  with `contents: write` opted into only by `publish-go` (pushes the
  `bindings/go/vX.Y.Z` tag) and `github-release`. `ci.yml` had no
  `permissions:` block at all and so inherited the repository default;
  it now declares `contents: read`.
- **Removed an expression-interpolation sink.** `release-build-ui.yml`
  spliced `${{ inputs.version }}` directly into a shell script; the
  value now reaches the script through the environment.
- **Two high-severity dev-dependency advisories patched in the UI
  package.** `postcss` 8.5.16 → 8.5.23
  ([GHSA-r28c-9q8g-f849](https://github.com/advisories/GHSA-r28c-9q8g-f849),
  path traversal in previous-source-map auto-loading) and
  `brace-expansion` 5.0.7 → 5.0.8
  ([GHSA-mh99-v99m-4gvg](https://github.com/advisories/GHSA-mh99-v99m-4gvg),
  DoS via unbounded expansion). Both are transitive and build-time only
  (via `vite` and `eslint` respectively), so neither ships to consumers
  of `@goplasmatic/datalogic-ui`. Note the `brace-expansion` advisory
  covers `<= 5.0.7`, so the earlier 5.0.6 → 5.0.7 bump had landed inside
  the vulnerable range; only `npm audit` surfaced it, not the Dependabot
  alert list. `npm audit` now reports zero vulnerabilities.

### Changed

- Four private helpers that reimplemented `datavalue` methods verbatim
  are gone: three copies of `DataValue::as_str` (in `missing`,
  `comparison`, and `datetime`) and one of `DataValue::is_null`. The
  `flagd` operators' private object lookup now routes through the
  shared `object_lookup_field`, which carries the length and
  first-byte prefilter the rest of the crate already uses. No public
  API change.
- CI gains a `feature-matrix` job building each opt-in feature
  standalone. `check` previously ran only `--all-features` and
  `--no-default-features`, so a cross-feature reference missing a
  `#[cfg]` compiled in both and broke only for users enabling a single
  feature. All ten features build clean today.
- The `c-abi-host` composite action moves to `actions/cache@v5`,
  clearing the Node.js 20 deprecation warnings on the C ABI and JVM
  binding jobs.

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
