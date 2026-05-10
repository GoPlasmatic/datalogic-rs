# Examples

Runnable demos for the Rust crate. Each one opens with a doc comment
stating its goal; this README is the index. Examples that depend on
opt-in features must be invoked with the matching `--features` flag (the
`required-features` block in `packages/core/Cargo.toml` enforces this).

| Example                    | What it shows                                                                    | Required features    |
|----------------------------|----------------------------------------------------------------------------------|----------------------|
| `getting_started`          | The three pillars in one file — start here                                       | `templating`         |
| `compile_once_evaluate_many` | Throughput patterns: shared `Logic` + reusable `Session`                       | _(none)_             |
| `configuration`            | `EvaluationConfig` presets and per-field knobs                                   | _(none)_             |
| `custom_operator`          | Implementing `CustomOperator` and registering it on the builder                  | _(none)_             |
| `structured_objects`       | Templating mode for response shaping                                             | `templating`         |
| `thread_safety`            | Sharing a compiled `Logic` across threads via `Arc`                              | _(none)_             |
| `datetime_ops`             | Parse, format, compare, and do arithmetic on dates                               | `datetime`           |
| `tracing`                  | Recording every evaluation step for debugging                                    | `trace`              |
| `error_handling`           | `try` / `throw`, structured `Error` shape                                        | `error-handling`     |
| `zero_copy_input`          | The `EvalInput` shapes side by side, with per-call cost commentary               | `serde_json`         |

## Running

```bash
# A no-feature example
cargo run -p datalogic-rs --example custom_operator

# A feature-gated one
cargo run -p datalogic-rs --example getting_started --features templating
cargo run -p datalogic-rs --example tracing         --features trace
cargo run -p datalogic-rs --example datetime_ops    --features datetime
```

To run *all* examples (useful before publishing), build with every feature:

```bash
cargo build -p datalogic-rs --examples --all-features
```

If you're unsure where to start, open `getting_started.rs` first — it
walks through `Engine::new`, `eval_str`, and `Session` in roughly
sixty lines.
