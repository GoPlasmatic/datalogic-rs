# Quick Start

Evaluate rules in Go with the `datalogic-go` package.

## One-Shot Evaluation

For quick calculations, use the package-level `Apply` function:

```go
package main

import (
    "fmt"
    datalogic "github.com/GoPlasmatic/datalogic-rs/bindings/go/v5"
)

func main() {
    // Apply takes (ruleJSON, dataJSON) strings
    result, err := datalogic.Apply(`{"+": [1, 2, 3]}`, `{}`)
    if err != nil {
        panic(err)
    }
    fmt.Println(result) // "6"
}
```

## Reusable Compiled Rules

For performance-critical code paths, compile the rule once. The engine parses the rule a single time into a compiled node tree, so repeated evaluations skip re-parsing.

Defer `.Close()` on engines and rules: a GC finalizer frees a handle you forget, but only when the collector runs, and the collector cannot see the native memory behind the handle.

```go
package main

import (
    "fmt"
    datalogic "github.com/GoPlasmatic/datalogic-rs/bindings/go/v5"
)

func main() {
    // 1. Create an engine
    engine := datalogic.NewEngine()
    defer engine.Close() // Releases engine configuration memory

    // 2. Compile once
    rule, err := engine.Compile(`{"if": [{">": [{"var": "score"}, 50]}, "pass", "fail"]}`)
    if err != nil {
        panic(err)
    }
    defer rule.Close() // Releases compiled rule memory

    // 3. Evaluate many times
    result1, _ := rule.Evaluate(`{"score": 75}`)
    result2, _ := rule.Evaluate(`{"score": 30}`)

    fmt.Println(result1) // "pass"
    fmt.Println(result2) // "fail"
}
```

## Catching Rule Mistakes Before They Run

`Compile` accepts a rule that calls an operator the engine doesn't have; the call fails when it runs. `CompileChecked` refuses such a rule up front with a `*datalogic.Error` of `Type == "CompileError"`, whose `DiagnosticsJSON` lists every problem with a JSON Pointer into the rule. `engine.Check(ruleJSON, datalogic.ModeEngine)` returns the same list without compiling:

```go
_, err := engine.CompileChecked(`{"if": [true, {"vr": "x"}, "no"]}`)
var dlErr *datalogic.Error
if errors.As(err, &dlErr) && dlErr.Type == "CompileError" {
    fmt.Println(dlErr.DiagnosticsJSON)
    // [{"code":"UnknownOperator","severity":"error","message":"unknown operator `vr`; did you mean `var`?","pointer":"/if/1","operator":"vr"}]
}
```

Next: [Configuration & Errors](configuration-and-errors.md) covers engine configuration via the builder and the `*datalogic.Error` type.
