# datalogic-c

C ABI for the [`datalogic-rs`](https://github.com/GoPlasmatic/datalogic-rs/tree/main/crates/datalogic-rs) JSONLogic engine. This crate is
the FFI boundary the Go, JVM, .NET and PHP bindings under `bindings/`
consume.

> **Not a user-facing binding.** If you're writing a Go, JVM, .NET, or
> PHP application, use the corresponding language binding; see the
> [repo README](https://github.com/GoPlasmatic/datalogic-rs#readme) for
> the list. This crate is for authors of *new* language bindings that
> want to route through a shared FFI surface.

The Rust source lives in `src/`. **cbindgen generates** the C header
`include/datalogic.h` on every `cargo build`, and the repo commits it so
downstream language packages don't need a cbindgen toolchain. Don't
edit the header by hand; edit `src/` and rebuild.

## Build

```bash
cd bindings/c
cargo build --release
# Artifacts:
#   target/release/libdatalogic_c.{so,dylib,a}     (cdylib + staticlib)
#   include/datalogic.h                            (regenerated)
```

The Go binding builds the staticlib on its own with the `go-release`
profile (no debuginfo, fat LTO); see `bindings/go/Makefile`.
`examples/` holds three C programs (`getting-started`,
`custom-operator`, `compile-once-evaluate-many`) that `make run` builds
against the release cdylib.

## Test

```bash
cargo test
```

| Test | Covers |
|---|---|
| `tests/smoke.rs` | The `extern "C"` contract mechanics: ABI version, status codes, error handles, borrowed session results, owned bufs, data handles, typed results, batch, callbacks, NULL-safety |
| `tests/conformance.rs` | The 2,128-case conformance suite (66 suites) through the ABI, each case evaluated twice: once via one-shot `apply` and once via compile + data handle + session |
| `tests/introspect.rs` | The minor-1 entry points: compile modes, `check`, `compile_checked`, `operators`, `truthy`, rule facts, metered sessions, the builder's escape and strict names, the new error accessors |
| `tests/scenarios.rs` | The cross-binding scenarios in `bindings/scenarios/api.json` |
| `tests/header_sync.rs` | PHP's FFI header, the JVM's downcall handles and .NET's imports declare what `include/datalogic.h` declares |

## API surface

See [`include/datalogic.h`](https://github.com/GoPlasmatic/datalogic-rs/blob/main/bindings/c/include/datalogic.h). High-level shape (ABI **v2**, minor **1**):

| Group | Functions |
|---|---|
| Meta | `datalogic_abi_version` (must equal `DATALOGIC_ABI_VERSION` = 2), `datalogic_abi_minor` (`DATALOGIC_ABI_MINOR` = 1), `datalogic_version`, `datalogic_buf_free` |
| Engine | `datalogic_engine_new` / `_free` / `_compile` / `_compile_mode` / `_compile_checked` / `_check` / `_operators` / `_truthy` / `_apply` / `_session` / `_traced_session` |
| Builder | `datalogic_engine_builder_new` / `_free` / `_set_templating` / `_set_config_json` / `_set_families` / `_set_template_key_escape` / `_set_strict_operator_names` / `_add_operator` / `_build`; callbacks write results via `datalogic_op_result_set_json` / `_set_error` |
| Data | `datalogic_data_parse` / `_free` / `_allocated_bytes`; parse once, evaluate many |
| Rule | `datalogic_rule_free` / `_evaluate` / `_evaluate_data` / `_facts` |
| Session | `datalogic_session_free` / `_reset` / `_allocated_bytes` / `_evaluate` / `_evaluate_metered` / `_evaluate_data` / `_evaluate_bool` / `_evaluate_i64` / `_evaluate_f64` / `_evaluate_truthy` / `_evaluate_batch` / `_evaluate_many` |
| Traced | `datalogic_traced_session_free` / `_evaluate` / `_evaluate_mode` |
| Errors | `datalogic_error_free` / `_status` / `_message` / `_tag` / `_operator` / `_path_json` / `_node_ids_json` / `_diagnostics_json` |

`datalogic_status` is the coarse outcome every fallible call returns:

| Status | Meaning |
|---|---|
| `DATALOGIC_STATUS_OK` (0) | Success |
| `DATALOGIC_STATUS_INVALID_ARG` (1) | A NULL handle or pointer, invalid UTF-8, a rule from a different engine than the session's, a session already in use, or a builder setter after `_build` |
| `DATALOGIC_STATUS_PARSE` (2) | Rule, data or config JSON failed to parse, or `_compile_checked` refused the rule (tag `"CompileError"`) |
| `DATALOGIC_STATUS_EVAL` (3) | Evaluation failed; the tag carries the detail |
| `DATALOGIC_STATUS_TYPE_MISMATCH` (4) | A typed-result call evaluated but the result has another type |
| `DATALOGIC_STATUS_INTERNAL` (5) | A panic caught at the FFI boundary |

`datalogic_mode` (`DATALOGIC_MODE_ENGINE` 0, `_STRICT` 1, `_TEMPLATE` 2)
selects how `_compile_mode`, `_check` and `_traced_session_evaluate_mode`
read a rule, passed as a `uint32_t`.

### Contract (v2)

- **ABI check first**: call `datalogic_abi_version()` once at load and
  refuse to run unless it returns `2`; a mismatch means a stale
  library, and the wrapper must fail initialisation. A wrapper that calls
  entry points added in minor `n` also checks
  `datalogic_abi_minor() >= n`. Minor 1 (5.8) added the compile modes,
  `_compile_checked`, `_check`, `_operators`, `_truthy`,
  `datalogic_rule_facts`, `datalogic_session_evaluate_metered`,
  `_set_families`, `_set_template_key_escape`,
  `_set_strict_operator_names`, `_node_ids_json`, `_diagnostics_json`
  and `datalogic_traced_session_evaluate_mode`.
- **Inputs** are `(pointer, length)` UTF-8 byte ranges, **not**
  NUL-terminated.
- **Fallible calls** return `datalogic_status` and take a trailing
  `datalogic_error **err`. Pass `NULL` to skip capture; otherwise
  release the stored handle via `datalogic_error_free` after reading
  the accessors (`_status` / `_message` / `_tag` / `_operator` /
  `_path_json` / `_node_ids_json` / `_diagnostics_json`). There is
  **no thread-local error state**. `_operator` names the innermost
  failing operator; `_diagnostics_json` is set only for tag
  `"CompileError"`.
- **Session results are borrowed**: `datalogic_session_evaluate*`
  return pointers into a session-owned buffer, valid until the next
  call touching that session. Copy immediately.
- **One-shot results are owned** `datalogic_buf` values; release via
  `datalogic_buf_free`.
- **`engine` / `rule` / `data` / `traced_session`** handles are
  thread-safe; **`session`** is not, so open one per thread. A rule,
  session or traced session holds its own reference on the engine, so
  freeing the engine first is fine.
- **Custom operators** receive `(args_ptr, args_len, user_data, out)`,
  write their outcome via `datalogic_op_result_set_json` /
  `_set_error` (both copy immediately), and return `0` / non-zero. No
  allocator crosses the boundary in either direction. A name a
  built-in answers to never dispatches; after
  `_set_strict_operator_names(b, 1)`, `_add_operator` refuses such a
  name with tag `"ConfigurationError"` instead.
- **A callback must not re-enter the session running it.** That session
  is in use until the callback returns: its evaluate calls fail with
  `DATALOGIC_STATUS_INVALID_ARG`, `reset` / `free` on it do nothing,
  and `allocated_bytes` returns 0. Open a second session, or use the
  session-less `datalogic_rule_*` calls, for a nested evaluation.
- **Builder setters after `_build`** fail with
  `DATALOGIC_STATUS_INVALID_ARG` where they return a status
  (`_set_config_json`, `_set_families`, `_set_template_key_escape`,
  `_add_operator`); `_set_templating` and `_set_strict_operator_names`
  return nothing and do nothing on a built builder.
- **Diagnostics are data**: `_check` returns `DATALOGIC_STATUS_OK` with
  every problem in a JSON array of `{code, severity, message, pointer,
  operator}`; `_compile_checked` refuses a rule with an error
  diagnostic.
- **Metered evaluation**: `datalogic_session_evaluate_metered` takes a
  `budget`, where `0` means the engine's configured `ops_budget` (or
  unbounded when it has none), and reports the operations charged in
  `*out_ops`. Crossing the budget fails with tag `"BudgetExceeded"`.
- **Batch items** fail one at a time: `out_statuses[i]` carries the
  item's status and `out_results[i]` the result JSON or
  `{"tag", "message", "operator"?}`.
- **Traced runs** return `{result, expression_tree, steps, error?,
  structured_error?, pointers?}` with `DATALOGIC_STATUS_OK` even when
  the rule fails; `result` keeps the object key order the rule
  produced, and `pointers` maps each node id to the JSON Pointer of the
  rule value it was compiled from.

v2 replaced v1 (NUL-terminated strings, `datalogic_string_free`, the
thread-local `datalogic_last_error_*` block) in 5.0.1; see the C ABI
section of
[`MIGRATION.md`](https://github.com/GoPlasmatic/datalogic-rs/blob/main/MIGRATION.md).

## Consumers

| Binding | Path | Mechanism |
|---|---|---|
| Go | `bindings/go/` | `cgo` over the staticlib |
| JVM | `bindings/jvm/` | `java.lang.foreign` (FFM, JDK 22+) over the cdylib |
| .NET | `bindings/dotnet/` | P/Invoke (`LibraryImport`) over the cdylib |
| PHP | `bindings/php/` | PHP FFI over the cdylib (preloaded `FFI::load` scope, `FFI::cdef` fallback) |

The Python (`bindings/python/`), Node (`bindings/node/`) and WASM
(`bindings/wasm/`) bindings target their language runtimes through
pyo3, napi-rs and wasm-bindgen and do **not** route through this C ABI.
All of them share the JSON wire formats in `crates/datalogic-bind`.

## Skipping cbindgen during downstream builds

Language packages that vendor or embed this crate can set
`DATALOGIC_C_SKIP_CBINDGEN=1` in their build env to suppress header
regeneration and use the committed `include/datalogic.h`.
