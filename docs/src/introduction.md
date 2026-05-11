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

**datalogic-rs** is a high-performance Rust implementation of [JSONLogic](http://jsonlogic.com) for evaluating logical rules expressed as JSON. It provides a fast, memory-efficient, and thread-safe way to evaluate complex business rules, feature flags, dynamic pricing logic, and more.

The same engine ships across runtimes: **Rust, JavaScript / TypeScript (WebAssembly), Python, Go, and a React visual debugger**. Author the rule once; evaluate it anywhere. For the cross-runtime overview and per-binding install instructions, see the [repository README](https://github.com/GoPlasmatic/datalogic-rs#readme).

> **v5 is here.** v5 is a breaking release that renames `DataLogic` → `Engine`, makes one-shot evaluation string-based, switches custom operators to a pre-evaluated arena API, and removes the implicit `serde_json` dependency from the default build. v5 is a hard cliff — there is no compatibility shim. See the [Migration Guide](migration.md) for the conceptual overview and the repo-root `MIGRATION.md` for the full v4 → v5 cookbook.

## Why datalogic-rs?

- **Fast** - OpCode-based dispatch with compile-time optimization, plus arena allocation for zero-copy reads
- **Thread-Safe** - Wrap `Logic` in `Arc` and share across threads (or use `Engine::compile_arc` to do it in one step)
- **Zero `unsafe`** - The crate enforces `#![forbid(unsafe_code)]`
- **serde_json-free by default** - The string-based API needs no `serde_json` dependency; opt into the `serde_json` feature when you need `serde_json::Value` interop or the typed `eval_into::<T>` paths
- **Five-tier API ladder** - module-level helpers (`datalogic_rs::eval_str`, …) for one-shot use, `Engine` for configured workloads, `Session` for compile-once / evaluate-many hot loops, raw `evaluate(&Bump)` for zero-copy result pipelines, and `Engine::trace()` for debugging
- **Cross-runtime** - same rules, same semantics across Rust, WASM, Python, Go, and the React debugger
- **Extensible** - Register custom operators on an `EngineBuilder`
- **Feature-Rich** - 59 built-in operators including datetime, regex, and error handling
- **Fully Compliant** - Passes the official JSONLogic test suite

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

## Quick Example

```rust
// One-shot evaluation: returns a JSON string.
let result = datalogic_rs::eval_str(
    r#"{">": [{"var": "age"}, 18]}"#,
    r#"{"age": 21}"#,
).unwrap();
assert_eq!(result, "true");
```

For repeated evaluation, compile once and reuse via a session:

```rust
use datalogic_rs::Engine;

let engine = Engine::new();
let compiled = engine.compile(r#"{">": [{"var": "age"}, 18]}"#).unwrap();
let mut session = engine.session();

let r1 = session.eval_str(&compiled, r#"{"age": 21}"#).unwrap();
let r2 = session.eval_str(&compiled, r#"{"age": 16}"#).unwrap();
assert_eq!(r1, "true");
assert_eq!(r2, "false");
session.reset();
```

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

- [Installation](getting-started/installation.md) - Add datalogic-rs to your project
- [Quick Start](getting-started/quick-start.md) - Get up and running in minutes
- [Migration Guide](migration.md) - Move from v4 to v5
- [Operators](operators/overview.md) - Explore all 59 built-in operators
- [API Reference](api/reference.md) - Public Rust types and the 5-tier API model

**Using another language?** This site focuses on the Rust crate; for JavaScript / TypeScript, Python, Go, and React, jump straight to the per-binding README in the [repo root](https://github.com/GoPlasmatic/datalogic-rs#readme).
