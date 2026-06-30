# Quick Start

Evaluate rules instantly in Go using the `datalogic-go` package.

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

For performance-critical code paths, compile the rule once. This stores the parsed rule in memory as optimized Rust bytecodes.

> **Important:** Always defer `.Close()` on engines and rules to prevent C FFI memory leaks!

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
