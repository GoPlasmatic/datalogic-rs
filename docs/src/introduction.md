# Introduction

**datalogic-rs** is a JSONLogic rules engine: one Rust core with official bindings for Rust, Node.js, the browser (WASM), Python, Go, Java, .NET, and PHP, plus a React visual debugger. Rules are plain JSON; the same rule evaluates with identical semantics in every runtime, verified by a 1,953-case conformance battery that runs against the same core every binding ships.

This site is the reference documentation. For the project pitch, benchmarks, and package matrix, see the [GitHub repository](https://github.com/GoPlasmatic/datalogic-rs#readme); to try rules in your browser, open the [playground](https://goplasmatic.github.io/datalogic-rs/playground/).

## What is JSONLogic?

[JSONLogic](http://jsonlogic.com) is a standard for expressing logic rules as JSON. This makes it:

- **Portable**: You can store rules in databases, send them over APIs, or embed them in configuration
- **Language-agnostic**: The same rules work across different implementations
- **Human-readable**: Rules are easier to understand than arbitrary code
- **Safe**: Evaluating a rule executes no arbitrary code

A JSONLogic rule is a JSON object where the key is the operator name and the value is an array of arguments:

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

## How the engine works

datalogic-rs uses a two-phase approach:

1. **Compilation**: The engine parses your JSON logic and compiles it into a reusable `Logic`. This phase:
   - Assigns OpCodes to built-in operators for fast dispatch
   - Pre-evaluates constant expressions
   - Analyzes structure for templating mode

2. **Evaluation**: The engine evaluates the compiled logic against your data with:
   - Direct OpCode dispatch (no string lookups at runtime)
   - Arena-allocated results that can borrow zero-copy from the input
   - A context stack for nested operations (`map`, `filter`, `reduce`)

Every binding exposes this compile-once, evaluate-many pattern, and it keeps evaluation in the nanosecond range.

## Find your language

Every language has its own chapter with install, quickstart, and the API surface:

| Your stack | Start here |
| :--- | :--- |
| Rust | [Rust (native crate)](rust/overview.md) |
| Node.js services | [Node.js (native)](nodejs/overview.md) |
| Browser, edge, Deno, Bun | [JavaScript (WASM)](javascript/installation.md) |
| Python | [Python](python/installation.md) |
| Go | [Go](go/installation.md) |
| Java, Kotlin, Scala | [Java / Kotlin (JVM)](jvm.md) |
| .NET (C#, F#) | [.NET](dotnet.md) |
| PHP | [PHP](php.md) |
| Another language entirely | [C ABI](c-abi.md) |
| React rule-builder UI | [React Visual Debugger](react-ui/installation.md) |

## How these docs are organized

- **[Getting Started](getting-started/installation.md)**: install, first evaluation, core concepts, starter microservice templates
- **[Operators](operators/overview.md)**: reference for all 84 built-in operators, with runnable examples on every page
- **Languages**: one chapter per binding (see the table above)
- **Guides**: [custom operators](advanced/custom-operators.md), [configuration](advanced/configuration.md), [structured objects / templating](advanced/structured-objects.md), [thread safety](advanced/threading.md), and [security & sandboxing](advanced/security.md)
- **Reference**: [use-case cookbook](use-cases/examples.md), [performance](performance.md), [comparisons](comparison.md), [migration](migration.md), [FAQ](faq.md), and [troubleshooting](troubleshooting.md)

## Next steps

- [Installation](getting-started/installation.md): add datalogic to your project
- [Quick Start](getting-started/quick-start.md): your first evaluation
- [Use Cases & Examples](use-cases/examples.md): feature flags, pricing, validation, fraud scoring
- [Coming from json-logic-js?](coming-from-json-logic-js.md): your rules run unchanged
- [Migrating from v4](migration.md)
