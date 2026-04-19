# Contributing to datalogic-rs

Thanks for your interest in contributing! This monorepo contains three
packages that ship together:

| Path       | Package                      | Publishes to |
|------------|------------------------------|--------------|
| `/` (root) | `datalogic-rs` (Rust)        | crates.io    |
| `/wasm`    | `@goplasmatic/datalogic`     | npm          |
| `/ui`      | `@goplasmatic/datalogic-ui`  | npm          |

Most contributions touch only the Rust crate. You only need the WASM / UI
toolchains if you are changing those layers or verifying an end-to-end
change in the React debugger.

---

## Prerequisites

- **Rust** 1.70 or newer (`rustup update stable`)
- **pnpm** 9.15 or newer — for the WASM and UI packages
- **wasm-pack** — only needed if you are rebuilding WASM
  (`curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh`)
- **mdbook** — only needed if you are building the docs site
  (`cargo install mdbook`)

## Setup

```bash
git clone https://github.com/GoPlasmatic/datalogic-rs.git
cd datalogic-rs

# Rust-only workflow
cargo test

# Full workflow (Rust → WASM → UI)
pnpm install
pnpm build:all
```

---

## Development loop

### Rust

```bash
cargo check                   # fast feedback
cargo test                    # unit tests + JSONLogic suites
cargo test --doc              # verify rustdoc examples compile
cargo fmt                     # format
cargo clippy -- -D warnings   # lint (CI enforces zero warnings)
```

To run a single JSONLogic suite:

```bash
JSONLOGIC_TEST_FILE=tests/suites/arithmetic/plus.json \
  cargo test test_jsonlogic -- --nocapture
```

### WASM

```bash
cd wasm && ./build.sh         # or: pnpm build:wasm (from root)
```

This generates `wasm/pkg/` (the published `@goplasmatic/datalogic`).

### UI

```bash
pnpm dev:ui                   # dev server with hot reload
pnpm build:ui:lib             # library build for publishing
pnpm lint:ui                  # lint the React code
```

The UI consumes the local WASM build, so rebuild WASM first if you change
anything in the Rust crate.

---

## Code style

- `cargo fmt` and `cargo clippy` must pass with no warnings before opening
  a PR. CI will fail otherwise.
- Public items should have rustdoc. Examples in rustdoc should compile
  (they run under `cargo test --doc`).
- Prefer editing existing files over adding new ones. Keep comments
  focused on *why*, not *what*.

## Writing tests

There are two complementary test systems:

- **Rust unit tests** live in `tests/*.rs` (e.g. `basic_test.rs`,
  `config_test.rs`, `trace_test.rs`). Use these for engine-level
  behavior, configuration, tracing, custom operators, and anything that
  needs Rust-specific setup.
- **JSONLogic suites** live in `tests/suites/*.json`. Use these for any
  new JSONLogic operator or edge case — they double as the canonical
  behavior spec and are replayable in the playground.

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
[CLAUDE.md](./CLAUDE.md) for the full schema.

## Adding a built-in operator

Built-in operators use a fast OpCode dispatch path. Adding one requires
edits in two files:

1. `src/opcode.rs` — add an `OpCode` variant, plus entries in the
   `FromStr` and `as_str()` implementations, plus a dispatch arm in
   `OpCode::evaluate_direct()`.
2. `src/operators/<category>.rs` — implement
   `pub fn evaluate_<op>(args, context, engine) -> Result<Value>`.
3. Add a JSON suite under `tests/suites/<category>/` covering the happy
   path and at least one error case.

If your operator is domain-specific, prefer a **custom operator** via the
`Operator` trait instead — see `examples/custom_operator.rs`.

## Adding a custom operator (user-facing extension)

If you are extending datalogic-rs in your own application, implement the
`Operator` trait and register via `DataLogic::add_operator`. The trait
receives **unevaluated** arguments — call
`evaluator.evaluate(&arg, context)` to resolve each one. See the trait
docs and `examples/custom_operator.rs` for patterns.

---

## Debugging rules

The `trace` feature (enabled by default) records every evaluation step:

```rust
let engine = DataLogic::new();
let traced = engine.evaluate_json_with_trace(
    r#"{"if": [{">": [{"var": "age"}, 18]}, "adult", "minor"]}"#,
    r#"{"age": 21}"#,
)?;

for step in &traced.steps {
    println!("{:?}", step);
}
```

From JavaScript, call `evaluate_with_trace` from `@goplasmatic/datalogic`
or drop the `@goplasmatic/datalogic-ui` debugger into your app.

---

## Submitting a PR

1. Fork and create a topic branch.
2. Make your change. Add or update tests.
3. Run `cargo fmt && cargo clippy -- -D warnings && cargo test`. If you
   touched WASM/UI, also run the relevant `pnpm` commands.
4. Open a PR with a description of the *why* and a short test plan.

Architectural notes live in [CLAUDE.md](./CLAUDE.md). Questions and
proposals are welcome via GitHub issues.
