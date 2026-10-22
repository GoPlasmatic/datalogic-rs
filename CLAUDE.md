# CLAUDE.md

Guidance for Claude Code (claude.ai/code) when working in this repository.

## Project type

Cargo workspace + npm monorepo organized into four areas: **`crates/`**
(Rust core), **`bindings/`** (one folder per language wrapper),
**`ui/`** (React debugger), and **`tools/`** (dev-only).

**Core**: `crates/`

| Path                    | Package                                     | Publishes to  |
|-------------------------|---------------------------------------------|---------------|
| `crates/datalogic-rs`   | `datalogic-rs` (Rust)                       | crates.io     |
| `crates/datalogic-bind` | `datalogic-bind` (shared binding plumbing)  | not published |

**Language bindings**: `bindings/`

| Path              | Package                                   | Publishes to                         |
|-------------------|-------------------------------------------|--------------------------------------|
| `bindings/wasm`   | `@goplasmatic/datalogic-wasm` (WASM)      | npm                                  |
| `bindings/node`   | `@goplasmatic/datalogic-node` (napi-rs)   | npm                                  |
| `bindings/python` | `datalogic-py` (pyo3)                     | PyPI                                 |
| `bindings/c`      | `datalogic-c` (C ABI / cbindgen)          | in-tree only (consumed by Go/JVM/.NET/PHP) |
| `bindings/go`     | `datalogic-go` (cgo over C ABI)           | Go modules (`bindings/go/v*` tag)    |
| `bindings/jvm`    | `io.github.goplasmatic:datalogic` (FFM)   | Maven Central                        |
| `bindings/dotnet` | `Goplasmatic.Datalogic` (P/Invoke)        | NuGet                                |
| `bindings/php`    | `goplasmatic/datalogic` (PHP FFI)         | Packagist                            |

**UI**: `ui/`

| Path | Package                                        | Publishes to |
|------|------------------------------------------------|--------------|
| `ui` | `@goplasmatic/datalogic-ui` (React component)  | npm          |

**Tools**: `tools/`

| Path              | Package                      | Publishes to  |
|-------------------|------------------------------|---------------|
| `tools/benchmark` | `datalogic-bench` (dev-only) | not published |

## Where to read first

- **Project overview & package matrix:** [`README.md`](./README.md)
- **Architecture, design decisions, feature-flag matrix:** [`ARCHITECTURE.md`](./ARCHITECTURE.md)
- **Build / test / run commands per package:** [`DEVELOPMENT.md`](./DEVELOPMENT.md)
- **Contribution flow:** [`CONTRIBUTING.md`](./CONTRIBUTING.md)
- **Security policy & sandbox scope:** [`SECURITY.md`](./SECURITY.md)
- **JSONLogic test-suite schema:** [`crates/datalogic-rs/tests/README.md`](./crates/datalogic-rs/tests/README.md)
- **Cross-binding contract & scenarios:** [`bindings/BINDINGS.md`](./bindings/BINDINGS.md) and [`schemas/README.md`](./schemas/README.md)
- **v4 → v5 and 5.x migration cookbook:** [`MIGRATION.md`](./MIGRATION.md)
- **Documentation book source (`mdbook`):** [`docs/src/SUMMARY.md`](./docs/src/SUMMARY.md)

Do not duplicate the architecture or build pipeline here. Those documents
are authoritative—link to them instead.

## Project-specific conventions

- **Use `make` for anything repo-wide.** Root-level `cargo fmt --all`,
  `cargo clippy --workspace`, and `cargo clean` skip the four Rust bindings and
  the fuzz crate without warning because each is its own workspace root. Run `make help`
  to list the fan-out targets; see
  [DEVELOPMENT.md](./DEVELOPMENT.md#repo-wide-commands).
- **Always pass `--all-features` to `cargo test`.** Most integration tests
  declare `required-features` (at least `serde_json`, and `templating` for the JSONLogic suite
  runner). Without `--all-features`, Cargo skips
  those tests and the run looks green while testing very little.
- **Run a single JSONLogic suite** via the environment variable harness. The path is
  relative to `crates/datalogic-rs/` (the test binary's working directory):
  ```bash
  JSONLOGIC_TEST_FILE=tests/suites/arithmetic/plus.json \
    cargo test -p datalogic-rs --all-features --test test_jsonlogic -- --nocapture
  ```
- **Adding an operator?** Add one row in `operators/table.rs` (with metadata in
  braces over the family's preset, e.g., `{ cost: Cost::Node }`), one function
  under `operators/`, and one JSON suite under `tests/suites/`. The `OpCode`, names,
  dispatch, and fold/CSE/scope classification are all generated from the table
  row. Then regenerate the operator catalogue:
  `UPDATE_OPERATORS_JSON=1 cargo test -p datalogic-rs --all-features --test operators_json_test`.
  Follow the step-by-step guide in
  [`DEVELOPMENT.md`](./DEVELOPMENT.md#adding-a-built-in-operator).
- **Operator function signature:**
  - `eager(..)` rows use a typed body: `fn op<'a>(cx: &mut Cx<'_, 'a>, a: &'a str, ...) -> Result<impl IntoValue>`.
  - `raw` rows take `(args: &'a [CompiledNode], ctx: &mut ContextStack<'a>, engine: &Engine, arena: &'a Bump) -> Result<&'a DataValue<'a>>`.
  - `iter` rows add `iter_arg_kind: IterArgKind` after `args`.
  - `each` rows take `items: Items<'a>` first (the resolved, non-empty source).
  - Rows written `@ Kind(payload)` pass `payload` last.
  - Declare the argument count on the row (`raw[2..]`, `each[2]`), not inside the function body.
- **Test-suite JSON format:** an array of test-case objects with
  `description`, `rule`, optional `data` (default `{}`), and either
  `result` or `error`. Plain strings in the array act as section headers. Register any new
  suite file in `tests/suites/index.json` by running:
  `UPDATE_SUITE_INDEX=1 cargo test -p datalogic-rs --all-features --test test_jsonlogic suite_index`. See the full schema in
  [`crates/datalogic-rs/tests/README.md`](./crates/datalogic-rs/tests/README.md).
- **Isolated Cargo workspaces for bindings:** `bindings/wasm/`, `bindings/python/`, `bindings/c/`, and `bindings/node/` each have their own `Cargo.toml` workspace for release-profile and build-dependency isolation. Running `cargo` inside those directories operates standalone and does not touch the root workspace.
- **Node binding builds via `napi-cli`, not raw `cargo`.** Inside
  `bindings/node/`, run `npx napi build --platform --release` (which
  emits `datalogic-node.<triple>.node`, `index.js`, and `index.d.ts`) followed by
  `npm test` (which runs `node --test __test__/*.test.mjs`). The
  generated `.node`, `index.js`, and `index.d.ts` files are gitignored.
