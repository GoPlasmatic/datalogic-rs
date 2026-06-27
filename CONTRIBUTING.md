# Contributing to datalogic-rs

Thanks for your interest in contributing! This file is the contribution
workflow itself — for the cross-package layout see
[README.md](./README.md), for design and dependency flow see
[ARCHITECTURE.md](./ARCHITECTURE.md), for build / test / run commands
per package see [DEVELOPMENT.md](./DEVELOPMENT.md).

Most contributions touch only the Rust crate. You only need the
WASM / Python / Go / UI toolchains if you are changing those layers or
verifying an end-to-end change in the React debugger.

---

## Prerequisites

- **Rust** 1.85 or newer (`rustup update stable`) — the core crate uses `edition = "2024"`
- **wasm-pack** — only if you are rebuilding WASM
  (`curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh`)
- **Node.js** 20+ — only if you are working on `ui/`, `bindings/node/` or `bindings/wasm/`
- **Python 3.10+** with [`maturin`](https://www.maturin.rs/) — only if you are working on `bindings/python/`
- **Go 1.22+** and a C compiler — only if you are working on `bindings/go/`
- **Java JDK 11+ & Maven** — only if you are working on `bindings/jvm/`
- **.NET SDK 8.0+** — only if you are working on `bindings/dotnet/`
- **PHP 8.4+ & Composer** — only if you are working on `bindings/php/`
- **mdbook** — only if you are building the docs site
  (`cargo install mdbook`)

## Setup

```bash
git clone https://github.com/GoPlasmatic/datalogic-rs.git
cd datalogic-rs

# Rust-only workflow — most contributions stop here.
# --all-features unlocks the full test surface; without it most
# integration tests skip silently (they require feature = "serde_json").
cargo test --workspace --all-features
```

For the full Rust → WASM → UI link dance, and per-binding build
commands, see [DEVELOPMENT.md](./DEVELOPMENT.md).

---

## Code style

- `cargo fmt` and `cargo clippy --workspace --all-targets -- -D warnings`
  must pass before opening a PR. CI enforces this.
- Public items should have rustdoc. Examples in rustdoc should compile
  (they run under `cargo test --doc`).
- Prefer editing existing files over adding new ones. Keep comments
  focused on *why*, not *what*.

## Writing tests

There are two complementary test systems in `crates/datalogic-rs/tests/`:

- **Rust integration tests** in `crates/datalogic-rs/tests/*.rs` (e.g.
  `basic_test.rs`, `config_test.rs`, `trace_test.rs`). Use these for
  engine-level behaviour, configuration, tracing, custom operators, and
  anything that needs Rust-specific setup.
- **JSONLogic suites** in `crates/datalogic-rs/tests/suites/*.json`. Use these
  for any new JSONLogic operator or edge case — they double as the
  canonical behaviour spec and are replayable in the playground.

A suite entry looks like:

```json
{
  "description": "Addition with variables",
  "rule": { "+": [{ "var": "x" }, { "var": "y" }] },
  "data": { "x": 1, "y": 2 },
  "result": 3
}
```

Error cases use `"error": { "type": "NaN" }` instead of `"result"`. See
[crates/datalogic-rs/tests/README.md](./crates/datalogic-rs/tests/README.md)
for the full schema.

## Adding an operator

- **Built-in operator** (ships in the crate, gets an `OpCode`): canonical
  step-by-step is in
  [DEVELOPMENT.md → Adding a built-in operator](./DEVELOPMENT.md#adding-a-built-in-operator).
- **Custom operator** (your own application extends the engine): implement
  `CustomOperator`, register on `Engine::builder().add_operator(...)`. See
  the [`custom_operator` example](./crates/datalogic-rs/examples/custom_operator.rs)
  and the [Custom operators section in the crate README](./crates/datalogic-rs/README.md#custom-operators).

## Debugging rules

Enable the `trace` feature on the Rust crate to record every evaluation
step, then inspect the trace programmatically (Rust) or visually (the
React debugger). See the [Tier 4 example in the crate README](./crates/datalogic-rs/README.md#tier-4--traced-evaluation-trace-feature)
for the Rust pattern, or drop into
[`@goplasmatic/datalogic-ui`](./ui/README.md) for the visual debugger.

---

## Submitting a PR

1. Fork and create a topic branch.
2. Make your change. Add or update tests.
3. Run `cargo fmt && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo test --workspace --all-features`.
   If you touched WASM / Python / Go / UI, also run the relevant build
   scripts ([DEVELOPMENT.md](./DEVELOPMENT.md) has the commands).
4. Open a PR with a description of the *why* and a short test plan.

Architectural notes live in [ARCHITECTURE.md](./ARCHITECTURE.md).
Questions and proposals are welcome via GitHub issues.
