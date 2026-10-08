# Examples

Runnable demos for the Rust crate. Each one opens with a doc comment
stating its goal; this README is the index. To run an example that
depends on opt-in features, pass the matching `--features` flag (the
`[[example]] required-features` entries in `crates/datalogic-rs/Cargo.toml`
enforce this).

| Example                      | What it shows                                                                    | Required features    |
|------------------------------|----------------------------------------------------------------------------------|----------------------|
| `getting_started`            | The three pillars in one file (business rules, templates, expressions); start here | `templating`       |
| `compile_once_evaluate_many` | Throughput patterns: shared `Logic` + reusable `Session`                         | none                 |
| `configuration`              | `EvaluationConfig` presets and per-field knobs                                   | none                 |
| `custom_operator`            | Implementing `CustomOperator` and registering it on the builder                  | none                 |
| `structured_objects`         | Templating mode for response shaping                                             | `templating`         |
| `thread_safety`              | Sharing a compiled `Logic` across threads via `Arc`                              | none                 |
| `datetime_ops`               | Parse, format, compare, and do arithmetic on dates                               | `datetime`           |
| `tracing`                    | Recording every evaluation step for debugging                                    | `trace`              |
| `error_handling`             | `try` / `throw`, structured `Error` shape                                        | `error-handling`     |
| `zero_copy_input`            | The `EvalInput` shapes side by side, with per-call cost commentary               | `serde_json`         |

## Running

```bash
# Examples with no required features
cargo run -p datalogic-rs --example compile_once_evaluate_many
cargo run -p datalogic-rs --example configuration
cargo run -p datalogic-rs --example custom_operator
cargo run -p datalogic-rs --example thread_safety

# Feature-gated ones
cargo run -p datalogic-rs --example getting_started    --features templating
cargo run -p datalogic-rs --example structured_objects --features templating
cargo run -p datalogic-rs --example datetime_ops       --features datetime
cargo run -p datalogic-rs --example tracing            --features trace
cargo run -p datalogic-rs --example error_handling     --features error-handling
cargo run -p datalogic-rs --example zero_copy_input    --features serde_json
```

To build every example (useful before publishing), enable every feature:

```bash
cargo build -p datalogic-rs --examples --all-features
```

If you're unsure where to start, open `getting_started.rs` first. It
walks through `Engine::new`, `eval_str` and templating mode in under a
hundred lines.
