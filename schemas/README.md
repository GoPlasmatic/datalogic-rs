# Wire schemas

JSON Schema (draft 2020-12) for the documents the engine hands to other
programs. The bindings produce them through `crates/datalogic-bind`, whose
tests validate real output against these files, so a format change shows up
as a schema change in review.

| File | Document |
|---|---|
| `trace.v1.json` | The traced-run envelope of `evaluateWithTrace` |
| `error.v1.json` | A serialised engine error |
| `operators.v1.json` | The operator catalogue (`operators()`, `docs/src/operators/operators.json`) |
| `facts.v1.json` | Rule facts (`Rule.facts()`) |
| `diagnostics.v1.json` | Check diagnostics (`check()`, `CompileError`) |

A file is never edited incompatibly: a breaking change ships as a `v2` file
next to `v1`. Adding an optional property is compatible.
