# Architecture

This monorepo ships one logical product — a JSONLogic engine — across three
runtime targets that build on each other:

```
                  +-------------------+
                  |  packages/core    |   Rust crate -> crates.io
                  |  datalogic-rs     |   the engine; everything else wraps it
                  +---------+---------+
                            |
                  uses path = "../core"
                            |
                            v
                  +-------------------+
                  |  packages/wasm    |   wasm-bindgen wrapper -> @goplasmatic/datalogic (npm)
                  |  datalogic-wasm   |   wasm-pack builds three targets: web/bundler/nodejs
                  +---------+---------+
                            |
                  consumed via npm link / npm install
                            |
                            v
                  +-------------------+
                  |  packages/ui      |   React component -> @goplasmatic/datalogic-ui (npm)
                  |                   |   visual debugger + playground
                  +-------------------+

                  +-------------------+
                  |  packages/benchmark |   Rust dev binary, NOT published
                  |  datalogic-bench    |   `self` (single-engine) and `compare` (cross-library)
                  +-------------------+
```

The Rust core is the source of truth for behaviour. WASM is a thin FFI shell
that converts strings at the boundary and re-exposes the engine. The UI
consumes the published WASM (or a locally-linked build) and adds editing,
visualisation, and trace inspection on top.

## Cargo workspace layout

The repo root holds a Cargo workspace with two members:

- `packages/core` — the published crate, `datalogic-rs`.
- `packages/benchmark` — dev-only binaries (`self`, `compare`), `publish = false`.

`packages/wasm` declares its own `[workspace]` table and is excluded from the
parent workspace. This is deliberate: `wasm-pack` needs the WASM-specific
release profile (`opt-level = "z"`, `lto = true`, `panic = "abort"`,
`strip = true`), and Cargo only honours `[profile.*]` at a workspace root.
Keeping WASM as its own workspace lets it apply that profile without
affecting builds of `core` or `benchmark`.

`packages/ui` is a Node package. Cargo ignores it.

## Two-phase evaluation (in `packages/core`)

1. **Compile** — `Engine::compile` parses JSON logic into a `Logic` tree.
   String operator names are resolved to an `OpCode` enum so dispatch at
   eval time is a `match` on a `u8`-sized discriminant. Constant
   sub-expressions are folded; dead branches are elided.
2. **Evaluate** — `Engine::evaluate` walks the compiled tree against an
   input `&DataValue`. Results are `&'a DataValue<'a>` allocated in a
   caller-supplied `bumpalo::Bump` arena. Read-through ops like `var`
   borrow zero-copy directly from the input; arithmetic and reductions
   allocate into the arena.

For high-throughput callers, `Engine::session()` returns a `Session` that
owns a reusable arena and resets it between calls — peak memory tracks the
largest single evaluation, not the sum.

`Logic` is `Send + Sync` and wrapped in `Arc` internally, so a compiled
rule can be shared across threads with no extra setup.

## Feature flags and where they apply

Features are declared on `datalogic-rs` (the core crate). Other crates
opt in via their dependency line.

| Feature           | Effect                                                      | Used by                        |
|-------------------|-------------------------------------------------------------|--------------------------------|
| `compat`          | Enables `serde_json` bridging + 4.x `LegacyApi` shims       | `benchmark`, integration tests |
| `preserve`        | Enables structure-preservation (templating) mode            | examples, WASM                 |
| `datetime`        | Date/time operators (pulls in `chrono`)                     | WASM, `datetime_ops` example   |
| `trace`           | Execution-step recording for the debugger (implies `compat`)| WASM, `tracing` example        |
| `error-handling`  | `try` / `throw` operators                                   | `error_handling` example       |
| `ext-string`, `ext-array`, `ext-control`, `ext-math` | Optional operator families     | opt-in per consumer            |
| `wasm`            | Convenience meta-feature: `datetime + trace + preserve`     | `packages/wasm` only           |

The `datalogic-bench` crate enables `compat` because it reads the
`serde_json` test-suite files; it does not need `wasm` or `preserve`.

## Where things live

| Concern                        | Path                                              |
|--------------------------------|---------------------------------------------------|
| Public Rust API                | `packages/core/src/lib.rs`                        |
| Engine + dispatcher            | `packages/core/src/engine/`                       |
| Compile pipeline + optimiser   | `packages/core/src/compile/`                      |
| OpCode enum (59 builtins)      | `packages/core/src/opcode.rs`                     |
| Operator implementations       | `packages/core/src/operators/`                    |
| Arena value types & context    | `packages/core/src/arena/`                        |
| Rust integration tests         | `packages/core/tests/`                            |
| JSONLogic JSON test suites     | `packages/core/tests/suites/`                     |
| Executable examples            | `packages/core/examples/`                         |
| WASM FFI surface               | `packages/wasm/src/lib.rs`                        |
| WASM build script              | `packages/wasm/build.sh`                          |
| React editor + debugger        | `packages/ui/src/components/logic-editor/`        |
| Benchmark harness              | `packages/benchmark/src/`                         |

For day-to-day commands (build, test, run, link), see [DEVELOPMENT.md](./DEVELOPMENT.md).
