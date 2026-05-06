# Development

A walkthrough of working on each package in this monorepo. For the big
picture (what depends on what, why the layout is shaped this way), see
[ARCHITECTURE.md](./ARCHITECTURE.md).

## Prerequisites

| Tool       | Version | Why                                                             |
|------------|---------|-----------------------------------------------------------------|
| Rust       | 1.85+   | The core crate uses `edition = "2024"`                          |
| `wasm-pack`| latest  | Builds `packages/wasm` (only needed for WASM/UI changes)        |
| Node.js    | 20+     | Builds and runs `packages/ui`                                   |
| `mdbook`   | latest  | Builds the docs site under `docs/`                              |

```bash
rustup update stable
curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
# Node via your package manager of choice
cargo install mdbook   # only if you are editing docs/
```

## The build pipeline

The packages have a strict build order. From a fresh clone:

```bash
# 1. Rust workspace — runs core unit/integration tests and the bench crate's checks.
# Most integration tests are gated behind feature = "compat"; the JSONLogic
# runner additionally needs feature = "preserve". --all-features unlocks both.
cargo test --workspace --all-features

# 2. WASM bindings — produces packages/wasm/pkg/{web,bundler,nodejs}.
cd packages/wasm && ./build.sh && cd ../..

# 3. UI — needs the locally-built WASM linked into node_modules first.
cd packages/wasm/pkg && npm link
cd ../../ui && npm link @goplasmatic/datalogic && npm install
npm run dev   # or: npm run build:lib for the publishable bundle
```

The `npm link` step is what wires the *just-built* WASM into the UI; without
it, `npm install` would pull `@goplasmatic/datalogic` from the registry and
silently mask any local Rust changes you wanted to test.

## `packages/core` — Rust library

```bash
cargo check -p datalogic-rs
cargo test  -p datalogic-rs                        # default features
cargo test  -p datalogic-rs --all-features         # everything
cargo fmt   --all
cargo clippy --workspace --all-targets -- -D warnings
```

Run a single JSONLogic suite (the `test_jsonlogic` harness picks the file
from an env var). The path is relative to `packages/core/` because that's
the test binary's cwd; the harness needs both `compat` and `preserve`:

```bash
JSONLOGIC_TEST_FILE=tests/suites/arithmetic/plus.json \
  cargo test -p datalogic-rs --all-features --test test_jsonlogic -- --nocapture
```

Run a feature-gated example:

```bash
cargo run -p datalogic-rs --example getting_started --features preserve
cargo run -p datalogic-rs --example tracing         --features trace
cargo run -p datalogic-rs --example datetime_ops    --features datetime
cargo run -p datalogic-rs --example error_handling  --features error-handling
cargo run -p datalogic-rs --example migrating_from_v4 --features compat
```

See [packages/core/examples/README.md](./packages/core/examples/README.md)
for the full table.

## `packages/wasm` — WebAssembly bindings

```bash
cd packages/wasm
./build.sh               # builds web, bundler, and nodejs targets
./publish.sh             # only used by CI; talks to npm
```

The crate is its own Cargo workspace (see ARCHITECTURE.md for why), so
`cargo` commands inside `packages/wasm/` operate on it standalone. Run
`cargo test` from inside that directory if you need to test the FFI.

## `packages/ui` — React component

```bash
cd packages/ui
npm install
npm run dev              # local playground, hot reload
npm run build            # standalone playground (dist/)
npm run build:lib        # publishable component (dist/)
npm run build:embed      # embeddable widget for the docs site (dist-embed/)
npm run lint
```

Three Vite configs power the three build modes:

- `vite.config.ts` — playground SPA
- `vite.lib.config.ts` — `@goplasmatic/datalogic-ui` library bundle
- `vite.embed.config.ts` — embeddable widget for docs

If you change the Rust core, rebuild WASM (`cd packages/wasm && ./build.sh`)
before reloading the UI dev server — the linked package picks up the new
artifacts automatically.

## `packages/benchmark` — performance harness

Dev-only, never published. Two binaries share `src/lib.rs`:

```bash
# datalogic-rs alone, fast arena path
cargo run --release -p datalogic-bench --bin self
cargo run --release -p datalogic-bench --bin self -- --all   # every suite + JSON report

# Cross-library comparison (only datalogic-rs ships by default)
cargo run --release -p datalogic-bench --bin compare -- --all
```

Reports land in `packages/benchmark/output/` (gitignored). To add another
JSONLogic implementation as a comparison subject, see
[packages/benchmark/README.md](./packages/benchmark/README.md).

## Adding a built-in operator

1. Add a variant to `OpCode` in `packages/core/src/opcode.rs` and wire its
   `FromStr` + `as_str()` entries.
2. Implement `evaluate_<op>` under `packages/core/src/operators/<category>/`
   following the established signature
   (`args: &'a [CompiledNode], ctx: &mut DataContextStack<'a>, engine: &Engine, arena: &'a Bump`).
3. Add a dispatch arm in `packages/core/src/engine/dispatch.rs` (or in
   `OpCode::evaluate_direct()` — same path).
4. Add a JSON suite under `packages/core/tests/suites/<category>/` covering
   the happy path and at least one error case. See
   [packages/core/tests/README.md](./packages/core/tests/README.md) for the
   suite format.
5. If you also want it accessible from JS, no further work — the WASM
   wrapper exposes the engine as-is; new operators are picked up
   automatically once you rebuild WASM.

## Adding a custom operator (your own application)

Implement `CustomOperator` and register on the builder:

```rust
let engine = Engine::builder()
    .add_operator("my_op", MyOp)
    .build();
```

Args arrive *pre-evaluated* as `&'a DataValue<'a>`. Allocate the result
into the supplied arena. See
[`packages/core/examples/custom_operator.rs`](./packages/core/examples/custom_operator.rs)
for a complete example, and the `CustomOperator` rustdoc for lifetime
notes.

## Documentation site (`docs/`)

```bash
mdbook serve docs       # live preview at http://localhost:3000
mdbook build docs       # produces docs/book/
```

The published site at https://goplasmatic.github.io/datalogic-rs/ is built
by `.github/workflows/docs.yml` on every push to `main` that touches docs,
WASM, or UI. The workflow also bundles the UI playground and the embed
widget into the rendered book.
