# CLAUDE.md

Guidance for Claude Code (claude.ai/code) when working in this repository.

## Project type

Cargo workspace + npm monorepo organized into four areas: **`crates/`**
(Rust core), **`bindings/`** (one folder per language wrapper),
**`ui/`** (React debugger), and **`tools/`** (dev-only).

**Core**: `crates/`

| Path                  | Package               | Publishes to |
|-----------------------|-----------------------|--------------|
| `crates/datalogic-rs` | `datalogic-rs` (Rust) | crates.io    |

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

- **Architecture, design decisions, feature-flag matrix:** [`ARCHITECTURE.md`](./ARCHITECTURE.md)
- **Build / test / run commands per package:** [`DEVELOPMENT.md`](./DEVELOPMENT.md)
- **Contribution flow:** [`CONTRIBUTING.md`](./CONTRIBUTING.md)
- **JSONLogic test-suite schema:** [`crates/datalogic-rs/tests/README.md`](./crates/datalogic-rs/tests/README.md)
- **v4 → v5 migration cookbook:** [`MIGRATION.md`](./MIGRATION.md)

Don't re-explain the architecture or build pipeline here. Those documents
are authoritative; link to them.

## Project-specific conventions

- **Use `make` for anything repo-wide.** Root-level `cargo fmt --all`,
  `cargo clippy --workspace` and `cargo clean` silently skip the four
  bindings and the fuzz crate (each is its own workspace root). `make help`
  lists the fan-out targets; see
  [DEVELOPMENT.md](./DEVELOPMENT.md#repo-wide-commands).
- **Always pass `--all-features` to `cargo test`.** Most integration tests
  are gated behind `feature = "serde_json"` and the JSONLogic suite runner
  also needs `feature = "templating"`. Without `--all-features`, tests
  silently skip and you'll think they passed.
- **Run a single JSONLogic suite** via the env-var harness. The path is
  relative to `crates/datalogic-rs/` (the test binary's cwd):
  ```bash
  JSONLOGIC_TEST_FILE=tests/suites/arithmetic/plus.json \
    cargo test -p datalogic-rs --all-features --test test_jsonlogic -- --nocapture
  ```
- **Adding an operator?** One row in `operators/table.rs`, one function
  under `operators/<category>/`, one JSON suite under `tests/suites/`.
  OpCode, names, dispatch and the fold/CSE/scope classification are
  generated from the row. See the canonical step-by-step in
  [`DEVELOPMENT.md`](./DEVELOPMENT.md#adding-a-built-in-operator).
- **Operator function signature:** an `eager(..)` row's body is typed,
  `fn op<'a>(cx: &mut Cx<'_, 'a>, a: &'a str, ...) -> Result<impl IntoValue>`.
  `raw` rows take
  `(args: &'a [CompiledNode], ctx: &mut ContextStack<'a>, engine: &Engine, arena: &'a Bump) -> Result<&'a DataValue<'a>>`;
  `iter` rows add `iter_arg_kind: IterArgKind` after `args`; `each` rows
  take `items: Items<'a>` first (the resolved, non-empty source); a row
  written `@ Kind(payload)` passes `payload` last. Declare the argument
  count on the row (`raw[2..]`, `each[2]`), not in the body.
- **Test-suite JSON format:** array of test-case objects with
  `description`, `rule`, `data`, and either `result` or `error`. Strings
  in the array are skipped (used as section headers). Full schema in
  [`crates/datalogic-rs/tests/README.md`](./crates/datalogic-rs/tests/README.md).
- **WASM is its own Cargo workspace** for release-profile isolation:
  `cargo` commands inside `bindings/wasm/` operate on it standalone and do
  not touch the root workspace. The Python, C, and Node bindings follow
  the same pattern.
- **Node binding builds via napi-cli, not raw cargo.** Inside
  `bindings/node/`, use `npx napi build --platform --release` (which
  emits `datalogic-node.<triple>.node`, `index.js`, and `index.d.ts`) and
  `npm test` (which runs `node --test '__test__/*.test.mjs'`). The
  generated `.node` / `index.js` / `index.d.ts` are gitignored.
