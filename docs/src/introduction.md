<div align="center">
  <img src="https://avatars.githubusercontent.com/u/207296579?s=200&v=4" alt="Plasmatic Logo" width="120" height="120">

# datalogic-rs

**A fast, production-ready Rust engine for JSONLogic.**

[![Crates.io](https://img.shields.io/crates/v/datalogic-rs.svg)](https://crates.io/crates/datalogic-rs)
[![Documentation](https://docs.rs/datalogic-rs/badge.svg)](https://docs.rs/datalogic-rs)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

</div>

---

<div align="center">
  <a href="https://goplasmatic.github.io/datalogic-rs/playground/">
    <img src="assets/demo.gif" alt="JSONLogic Online Debugger Demo" width="700">
  </a>
  <p><em>Try the <a href="https://goplasmatic.github.io/datalogic-rs/playground/">JSONLogic Online Debugger</a> to interactively test your rules</em></p>
</div>

---

**datalogic-rs** is a high-performance Rust implementation of [JSONLogic](http://jsonlogic.com) for evaluating logical rules expressed as JSON. It provides a blazing-fast, memory-efficient, sandbox-safe, and thread-safe way to evaluate complex business rules, feature flags, dynamic pricing logic, and more.

The same engine is compiled and wrapped for multiple runtimes: **Rust, Node.js (native napi), WebAssembly, Python, Go, Java, .NET, and PHP**, and features a companion **React visual debugger / editor**. Author the rule once, evaluate it anywhere with absolute semantic parity.

For the cross-runtime installation instructions and repository details, see the [GitHub repository](https://github.com/GoPlasmatic/datalogic-rs).

> **v5 is here.** v5 is a major release that renames `DataLogic` → `Engine`, makes one-shot evaluation string-based (eliminating the mandatory `serde_json` dependency), switches custom operators to a pre-evaluated arena API, and removes mutable operator registration. v5 is a hard cliff — there are no compatibility shims. See the [Migration Guide](migration.md) for details.

## Why datalogic-rs?

- 🔒 **100% Sandbox-Safe:** Evaluate rules and formulas securely without arbitrary code execution (no scripting engine or `eval()`).
- 🌐 **Single Source of Truth:** Run identical JSON rules across your entire stack (Rust, Go, Python, Node, browser, etc.) with 100% semantic parity.
- ⚡ **Blazing Fast:** Compiles JSON logic into optimized bytecode. Evaluates using O(1) OpCode dispatch and `bumpalo` arena-based allocation for zero-copy variables and minimal heap allocations.
- 🛠️ **Ready-Made Rule Builder:** Drop `@goplasmatic/datalogic-ui` into your React dashboard to let users edit and step-through rules visually.
- 🦀 **Rust-First Core:** Clean, robust Rust API designed to be zero-cost, fully thread-safe (`Logic` is `Send + Sync`), and buildable with `#![forbid(unsafe_code)]`.
- 📦 **Serde-Optional:** Compile without `serde_json` for a minimal dependency tree. Opt-in when you need direct typed JSON serialization/deserialization.
- 🔋 **Battery-Included Operators:** Comes with 50+ built-in operators (59 with all operator features enabled), spanning datetime, arithmetic, regex, and logical categories, and is easily extensible with custom operators.


## How It Works

datalogic-rs uses a two-phase approach:

1. **Compilation**: Your JSON logic is parsed and compiled into a reusable `Logic`. This phase:
   - Assigns OpCodes to built-in operators for fast dispatch
   - Pre-evaluates constant expressions
   - Analyzes structure for templating mode

2. **Evaluation**: The compiled logic is evaluated against your data with:
   - Direct OpCode dispatch (no string lookups at runtime)
   - Arena-allocated `&DataValue<'a>` results that can borrow zero-copy from the input
   - Context stack for nested operations (`map`, `filter`, `reduce`)

## Quick Example: One-Shot Helper

For quick one-off evaluations with zero setup, use the library's module-level helpers:

<div class="codetabs">

```rust
// One-shot evaluation: returns a JSON string.
let result = datalogic_rs::eval_str(
    r#"{">": [{"var": "age"}, 18]}"#,
    r#"{"age": 21}"#,
).unwrap();
assert_eq!(result, "true");
```

```javascript
import init, { evaluate } from '@goplasmatic/datalogic-wasm';
await init();

// One-shot evaluation: returns a JSON string.
const result = evaluate(
  '{">": [{"var": "age"}, 18]}',
  '{"age": 21}',
  false
);
console.log(result); // "true"
```

```python
from datalogic_py import apply

# One-shot evaluation: returns native boolean.
result = apply(
    {">": [{"var": "age"}, 18]},
    {"age": 21}
)
print(result) # True
```

```go
result, _ := datalogic.Apply(
    `{">": [{"var": "age"}, 18]}`,
    `{"age": 21}`,
)
fmt.Println(result) // "true"
```

</div>

## Quick Example: Compiled Rule (Production Loop)

For repeated evaluation of the same logic against thousands of records, compile the rule once. This compiles the JSON Logic to optimized bytecode, enabling sub-microsecond evaluations:

<div class="codetabs">

```rust
use datalogic_rs::Engine;

let engine = Engine::new();
// Compile once (returns Logic bytecode)
let compiled = engine.compile(r#"{">": [{"var": "age"}, 18]}"#).unwrap();

// Create a session to reuse allocation buffers
let mut session = engine.session();

let r1 = session.eval_str(&compiled, r#"{"age": 21}"#).unwrap();
let r2 = session.eval_str(&compiled, r#"{"age": 16}"#).unwrap();
assert_eq!(r1, "true");
assert_eq!(r2, "false");

session.reset(); // Reset between batches
```

```javascript
import init, { CompiledRule } from '@goplasmatic/datalogic-wasm';
await init();

// Compile once
const rule = new CompiledRule('{">": [{"var": "age"}, 18]}', false);

// Evaluate many times
const r1 = rule.evaluate('{"age": 21}');
const r2 = rule.evaluate('{"age": 16}');
console.log(r1, r2); // "true" "false"
```

```python
from datalogic_py import Engine

engine = Engine()
# Compile once
rule = engine.compile({">": [{"var": "age"}, 18]})

# Evaluate many times
r1 = rule.evaluate({"age": 21})
r2 = rule.evaluate({"age": 16})
print(r1, r2) # True False
```

```go
engine := datalogic.NewEngine()
defer engine.Close()

// Compile once
rule, _ := engine.Compile(`{">": [{"var": "age"}, 18]}`)
defer rule.Close()

// Open a session for arena recycling
session := engine.Session()
defer session.Close()

r1, _ := session.Evaluate(rule, `{"age": 21}`)
r2, _ := session.Evaluate(rule, `{"age": 16}`)
fmt.Println(r1, r2) // "true" "false"
```

</div>

## What is JSONLogic?

[JSONLogic](http://jsonlogic.com) is a standard for expressing logic rules as JSON. This makes it:

- **Portable**: Rules can be stored in databases, sent over APIs, or embedded in configuration
- **Language-agnostic**: The same rules work across different implementations
- **Human-readable**: Rules are easier to understand than arbitrary code
- **Safe**: Rules can be evaluated without arbitrary code execution

A JSONLogic rule is a JSON object where:
- The key is the operator name
- The value is an array of arguments

```json
{"operator": [arg1, arg2, ...]}
```

For example:
```json
{"and": [
  {">": [{"var": "age"}, 18]},
  {"==": [{"var": "country"}, "US"]}
]}
```

This rule checks if `age > 18` AND `country == "US"`.

## Next Steps

- [Installation](getting-started/installation.md) - Add datalogic to your project
- [Quick Start](getting-started/quick-start.md) - Get up and running in minutes
- [Migration Guide](migration.md) - Move from v4 to v5
- [Operators](operators/overview.md) - Explore all built-in operators (59 with all operator features enabled)
- [API Reference](api/reference.md) - Public Rust types and the 5-tier API model
- [Starter Boilerplates](getting-started/boilerplates.md) - Deploy microservices in Express, FastAPI, and Axum.
