# Concurrency & Sessions

`datalogic-go` exposes the Rust core's thread-safety rules as goroutine-safety rules.

## Concurrency Model

*   **`Engine`**: Goroutine-safe (`Send + Sync` in Rust). Construct a single `Engine` and share it across goroutines.
*   **`Rule`**: Goroutine-safe. Compile a rule once, and call `rule.Evaluate()` from multiple goroutines concurrently.
*   **`Session`**: **Not goroutine-safe**. A session owns a reusable memory arena for evaluation buffers. Use each one from a single goroutine at a time.
*   **`DataHandle`**: Goroutine-safe and engine-independent. Parse a payload once with `datalogic.ParseData()`, share it across goroutines (evaluation only reads it), and `Close()` it after the last use.
*   **`TracedSession`**: Goroutine-safe. Every `Evaluate()` uses a fresh internal arena, so one traced session can serve many goroutines.
*   **`EngineBuilder`**: Not goroutine-safe. Build on one goroutine, then share the resulting `Engine`.

`Close()` on any handle is safe to call more than once and from several goroutines at once: exactly one call frees the native handle. Closing a handle while another goroutine is still using it is not supported. Rules, sessions and traced sessions keep working after their `Engine` is closed, custom operators included: the engine's callbacks stay registered until the last handle derived from it is unreachable.

## API Tiers

| Tier | Entry point | Use when |
|---|---|---|
| Compile once | `engine.Compile(rule)` then `rule.Evaluate(data)` / `rule.EvaluateData(handle)` | Same rule evaluated against many data inputs |
| Compile mode | `engine.CompileTemplate(rule)` / `engine.CompileStrict(rule)` | One rule in a templating mode other than the engine's |
| Checked | `engine.Check(rule, mode)` / `engine.CompileChecked(rule)` | Every problem in a rule before it runs, with JSON Pointers ([Rule Analysis](../advanced/rule-analysis.md)) |
| Introspection | `rule.Facts()` / `engine.Operators()` / `engine.Truthy(value)` | What a rule reads and calls, the operator catalogue, the engine's truthiness |
| Session | `engine.Session()` then `session.Evaluate(rule, data)` / `session.EvaluateData(rule, handle)` | Hot loops: arena reuse per goroutine |
| Metered | `session.EvaluateMetered(rule, data, budget)` | The operations an evaluation charges, optionally capped (`budget` 0 is the engine's own) |
| Data handle | `datalogic.ParseData(json)` | Same payload evaluated many times: parse once, zero parse work per call |
| Typed | `session.EvaluateBool/Int64/Float64/Truthy(rule, handle)` | Predicates and scalar results, no JSON decode on the way out (`TypeMismatch` error on the wrong type; `EvaluateTruthy` never mismatches) |
| Batch | `session.EvaluateBatch(rule, handles)` / `session.EvaluateMany(rules, handle)` | Many evaluations per native call, with per-item errors in each `BatchResult.Err` |
| Traced | `engine.TracedSession()` then `ts.Evaluate(ruleJSON, dataJSON)` / `ts.EvaluateMode(ruleJSON, dataJSON, mode)` | Step-level execution traces for debuggers and tooling |

Register custom operators with `datalogic.NewEngineBuilder().AddOperator(name, fn)` and an `OperatorFunc` (`func(argsJSON string) (string, error)`). The engine may call an operator from any goroutine that evaluates, so synchronise any state the callback shares. `datalogic.Version()` reports the linked engine version. Examples for every tier are in the [Go README](https://github.com/GoPlasmatic/datalogic-rs/tree/main/bindings/go#data-handles-typed-results-and-batch-evaluation).

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

A session evaluates only rules compiled by its own engine; a rule from another engine fails with `Type == "InvalidArgument"`.

## Error Handling

Failed calls return a `*datalogic.Error`. Its `Type` is the engine's error tag, `Operator` the innermost operator that failed, and `PathJSON` the path from the rule root to the failing node. [Configuration & Errors](configuration-and-errors.md#error-handling) lists every field and tag.

```go
_, err := rule.Evaluate(`{}`)
if err != nil {
    dErr, ok := err.(*datalogic.Error)
    if ok {
        fmt.Printf("Type: %s\n", dErr.Type)
        fmt.Printf("Operator: %s\n", dErr.Operator)
        fmt.Printf("Path: %s\n", dErr.PathJSON)
    }
}
```
