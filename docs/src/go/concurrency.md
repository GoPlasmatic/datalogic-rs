# Concurrency & Sessions

Go's goroutines make concurrency central. `datalogic-go` maps directly to Rust's thread-safety properties.

## Concurrency Model

*   **`Engine`**: Thread-safe (`Send + Sync` in Rust). Construct a single `Engine` and share it across goroutines safely.
*   **`Rule`**: Thread-safe. Compile a rule once, and call `rule.Evaluate()` from multiple goroutines concurrently.
*   **`Session`**: **Not thread-safe**. Sessions manage a reusable memory arena for evaluation buffers. Share them only within a single goroutine or task, never concurrently.
*   **`DataHandle`**: Thread-safe and engine-independent. Parse a payload once with `datalogic.ParseData()`, share it across goroutines (evaluation only reads it), and `Close()` it after the last use.
*   **`TracedSession`**: Thread-safe. Every `Evaluate()` uses a fresh internal arena, so one traced session can serve many goroutines.

## API Tiers

| Tier | Entry point | Use when |
|---|---|---|
| Compile once | `engine.Compile(rule)` then `rule.Evaluate(data)` / `rule.EvaluateData(handle)` | Same rule evaluated against many data inputs |
| Session | `engine.Session()` then `session.Evaluate(rule, data)` / `session.EvaluateData(rule, handle)` | Hot loops: arena reuse per goroutine |
| Data handle | `datalogic.ParseData(json)` | Same payload evaluated many times: parse once, zero parse work per call |
| Typed | `session.EvaluateBool/Int64/Float64/Truthy(rule, handle)` | Predicates and scalar results, no JSON decode on the way out (`TypeMismatch` error on the wrong type; `EvaluateTruthy` never mismatches) |
| Batch | `session.EvaluateBatch(rule, handles)` / `session.EvaluateMany(rules, handle)` | Many evaluations per native call, with per-item errors in each `BatchResult.Err` |
| Traced | `engine.TracedSession()` then `ts.Evaluate(ruleJSON, dataJSON)` | Step-level execution traces for debuggers and tooling |

Custom operators are registered on `datalogic.NewEngineBuilder().AddOperator(name, fn)` with an `OperatorFunc` (`func(argsJSON string) (string, error)`), and `datalogic.Version()` reports the linked engine version. Examples for every tier are in the [Go README](https://github.com/GoPlasmatic/datalogic-rs/tree/main/bindings/go#data-handles-typed-results-and-batch-evaluation).

## Reusing Arenas with `Session`

To avoid heap allocations in hot paths, create a `Session` per goroutine and defer its `Close()` call.

```go
package main

import (
    "fmt"
    datalogic "github.com/GoPlasmatic/datalogic-rs/bindings/go/v5"
)

func main() {
    engine := datalogic.NewEngine()
    defer engine.Close()

    rule, _ := engine.Compile(`{"var": "user.name"}`)
    defer rule.Close()

    // 1. Create a session (owns a reusable memory arena)
    session := engine.Session()
    defer session.Close()

    users := []string{
        `{"user": {"name": "Alice"}}`,
        `{"user": {"name": "Bob"}}`,
        `{"user": {"name": "Charlie"}}`,
    }

    for _, user := range users {
        // Reuses the session's internal arena allocation
        result, _ := session.Evaluate(rule, user)
        fmt.Println(result)
    }
}
```

## Error Handling

Errors are returned as `*datalogic.Error` structs, which carry detailed debugging metadata:
*   `Type`: The error class name (e.g. `ParseError`, `Thrown`, `TypeError`).
*   `Operator`: The outermost operator where the execution failed.
*   `PathJSON`: A JSON-array string describing the path from the rule root to the failing node, where elements carry fields like `operator` and `json_pointer`.

```go
_, err := rule.Evaluate(`{}`)
if err != nil {
    dErr, ok := err.(*datalogic.Error)
    if ok {
        fmt.Printf("Type: %s\n", dErr.Type)
        fmt.Printf("Operator: %s\n", dErr.Operator)
        fmt.Printf("AST Path: %s\n", dErr.PathJSON)
    }
}
```
