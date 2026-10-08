# Wire schemas

JSON Schema (draft 2020-12) for the documents the engine hands to other
programs. The bindings produce them through `crates/datalogic-bind`, whose
tests validate real output against these files and reject a property a
schema does not list, so a format change shows up as a schema change in
review. A second test (`crates/datalogic-bind/tests/typings.rs`) checks
the Node binding's TypeScript return types and the Python stub
(`bindings/python/datalogic_py.pyi`) against the same files.

| File | Document |
|---|---|
| `trace.v1.json` | The traced-run envelope of `evaluateWithTrace`, including `pointers` |
| `error.v1.json` | A serialised engine error (a traced run's `structured_error`, and the error the bindings surface) |
| `operators.v1.json` | The operator catalogue (`Engine.operators()`, `docs/src/operators/operators.json`) |
| `facts.v1.json` | Rule facts (`Rule.facts()`) |
| `diagnostics.v1.json` | Check diagnostics (`Engine.check()`, the diagnostics a `CompileError` carries) |

A file is never edited incompatibly: a breaking change ships as a `v2` file
next to `v1`. Adding an optional property is compatible.
