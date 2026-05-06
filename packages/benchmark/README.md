# datalogic-bench

Dev-only benchmark harness for `datalogic-rs`. Two binaries share a common
suite loader and reporter (`src/lib.rs`):

| Binary    | Purpose                                                                                |
|-----------|----------------------------------------------------------------------------------------|
| `self`    | Times datalogic-rs alone using the fast arena path (compile once, persistent input arena, eval-arena reset). Use this to track regressions in our own engine. |
| `compare` | Times multiple JSONLogic implementations through a uniform string-in/string-out `Subject` interface — apples-to-apples across languages and runtimes. |

Both read JSON suites from `packages/core/tests/suites/` and write reports
to `packages/benchmark/output/` (gitignored).

## Run

From the repo root:

```bash
# datalogic-rs alone — single suite (compatible.json by default)
cargo run --release -p datalogic-bench --bin self

# datalogic-rs alone — every suite in tests/suites/index.json
cargo run --release -p datalogic-bench --bin self -- --all

# Cross-library comparison (only datalogic-rs ships by default)
cargo run --release -p datalogic-bench --bin compare -- --all
```

## Adding another subject to `compare`

`compare.rs` defines a `Subject` trait with a single method:

```rust
fn evaluate(&self, rule_json: &str, data_json: &str) -> Result<String, String>;
```

To add a new JSONLogic implementation:

1. Add a Cargo feature in `packages/benchmark/Cargo.toml`, e.g.
   `subject-jsonlogic-rs = ["dep:jsonlogic-rs"]`. Default builds stay
   slim — each subject opts in.
2. Implement `Subject` for it inside `bin/compare.rs`, gated with
   `#[cfg(feature = "subject-jsonlogic-rs")]`.
3. Push it into `subjects()` (also gated).
4. Run with `--features subject-jsonlogic-rs`.

The harness times each subject independently and writes one report per
subject — there is no result-equality check across subjects (different
engines stringify floats and errors differently).
