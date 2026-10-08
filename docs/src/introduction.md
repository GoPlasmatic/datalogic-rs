# Introduction

**datalogic-rs** is a JSONLogic rules engine: one Rust core with official bindings for Rust, Node.js, the browser (WASM), Python, Go, Java, .NET, and PHP, plus a React visual debugger. Rules are plain JSON; the same rule evaluates with identical semantics in every runtime, verified by a 2,128-case conformance battery that runs against the same core every binding ships.

This site is the reference documentation. For the project pitch, benchmarks, and package matrix, see the [GitHub repository](https://github.com/GoPlasmatic/datalogic-rs#readme); to try rules in your browser, open the [playground](https://goplasmatic.github.io/datalogic-rs/playground/).

## What is JSONLogic?

[JSONLogic](http://jsonlogic.com) is a standard for expressing logic rules as JSON. A rule is:

- **Portable**: you store it in a database, send it over an API, or embed it in configuration
- **Language-agnostic**: the same rule runs on any conforming implementation
- **Readable**: a reviewer reads the rule itself, with no host code around it
- **Safe**: evaluating a rule runs no code the rule supplies

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

This rule checks that `age > 18` and `country == "US"`.

## How the engine works

datalogic-rs works in two phases:

1. **Compilation**: the engine parses the rule and compiles it into a reusable `Logic`. This phase:
   - assigns each built-in operator an OpCode, and each custom operator a slot on the engine
   - evaluates constant subexpressions once
   - builds output templates in templating mode

2. **Evaluation**: the engine runs the compiled logic against your data with:
   - OpCode dispatch, with no operator-name lookup at runtime
   - arena-allocated results that can borrow from the input without copying
   - a context stack for iterator bodies (`map`, `filter`, `reduce`)

Every binding exposes this compile-once, evaluate-many pattern; [Performance](performance.md) has the numbers.

Before a rule runs, you can check it: `Engine::check` (`check` in every binding) reports unknown operators with a "did you mean" suggestion, wrong argument counts and other mistakes, each with a JSON Pointer into the rule ([Rule Analysis](advanced/rule-analysis.md#checking-a-rule-enginecheck)). The `MissingVar::Error` setting turns a read of a misspelled data path into a `VariableNotFound` error instead of `null` ([Configuration](advanced/configuration.md#missing-variables)), and `Engine::operators()` lists every operator your build has ([API Reference](rust/api-reference.md)).

## Find your language

Each language has its own chapter with install steps, a quick start and the API surface:

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
| Any other language | [C ABI](c-abi.md) |
| React rule-builder UI | [React Visual Debugger](react-ui/installation.md) |

## How these docs are organized

- **[Getting Started](getting-started/installation.md)**: install, first evaluation, core concepts, starter service code
- **[Operators](operators/overview.md)**: reference for the 84 built-in operators, with runnable examples on every page
- **Languages**: one chapter per binding (see the table above)
- **Guides**: [custom operators](advanced/custom-operators.md), [configuration](advanced/configuration.md), [rule analysis](advanced/rule-analysis.md), [operation budget](advanced/operation-budget.md), [structured objects / templating](advanced/structured-objects.md), [thread safety](advanced/threading.md), and [security & sandboxing](advanced/security.md)
- **Reference**: [use-case cookbook](use-cases/examples.md), [performance](performance.md), [comparisons](comparison.md), [migration](migration.md), [FAQ](faq.md), and [troubleshooting](troubleshooting.md)

## Next steps

- [Installation](getting-started/installation.md): add datalogic to your project
- [Quick Start](getting-started/quick-start.md): your first evaluation
- [Use Cases & Examples](use-cases/examples.md): feature flags, pricing, validation, fraud scoring
- [Coming from json-logic-js?](coming-from-json-logic-js.md): your rules run unchanged
- [Migration Guide](migration.md): from v4, and the behaviour changes in 5.8
