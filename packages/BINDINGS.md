# Adding a language binding

Each language binding lives as a sibling crate under `packages/<lang>/` and
follows the same conventions, so a new binding can be added without
re-deriving the layout.

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
| Release tags | `<lang>-v*` (e.g. `python-v5.0.0`), so the binding can ship independently of the core |
| Versioning | Track the core version (5.x for this generation) — binding-specific patch bumps are fine, but major/minor align with core |

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

- **Per-binding release workflow + tag scheme.** A binding bug fix
  shouldn't force a core release. `<lang>-v*` tags decouple the
  cadences, and the workflow extracts the version from the binding's
  own `Cargo.toml` (and `pyproject.toml`/`package.json`/etc.) — not
  from `packages/core/Cargo.toml`.

## Existing bindings

| Binding | Path | Tech | Publishes to |
|---|---|---|---|
| WebAssembly | `packages/wasm/` | wasm-bindgen + wasm-pack | npm: `@goplasmatic/datalogic` |
| Python | `packages/python/` | pyo3 + maturin (abi3-py310) | PyPI: `datalogic` |

## Open candidates

- **Node.js native** (napi-rs) — faster than WASM for server-side Node
- **Ruby** (magnus / rb-sys)
- **Go** (cgo via cbindgen header)
- **Java/Kotlin** (jni or duchess)
- **Swift** (swift-bridge)

The core engine is `Send + Sync` and exposes a clean compile-once /
evaluate-many surface (`Engine`, `Logic`, `Session`), so the same
binding shape transfers to any language with a Rust FFI story.
