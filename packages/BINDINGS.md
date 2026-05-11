# Adding a language binding

Each language binding lives as a sibling crate under `packages/<lang>/` and
follows the same conventions, so a new binding can be added without
re-deriving the layout.

## Naming

Every binding's published artifact follows the **`datalogic-<lang>`** pattern.
`<lang>` is the short, established suffix for the target language — `rs` for
Rust, `py` for Python, `wasm` for WebAssembly, `rb` for Ruby, `go` for Go,
`java` / `kt` / `swift` for the JVM/mobile family.

| Language | Internal Cargo crate | Published artifact | Registry |
|---|---|---|---|
| Rust | `datalogic-rs` | `datalogic-rs` | crates.io |
| WebAssembly | `datalogic-wasm` | **`@goplasmatic/datalogic`** (grandfathered, predates this convention) | npm |
| Python | `datalogic-py` | `datalogic-py` (PyPI) → `import datalogic_py` | PyPI |
| _future_ Ruby | `datalogic-rb` | `datalogic-rb` | RubyGems |
| _future_ Go | `datalogic-go` | `github.com/GoPlasmatic/datalogic-go` | Go modules |

For Python the PyPI distribution name is `datalogic-py` but the Python
**module** name is `datalogic_py` — Python doesn't allow hyphens in
import paths, and PyPI's normalisation already treats hyphens and
underscores as equivalent for installation.

The npm WASM package is `@goplasmatic/datalogic` (without the `-wasm`
suffix) only because it predates the convention; renaming it would force
every existing consumer to update. New language packages should follow
the convention without exception.

## Convention

| Concern | Decision |
|---|---|
| Location | `packages/<lang>/` (sibling of `core`, `wasm`, `python`, `ui`) |
| Workspace | **Excluded** from the root workspace (own `[workspace]` block) |
| Cargo | `crate-type = ["cdylib", "rlib"]` — `cdylib` for the FFI artifact, `rlib` so Rust consumers can also link it |
| Dep on core | `datalogic-rs = { path = "../core", version = "5.x", features = [...explicit list...] }` — the binding inlines the feature set it wants |
| Core feature | **No umbrella feature in core.** The binding owns its operator surface; `packages/core/Cargo.toml` stays free of binding-specific bundling so the published crate is binding-agnostic |
| Tests | `packages/<lang>/tests/` in the binding's native test runner (pytest, jest, …) |
| CI | `.github/workflows/<lang>-release.yml` — separate from the crates.io release workflow |
| Release tags | `v*` (e.g. `v5.0.0`) — same trigger as `release.yml`, so one tag push releases core (crates.io), wasm (npm), ui (npm), python (PyPI), … together. Each binding's workflow validates its own `Cargo.toml` against the tag. |
| Versioning | Bindings track the core version exactly (5.0.0 → 5.0.0). The unified-tag scheme requires this — release-time validation fails if a binding's version drifts from core. For an independent cadence, switch the binding's workflow trigger to a binding-prefixed tag (`<lang>-v*`). |

## Why these conventions

- **Excluded from root workspace.** Bindings pull in language-specific
  build deps (pyo3, napi, jni, …) that bloat the default `cargo test
  --workspace --all-features` and require contributors to install
  language toolchains (Python interpreter, Node.js, JDK) just to run
  Rust tests. Excluding keeps the core's dev loop fast.

- **`cdylib` + `rlib`.** `cdylib` is the importable artifact every
  binding needs (`.so`/`.pyd`/`.dll`). `rlib` lets a downstream Rust
  crate (e.g. integration tests, the binding's own benchmarks) link it
  as a normal dep without duplicating source.

- **No umbrella feature in core.** Each binding inlines the explicit
  feature list it wants in its `datalogic-rs` dep stanza
  (`features = ["serde_json", "templating", "datetime", …]`). The core
  crate stays binding-agnostic — adding or removing a binding never
  touches `packages/core/Cargo.toml`. The trade-off is that bumping a
  shared operator family across all bindings is a multi-file edit, but
  that's a rare event and explicit listing makes each binding's surface
  obvious without cross-referencing.

- **Per-binding release workflow.** Each binding has its own
  `<lang>-release.yml`, even though they share the `v*` tag trigger
  with the core release. Separate files keep the per-binding wheel /
  package matrices contained, isolate publish failures
  (Python wheel build failure ≠ core publish failure), and let a
  binding switch to an independent tag scheme later without unwiring
  shared YAML.

## Existing bindings

| Binding | Path | Tech | Publishes to |
|---|---|---|---|
| WebAssembly | `packages/wasm/` | wasm-bindgen + wasm-pack | npm: `@goplasmatic/datalogic` |
| Python | `packages/python/` | pyo3 + maturin (abi3-py310) | PyPI: `datalogic-py` |

## Open candidates

- **Node.js native** (napi-rs) — faster than WASM for server-side Node
- **Ruby** (magnus / rb-sys)
- **Go** (cgo via cbindgen header)
- **Java/Kotlin** (jni or duchess)
- **Swift** (swift-bridge)

The core engine is `Send + Sync` and exposes a clean compile-once /
evaluate-many surface (`Engine`, `Logic`, `Session`), so the same
binding shape transfers to any language with a Rust FFI story.
