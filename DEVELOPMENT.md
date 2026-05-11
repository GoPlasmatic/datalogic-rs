# Development

A walkthrough of working on each package in this monorepo. For the big
picture (what depends on what, why the layout is shaped this way), see
[ARCHITECTURE.md](./ARCHITECTURE.md).

## Prerequisites

| Tool        | Version | Why                                                            |
|-------------|---------|----------------------------------------------------------------|
| Rust        | 1.85+   | The core crate uses `edition = "2024"`                         |
| `wasm-pack` | latest  | Builds `bindings/wasm` (only for WASM/UI changes)              |
| Node.js     | 20+     | Builds and runs `ui`, `bindings/wasm`, and `bindings/node`     |
| Python      | 3.10+   | Builds `bindings/python` via `maturin`                         |
| Go          | 1.22+   | Builds `bindings/go` (also needs a C compiler for cgo)         |
| `mdbook`    | latest  | Builds the docs site under `docs/`                             |

```bash
rustup update stable
curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
# Node via your package manager of choice
pip install maturin    # only if you are editing bindings/python/
cargo install mdbook   # only if you are editing docs/
```

## The build pipeline

The packages have a strict build order. From a fresh clone:

```bash
# 1. Rust workspace — runs core unit/integration tests and the bench crate's checks.
# Most integration tests are gated behind feature = "serde_json"; the JSONLogic
# runner additionally needs feature = "templating". --all-features unlocks both.
cargo test --workspace --all-features

# 2. WASM bindings — produces bindings/wasm/pkg/{web,bundler,nodejs}.
cd bindings/wasm && ./build.sh && cd ../..

# 3. Node native binding — produces bindings/node/datalogic-node.<triple>.node
#    plus the index.js/index.d.ts loaders. Skip if you're only touching the
#    WASM or browser side.
cd bindings/node && npm install && npx napi build --platform --release && cd ../..

# 4. UI — needs the locally-built WASM linked into node_modules first.
cd bindings/wasm/pkg && npm link
cd ../../../ui && npm link @goplasmatic/datalogic && npm install
npm run dev   # or: npm run build:lib for the publishable bundle
```

The `npm link` step is what wires the *just-built* WASM into the UI; without
it, `npm install` would pull `@goplasmatic/datalogic` from the registry and
silently mask any local Rust changes you wanted to test.

## `crates/datalogic-rs` — Rust library

```bash
cargo check -p datalogic-rs
cargo test  -p datalogic-rs                        # default features
cargo test  -p datalogic-rs --all-features         # everything
cargo fmt   --all
cargo clippy --workspace --all-targets -- -D warnings
```

Run a single JSONLogic suite (the `test_jsonlogic` harness picks the file
from an env var). The path is relative to `crates/datalogic-rs/` because that's
the test binary's cwd; the harness needs both `serde_json` and `templating`
(both included in `--all-features`):

```bash
JSONLOGIC_TEST_FILE=tests/suites/arithmetic/plus.json \
  cargo test -p datalogic-rs --all-features --test test_jsonlogic -- --nocapture
```

Run a feature-gated example:

```bash
cargo run -p datalogic-rs --example getting_started   --features templating
cargo run -p datalogic-rs --example structured_objects --features templating
cargo run -p datalogic-rs --example tracing           --features trace
cargo run -p datalogic-rs --example datetime_ops      --features datetime
cargo run -p datalogic-rs --example error_handling    --features error-handling
cargo run -p datalogic-rs --example zero_copy_input   --features serde_json
```

See [crates/datalogic-rs/examples/README.md](./crates/datalogic-rs/examples/README.md)
for the full table.

## `bindings/wasm` — WebAssembly bindings (browser / Deno / Bun / Workers)

```bash
cd bindings/wasm
./build.sh               # builds web, bundler, and nodejs targets
```

The crate is its own Cargo workspace (see ARCHITECTURE.md for why), so
`cargo` commands inside `bindings/wasm/` operate on it standalone. Run
`cargo test` from inside that directory if you need to test the FFI.
End-user API and install instructions: [bindings/wasm/README.md](./bindings/wasm/README.md).

The WASM build still ships a `nodejs` target — it's the right pick when
a consumer wants one artifact across Node + browser. **For production
Node workloads, prefer the native binding below**; it's noticeably
faster.

## `bindings/node` — Node native binding (napi-rs)

```bash
cd bindings/node
npm install                                   # one-time; pulls @napi-rs/cli
npx napi build --platform --release           # emits datalogic-node.<triple>.node + index.js + index.d.ts
npm test                                      # node --test '__test__/*.test.mjs'
```

This is the **first-class Node target** — published as
`@goplasmatic/datalogic-node` with per-platform `.node` prebuilds
distributed as npm `optionalDependencies`. The `.node` artifact,
`index.js`, and `index.d.ts` are generated by `napi build` and
gitignored; rerun the build after any Rust-side change.

The crate is its own Cargo workspace (matches the wasm/python/c
pattern) — `cargo` commands inside `bindings/node/` don't touch the
root workspace. End-user API and install instructions:
[bindings/node/README.md](./bindings/node/README.md).

## `bindings/python` — Python bindings (pyo3)

```bash
cd bindings/python
maturin develop --release         # build + install into the current venv
pytest                            # run the Python test suite
maturin build --release           # produce a wheel under target/wheels/
```

Like `bindings/wasm`, this crate is its own Cargo workspace (keeps the
pyo3 build deps out of the core `cargo test --workspace` path). End-user
API and install instructions: [bindings/python/README.md](./bindings/python/README.md).

## `bindings/c` — shared C ABI (cbindgen)

```bash
cd bindings/c
cargo build --release             # produces libdatalogic_c.{so,dylib,a}
cargo test                        # smoke-tests the extern "C" surface
```

The C header `include/datalogic.h` is regenerated by cbindgen on every
build. Don't edit by hand — edit `src/` and rebuild. Consumers can set
`DATALOGIC_C_SKIP_CBINDGEN=1` to suppress regeneration.

This crate is **not user-facing**; it's the FFI boundary the Go binding
(and future PHP / JVM bindings) consume. See
[bindings/c/README.md](./bindings/c/README.md) for the API surface and
memory / threading rules.

## `bindings/go` — Go binding (cgo over C ABI)

```bash
cd bindings/go
make build                        # cargo-builds bindings/c, stages lib/<host>/
make test                         # runs `go test -v ./...`
make print-platform               # prints the host's lib/ subdirectory name
```

The Makefile auto-detects host OS/arch and stages into
`lib/<host_os>_<host_arch>/` — only the matching `cgo_*_*.go` file
needs that subdirectory populated locally. Re-run `make build` after
any change to the C ABI's Rust source. End-user API, install
instructions, and prebuilt-library platform matrix:
[bindings/go/README.md](./bindings/go/README.md).

## `ui` — React component

```bash
cd ui
npm install
npm run dev              # local playground, hot reload (auto-syncs WASM)
npm run build            # standalone playground (dist/)
npm run build:lib        # publishable component (dist/)
npm run build:embed      # embeddable widget for the docs site (dist-embed/)
npm run lint
npm run sync-wasm        # manually re-copy ../bindings/wasm/pkg/ → vendor/datalogic/
```

Three Vite configs power the three build modes:

- `vite.config.ts` — playground SPA
- `vite.lib.config.ts` — `@goplasmatic/datalogic-ui` library bundle
- `vite.embed.config.ts` — embeddable widget for docs

The WASM dep is vendored under `ui/vendor/datalogic/` (gitignored),
synced from `bindings/wasm/pkg/` by `sync-wasm`. The `predev` and `prebuild*`
hooks run it automatically, so the typical loop is just:

```bash
cd bindings/wasm && ./build.sh    # rebuild after Rust changes
cd ../../ui && npm run dev        # predev re-vendors the fresh pkg/
```

## Releases

All publishing flows through `.github/workflows/release.yml`, triggered by
pushing a `v*` tag whose version matches `crates/datalogic-rs/Cargo.toml`. The
workflow validates → publishes the crate to crates.io → builds and publishes
every binding (`@goplasmatic/datalogic` WASM, `@goplasmatic/datalogic-node`
napi-rs prebuilds, `datalogic-py` to PyPI, the Go module tag, and
`@goplasmatic/datalogic-ui`) → cuts the GitHub Release. There are no local
publish scripts; do not run `npm publish` or `cargo publish` by hand.

## `tools/benchmark` — performance harness

Dev-only, never published. Two binaries share `src/lib.rs`:

```bash
# datalogic-rs alone, fast arena path
cargo run --release -p datalogic-bench --bin self
cargo run --release -p datalogic-bench --bin self -- --all   # every suite + JSON report

# Cross-library comparison (only datalogic-rs ships by default)
cargo run --release -p datalogic-bench --bin compare -- --all
```

Reports land in `tools/benchmark/output/` (gitignored). To add another
JSONLogic implementation as a comparison subject, see
[tools/benchmark/README.md](./tools/benchmark/README.md).

## Adding a built-in operator

1. Add a variant to `OpCode` in `crates/datalogic-rs/src/opcode.rs` and wire its
   `FromStr` + `as_str()` entries.
2. Implement `evaluate_<op>` under `crates/datalogic-rs/src/operators/<category>/`
   following the established signature
   (`args: &'a [CompiledNode], ctx: &mut DataContextStack<'a>, engine: &Engine, arena: &'a Bump`).
3. Add a dispatch arm in `crates/datalogic-rs/src/engine/dispatch.rs` (or in
   `OpCode::evaluate_direct()` — same path).
4. Add a JSON suite under `crates/datalogic-rs/tests/suites/<category>/` covering
   the happy path and at least one error case. See
   [crates/datalogic-rs/tests/README.md](./crates/datalogic-rs/tests/README.md) for the
   suite format.
5. If you also want it accessible from JS, no further work — the WASM
   wrapper exposes the engine as-is; new operators are picked up
   automatically once you rebuild WASM.

## Adding a custom operator (your own application)

Custom operators (extending the engine from your application code) are
covered in the
[Custom Operators guide](https://goplasmatic.github.io/datalogic-rs/advanced/custom-operators.html)
on the docs site, with a runnable
[`custom_operator` example](./crates/datalogic-rs/examples/custom_operator.rs)
in the core crate.

## Documentation site (`docs/`)

```bash
mdbook serve docs       # live preview at http://localhost:3000
mdbook build docs       # produces docs/book/
```

The published site at https://goplasmatic.github.io/datalogic-rs/ is built
by `.github/workflows/docs.yml` on every push to `main` that touches docs,
WASM, or UI. The workflow also bundles the UI playground and the embed
widget into the rendered book.
