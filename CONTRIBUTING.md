# Contributing to datalogic-rs

Thanks for your interest in contributing! This is a Cargo workspace + npm
monorepo. The packages live under `packages/`:

| Path                  | Package                          | Publishes to |
|-----------------------|----------------------------------|--------------|
| `packages/core`       | `datalogic-rs` (Rust)            | crates.io    |
| `packages/wasm`       | `@goplasmatic/datalogic`         | npm          |
| `packages/ui`         | `@goplasmatic/datalogic-ui`      | npm          |
| `packages/benchmark`  | `datalogic-bench` (dev-only)     | —            |

For a full picture of how the packages depend on each other, see
[ARCHITECTURE.md](./ARCHITECTURE.md). For day-to-day commands, the build
order, and the WASM/UI link dance, see [DEVELOPMENT.md](./DEVELOPMENT.md) —
this file focuses on the contribution workflow itself.

Most contributions touch only the Rust crate. You only need the WASM / UI
toolchains if you are changing those layers or verifying an end-to-end
change in the React debugger.

---

## Prerequisites

- **Rust** 1.85 or newer (`rustup update stable`) — the core crate uses `edition = "2024"`
- **wasm-pack** — only needed if you are rebuilding WASM
  (`curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh`)
- **Node.js** 20+ — only needed for `packages/ui`
- **mdbook** — only needed if you are building the docs site
  (`cargo install mdbook`)

## Setup

```bash
git clone https://github.com/GoPlasmatic/datalogic-rs.git
cd datalogic-rs

# Rust-only workflow (--all-features unlocks the full test surface; without
# it most integration tests skip silently because they require feature = "compat")
cargo test --workspace --all-features

# Full workflow (Rust → WASM → UI), see DEVELOPMENT.md for the npm link step
cd packages/wasm && ./build.sh
cd ../wasm/pkg && npm link
cd ../../ui && npm link @goplasmatic/datalogic && npm install && npm run dev
```

---

## Code style

- `cargo fmt` and `cargo clippy --workspace --all-targets -- -D warnings`
  must pass before opening a PR. CI enforces this.
- Public items should have rustdoc. Examples in rustdoc should compile
  (they run under `cargo test --doc`).
- Prefer editing existing files over adding new ones. Keep comments
  focused on *why*, not *what*.

## Writing tests

There are two complementary test systems in `packages/core/tests/`:

- **Rust integration tests** in `packages/core/tests/*.rs` (e.g.
  `basic_test.rs`, `config_test.rs`, `trace_test.rs`). Use these for
  engine-level behaviour, configuration, tracing, custom operators, and
  anything that needs Rust-specific setup.
- **JSONLogic suites** in `packages/core/tests/suites/*.json`. Use these
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
[packages/core/tests/README.md](./packages/core/tests/README.md) for the
full schema.

## Adding a built-in operator

Built-in operators use a fast OpCode dispatch path. Adding one requires
edits in three places:

1. `packages/core/src/opcode.rs` — add an `OpCode` variant, plus entries
   in the `FromStr` and `as_str()` implementations, plus a dispatch arm
   in `OpCode::evaluate_direct()`.
2. `packages/core/src/operators/<category>.rs` — implement
   `pub(crate) fn evaluate_<op><'a>(args, ctx, engine, arena) -> Result<&'a DataValue<'a>>`.
3. Add a JSON suite under `packages/core/tests/suites/<category>/`
   covering the happy path and at least one error case. Register the
   suite in `packages/core/tests/suites/index.json`.

If your operator is domain-specific, prefer a **custom operator** via the
`CustomOperator` trait instead — see
[`packages/core/examples/custom_operator.rs`](./packages/core/examples/custom_operator.rs).

## Adding a custom operator (user-facing extension)

If you are extending datalogic-rs in your own application, implement the
`CustomOperator` trait and register on the builder
(`Engine::builder().add_operator(...)`). Args arrive **pre-evaluated** as
arena-resident `&'a DataValue<'a>` borrows; allocate the result back into
the supplied arena. See the trait docs in `lib.rs` and the example for
patterns.

---

## Debugging rules

The `trace` feature records every evaluation step:

```rust
// Cargo.toml: datalogic-rs = { version = "5", features = ["trace"] }
use datalogic_rs::Engine;

let engine = Engine::new();
let run = engine.trace().evaluate_str(
    r#"{"if": [{">": [{"var": "age"}, 18]}, "adult", "minor"]}"#,
    r#"{"age": 21}"#,
);

println!("result: {}", run.result.unwrap());   // "adult"
println!("{} steps recorded", run.steps.len());
```

From JavaScript, call `evaluate_with_trace` from `@goplasmatic/datalogic`
or drop the `@goplasmatic/datalogic-ui` debugger into your app.

---

## Submitting a PR

1. Fork and create a topic branch.
2. Make your change. Add or update tests.
3. Run `cargo fmt && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo test --workspace --all-features`.
   If you touched WASM/UI, also run the relevant build scripts.
4. Open a PR with a description of the *why* and a short test plan.

Architectural notes live in [ARCHITECTURE.md](./ARCHITECTURE.md). Questions
and proposals are welcome via GitHub issues.
