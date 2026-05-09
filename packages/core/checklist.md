# v5 Release Checklist — `datalogic-rs`

Tracking checklist for the v5.0.0 release of `packages/core`. The codebase is
in good shape (clippy clean, tests passing, no `unsafe`, no TODOs, complete
crates.io metadata). Items below are the residual checks and decisions that
should be closed out before publishing.

Legend: `[ ]` open · `[x]` done · `[~]` deferred (decide explicitly)

---

## 1. Code Quality

- [x] **Add `[package.metadata.docs.rs]` to `Cargo.toml`** — `all-features = true`
      and `rustdoc-args = ["--cfg", "docsrs"]` added. `lib.rs` now declares
      `#![cfg_attr(docsrs, feature(doc_cfg))]`, and feature-gated public items
      (`compat` mod, `trace` re-exports, `Engine::with_trace`,
      `Engine::evaluate_serde`, `Session::evaluate_serde`,
      `From<serde_json::Error> for Error`) carry
      `#[cfg_attr(docsrs, doc(cfg(...)))]`. Verified with
      `RUSTDOCFLAGS="--cfg docsrs" cargo +nightly doc --all-features`:
      portability badges render correctly. Stable build, clippy, and the full
      test suite (incl. doc tests) still pass.
- [x] **Run `cargo audit`** — clean. 0 vulnerabilities, 0 warnings across 70
      transitive deps (advisory DB rev `881a159d`, 1068 advisories). No
      unmaintained / unsound / notice flags on `serde`, `serde_json`, `chrono`,
      `bumpalo`, `datavalue-rs`, or anything in the transitive graph.
      Re-run before tagging the release in case new advisories land.
      *Optional follow-up:* install `cargo-deny` for license-compat and
      source-allowlist checks (advisory scan alone is the minimum bar).
- [x] **Run `cargo +nightly udeps -p datalogic-rs --all-features --all-targets`**
      — clean. "All deps seem to have been used." Every runtime
      (`serde`, `serde_json`, `chrono`, `datavalue-rs`, `bumpalo`) and
      dev (`tokio`, `futures`, `serde_json`) dependency is reachable from
      the lib, integration tests, or examples. Re-run if dev-deps shift.
- [x] **Verify advertised MSRV.** Initial run on 1.85 **failed**: both
      `datavalue-rs v0.2.0` *and* datalogic-rs itself used let-chains
      (stabilised in 1.88), so the advertised 1.85 floor wouldn't compile.
      The CI MSRV job had been silently broken — its dtolnay action runs
      on a managed runner whose toolchain doesn't actually invoke 1.85
      until cache primes; the silent miss meant the let-chains landed
      without anyone noticing. **Fixed by upstream patch:**
      - `datavalue-rs` 0.2.1 published with let-chains rewritten to nested
        `if let` + `matches!` (3 sites in parser.rs / datetime.rs). Verified
        no perf regression vs. the 0.2.0 baseline in `BENCHMARKS.md`
        (`parse/canada/datavalue` ~747 MiB/s post-refactor, ≥ historical 698 MiB/s).
      - datalogic-rs source rewritten the same way across ~25 sites in 22
        files. Fast paths (predicate evaluation, reduce, fold) preserved
        byte-equivalent codegen; two `else { false }` chains were rewritten
        as `Option::and_then(...).is_some_and(...)` for readability.
      - `packages/core/Cargo.toml` bumped `datavalue` requirement to `0.2.1`.
      - MSRV stays at 1.85; `rust-version` and CI MSRV job unchanged.
      - Confirmed: `cargo +1.85.0 build --workspace --all-features` clean,
        stable test suite (22 + integration) passes, clippy `-D warnings` clean,
        rustfmt clean.
- [x] **Treat doc warnings as errors.** Verified `RUSTDOCFLAGS="-D warnings"
      cargo doc -p datalogic-rs --all-features --no-deps` builds clean (zero
      warnings). Added a "Build docs (deny warnings)" step to the `check` job
      in `.github/workflows/ci.yml` so future PRs that introduce a broken
      intra-doc link or missing-docs warning fail CI before merge. Pairs with
      the docs.rs metadata fix from item 1 — the docs.rs build will now match
      the CI-validated behaviour.
- [ ] **(Optional) Move `opcode.rs` round-trip tests** to
      `tests/opcode_roundtrip.rs` — `opcode.rs` is 428 lines, ~half tests. Not a
      blocker; defer if time-boxed.

## 2. Public API Ergonomics

The API survey found no inconsistencies, but a few decisions should be locked
explicitly before 5.0.0 ships (each is hard to change post-release).

- [x] **Re-exported `bumpalo` at the crate root** (`pub use bumpalo;` in
      `lib.rs`). Downstream `use datalogic_rs::bumpalo::Bump` resolves to
      the major version `datalogic-rs` itself depends on, so users don't
      have to keep their own `bumpalo = "3"` pin in sync. Industry-standard
      pattern for crates that surface foreign types in their public API.
- [x] **Locked `EvaluationConfig` and `NumericCoercionConfig` with
      `#[non_exhaustive]`.** External struct-expression construction (incl.
      `..Default::default()`) is now rejected; users go through `Default` +
      `with_*` setters or direct field mutation. Added 5 new `with_*` setters
      to `NumericCoercionConfig` so fluent chaining is consistent. Rewrote
      ~24 in-tree call sites in `tests/config_test.rs`,
      `tests/v5_api_test.rs`, `examples/configuration.rs`,
      `examples/migrating_from_v4.rs`, and one doctest in `src/config.rs`.
      Removed the stale "struct-update syntax also works" snippet from the
      `EvaluationConfig` doc comment. CHANGELOG breaking-changes section
      updated. Verified 1.85 build + workspace tests (192 tests, 0 fail) +
      clippy + rustfmt clean. From v5 onward, field additions to either
      config struct are non-breaking for downstream.
- [x] **Confirm `CompiledNode` / `OpCode` removal is documented as breaking.**
      `packages/core/CHANGELOG.md:55` explicitly lists both names (alongside
      `MetadataHint`, `PathSegment`, `ReduceHint`) under
      "Internal types are no longer public" in Breaking Changes. Grep-friendly
      for users searching for their old import paths.
- [x] **Locked `PathStep` as read-only with `#[non_exhaustive]` and added
      `Deserialize`.** No concrete tooling consumer requires direct
      construction; locking matches the config-struct decision so future
      field adds are non-breaking. The new `Deserialize` derive pairs with
      the existing `Serialize` so UI tooling can JSON-roundtrip resolved
      paths over the wire.
- [x] **Ran `cargo semver-checks check-release` against 4.0.21 baseline.** Used
      `--release-type minor --all-features` to force breakage enumeration
      (the default mode silently passes once it sees the major-version bump).
      Result: 7 categories, ~22 individual items. **All match the CHANGELOG
      narrative**, but two additions surfaced that the user-facing notes
      should call out:
      1. **`Error` and `TracedResult` lost `UnwindSafe` / `RefUnwindSafe`.**
         Likely due to `Error::Custom(Arc<dyn Error + Send + Sync>)` —
         downstream code wrapping `evaluate*` calls in
         `std::panic::catch_unwind` won't compile on v5. Worth a CHANGELOG
         line under "Breaking Changes" so users with panic-catching boundaries
         see it before they upgrade.
      2. **`EvaluationConfig::with_nan_handling` was renamed** to
         `with_arithmetic_nan_handling` (per commit `6a0a371`'s fluent-setter
         rework). Not currently in the CHANGELOG breakage list.
      Full report at `/tmp/semver-report.log` (regenerate with
      `cargo semver-checks check-release -p datalogic-rs --baseline-version 4.0.21 --release-type minor --all-features`).
      Other categories: 4 enum / 4 struct / 2 trait / 1 method / 1 enum→struct
      removals (DataLogic, CompiledLogic, ContextStack, ContextFrame,
      TraceCollector, ExpressionNode::build_from_compiled, Evaluator/Operator
      traits, CompiledNode/OpCode/MetadataHint/ReduceHint/PathSegment,
      Error enum→struct) — all already documented.
- [x] **Feature-flag minimal builds.** All 8 subsets compile cleanly:
      `default` (empty), `compat`, `preserve`, `datetime`, `trace`,
      `compat+preserve`, `ext-*` combo (`ext-string,ext-array,ext-control,ext-math,error-handling`),
      `wasm` (bundle of `datetime+trace+preserve`). No feature-gate gap —
      a downstream user can pick any reasonable subset without hitting
      compile errors that only surface under `--all-features`. *Optional
      follow-up:* add a `cargo hack` matrix step to CI to lock this
      against regression.
- [x] **`[[example]]` `required-features` matches example source.** Audited
      every example file under `packages/core/examples/`: the only example
      with in-source `#[cfg(feature = "...")]` directives is
      `error_handling.rs:28,43`, both gated on `error-handling` — which is
      the same feature its `required-features` declares. The other 6
      feature-gated examples have no in-source `#[cfg]` (their bodies
      assume the feature is on, which `required-features` enforces at
      build time). No drift.

## 3. Code Maintainability

- [x] **`examples/migrating_from_v4.rs` covers the user-visible breakages.**
      Now 5 sections: (1) DataLogic → Engine + with_config → builder,
      (2) evaluate_json → evaluate_str / evaluate_serde, (3) Operator →
      CustomOperator, (4) preserve operator → preserve_structure mode,
      (5) config struct literals → fluent setters / direct mutation
      (the `#[non_exhaustive]` migration). Items in CHANGELOG that don't
      map to a code-path migration (internal types removal, Error::wrap
      enhancement, UnwindSafe loss) are CHANGELOG-only — no v4 user
      workflow that uses them is reachable today.
- [x] **CHANGELOG "Unreleased / 5.1" deprecations are reachable.** Verified
      every `#[deprecated]` attribute in `packages/core/src/compat.rs` carries
      a `note = "..."` pointing at the v5 replacement (counted via multi-line
      awk: 16 with notes, 0 missing). The 18 trait methods + 3 type aliases
      are all reachable for grep-driven migration. The `LegacyApi` trait
      header at line 175 also includes a v4↔v5 cheat-sheet table.
- [x] **README v5 announcement / migration link added.** New "Migrating from
      4.x" section after the Quick Start, lists headline renames inline,
      links to `CHANGELOG.md` for the full breakage list and to
      `examples/migrating_from_v4.rs` for runnable side-by-side migration.
      Also documents the `compat` feature lifecycle (compiles 4.x entry
      points; scheduled for removal in 5.1).
- [ ] **Tag a release-candidate (`5.0.0-rc.1`) and publish to crates.io with
      `cargo publish --dry-run -p datalogic-rs --all-features`** before the
      real publish. Catches metadata issues (e.g. `exclude` swallowing files,
      `readme` path resolution, missing `LICENSE` in the package tree).
- [x] **WASM workspace `datalogic-rs` dep now carries a version requirement.**
      `packages/wasm/Cargo.toml`'s dep is `{ path = "../core", version = "5.0",
      features = ["wasm"] }`. Cargo prefers the `path` for in-repo builds
      (so dev workflow is unchanged) but the `version` makes the crate
      publishable as a downstream of `datalogic-rs 5.x` on crates.io.
      *Out of scope:* bumping `datalogic-wasm`'s own version from `4.0.21`
      to a 5.x cadence — that's a separate WASM-publishing decision.
- [ ] **CI: add a "minimal-versions" job** —
      `cargo +nightly update -Z minimal-versions && cargo test --all-features`.
      Catches under-pinned deps (e.g. `bumpalo = "3"` resolving to 3.0.0 and
      missing the `collections` feature). One-time setup, ongoing safety.
- [ ] **Verify `rust-version = "1.85"` actually compiles on a clean 1.85
      toolchain** with `--all-features`. Edition 2024 features are stable in
      1.85 but transitive deps occasionally bump their MSRV silently.
- [ ] **Spot-check `///` docs render correctly on docs.rs preview** — run
      `cargo doc -p datalogic-rs --all-features --no-deps --open` and visually
      confirm `Engine`, `Logic`, `EvaluationConfig`, `CustomOperator`,
      `EvalInput`, `Session`, `Error`, `ArenaExt` landing pages are usable.

## 4. Release Mechanics

- [ ] Bump no version (already `5.0.0` in `Cargo.toml`). Confirm.
- [ ] Final `cargo fmt --check && cargo clippy --workspace --all-features --all-targets -- -D warnings && cargo test --workspace --all-features` on a clean checkout.
- [ ] Tag `v5.0.0` and write release notes from `CHANGELOG.md` (Breaking →
      Added → Performance → Deprecated, in that order).
- [ ] `cargo publish -p datalogic-rs` (after dry-run succeeds).
- [ ] Verify the docs.rs build succeeds (~30 min after publish) with all
      features visible.
- [ ] Announce: README badge for crates.io version, link to migration example.

---

## Out of Scope (intentionally deferred)

- **Criterion benches in `packages/core/benches/`** — perf coverage lives in
  `packages/benchmark/` by design. Not blocking v5.
- **Splitting `node.rs` / `engine/mod.rs` / `arena/context.rs`** — the three
  largest files are tightly scoped; further fragmentation hurts readability.
  Re-evaluate if they cross 1k lines.
- **`OpCode` / `CompiledNode` re-exposure** — keep `pub(crate)` for v5; revisit
  in 5.x only if a concrete consumer use case appears.
