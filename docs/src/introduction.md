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

## Why datalogic-rs?

- **Fast**: Uses OpCode-based dispatch with compile-time optimization for maximum performance
- **Thread-Safe**: Compile once, evaluate anywhere with zero-copy `Arc` sharing
- **Intuitive**: Works seamlessly with `serde_json::Value`
- **Extensible**: Add custom operators with a simple trait
- **Feature-Rich**: 59 built-in operators including datetime, regex, and error handling
- **Fully Compliant**: Passes the official JSONLogic test suite

## How It Works

datalogic-rs uses a two-phase approach:

1. **Compilation**: Your JSON logic is parsed and compiled into an optimized `CompiledLogic` structure. This phase:
   - Assigns OpCodes to built-in operators for fast dispatch
   - Pre-evaluates constant expressions
   - Analyzes structure for templating mode

2. **Evaluation**: The compiled logic is evaluated against your data with:
   - Direct OpCode dispatch (no string lookups at runtime)
   - Context stack for nested operations (map, filter, reduce)
   - Efficient value passing with minimal allocations

## Quick Example

```rust
use datalogic_rs::DataLogic;
use serde_json::json;

let engine = DataLogic::new();

// Define a rule: is the user's age greater than 18?
let rule = json!({ ">": [{ "var": "age" }, 18] });

// Compile once
let compiled = engine.compile(&rule).unwrap();

// Evaluate against different data
let result = engine.evaluate_owned(&compiled, json!({ "age": 21 })).unwrap();
assert_eq!(result, json!(true));

let result = engine.evaluate_owned(&compiled, json!({ "age": 16 })).unwrap();
assert_eq!(result, json!(false));
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
- [Operators](operators/overview.md) - Explore all 59 built-in operators
