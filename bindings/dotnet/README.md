# Goplasmatic.Datalogic

[![NuGet](https://img.shields.io/nuget/v/Goplasmatic.Datalogic)](https://www.nuget.org/packages/Goplasmatic.Datalogic)
[![CI](https://github.com/GoPlasmatic/datalogic-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/GoPlasmatic/datalogic-rs/actions/workflows/ci.yml)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

Part of [datalogic-rs](https://github.com/GoPlasmatic/datalogic-rs) — one engine, every runtime.

.NET bindings for [datalogic-rs](https://github.com/GoPlasmatic/datalogic-rs),
the JSONLogic rules engine with one Rust core and official bindings for
Rust, Node.js, the browser (WASM), Python, Go, Java, .NET, and PHP. Same
rules, same semantics: every binding runs the same core and passes the
same 1,565-case conformance battery (54 suites). Compile once, evaluate
many, natively in .NET.

For the cross-runtime overview and the API-tier model every binding
implements, see the
[repo README](https://github.com/GoPlasmatic/datalogic-rs#readme).

> **New in v5.** This package is new: there is no v4 .NET artifact. If
> you are coming from the v4 Rust crate or the v4
> `@goplasmatic/datalogic` WASM package, the engine's v4 → v5 changes
> are catalogued in
> [MIGRATION.md](https://github.com/GoPlasmatic/datalogic-rs/blob/main/MIGRATION.md).

## Install

```bash
dotnet add package Goplasmatic.Datalogic
```

The binding is a P/Invoke wrapper over the engine's C ABI, built on
`LibraryImport` source-generated stubs, so the assembly is
NativeAOT-ready out of the box. The NuGet package ships the native
library under `runtimes/<rid>/native/` for every supported platform;
`dotnet publish` picks the right one for the target RID automatically.
No Rust toolchain needed.

| Platform | RIDs                   |
|----------|------------------------|
| Linux    | linux-x64, linux-arm64 |
| macOS    | osx-x64, osx-arm64     |
| Windows  | win-x64, win-arm64     |

Targets `net8.0` or newer.

## Quick start

```csharp
using Goplasmatic.Datalogic;

using var engine = new Engine();
var result = engine.Apply("""{"+":[1,2]}""", "{}");  // "3"
```

Rules, data, and results cross the boundary as JSON strings. The
`ApplyJson` / `EvaluateJson` variants return a parsed
`System.Text.Json.Nodes.JsonNode` instead of a string.

## Compile once, evaluate many

Compile the rule once when you'll evaluate it against many data inputs:

```csharp
using var engine = new Engine();
using var rule = engine.Compile("""{"var":"x"}""");
foreach (var x in new[] { 1, 2, 3 })
{
    Console.WriteLine(rule.Evaluate($"{{\"x\":{x}}}"));
}
```

`Engine` and compiled `Rule` objects are thread-safe: build and compile
once, share them across threads. Sessions (below) are not.

## Sessions (hot loops)

A `Session` reuses one arena across evaluations and resets it at the
start of every call, so peak memory stays bounded:

```csharp
using var session = engine.OpenSession();
foreach (var data in inputs)
{
    var result = session.Evaluate(rule, data);
}
```

Open one session per thread; a `Session` is not thread-safe. Every
public type implements `IDisposable` (with a finalizer as best-effort
fallback), so prefer `using` to release native handles deterministically.

## Parsed data handles

When the same payload feeds many evaluations, parse it once into a
`DataHandle` and skip the per-call JSON parse entirely:

```csharp
using var data = DataHandle.Parse("""{"user":{"age":25,"plan":"pro"}}""");
var a = rule.Evaluate(data);            // thread-safe one-shot
var b = session.Evaluate(rule, data);   // session hot path
```

A `DataHandle` is immutable, thread-safe, and engine-independent — one
handle can feed rules compiled by different engines, and evaluation
never consumes it. Dispose it after the last evaluation that uses it.

## Typed results

For predicate- and scalar-shaped rules, the typed session variants
return the value directly with no JSON serialization on the native
side. They take a `DataHandle` (the flows that want typed results are
exactly the flows that parse data once):

```csharp
bool   ok  = session.EvaluateBool(rule, data);    // strict JSON boolean
long   n   = session.EvaluateInt64(rule, data);   // exact integer
double x   = session.EvaluateDouble(rule, data);  // any JSON number
bool   t   = session.EvaluateTruthy(rule, data);  // JSONLogic truthiness
```

`EvaluateBool` / `EvaluateInt64` / `EvaluateDouble` throw
`EvaluateException` with `Status == EvaluationStatus.TypeMismatch`
(error type `"TypeMismatch"`) when the rule evaluates fine but the
result is not of the requested type. `EvaluateTruthy` never mismatches
— it collapses any result through the engine's configured truthiness
rules (the same coercion `if` / `and` / `or` apply).

## Batch evaluation

Two batch shapes cross the native boundary in a single call:

```csharp
// One rule x N payloads:
EvaluationResult[] perPayload = session.EvaluateBatch(rule, dataHandles);

// N rules x one payload (the rule-set / feature-flag shape):
EvaluationResult[] perRule = session.EvaluateMany(rules, data);
```

Per-item failures don't throw: each `EvaluationResult` carries either
the result (`IsSuccess`, `Json`) or the item's error detail
(`Status`, `ErrorTag`, `ErrorMessage`, `ErrorOperator`). `Value`
returns the JSON or throws the mapped exception for callers that treat
any item failure as exceptional. The batch call itself only throws for
argument-level problems (e.g. a rule compiled by a different engine).

## API surface

The binding mirrors the Rust engine's
[API tier model](https://github.com/GoPlasmatic/datalogic-rs#one-api-shape-every-binding).
Rules and results cross the boundary as JSON strings; data crosses as a
JSON string or a pre-parsed `DataHandle`, and the typed session
variants return .NET scalars directly.

| Tier            | Entry point                                                 | Use when                                              |
|-----------------|-------------------------------------------------------------|-------------------------------------------------------|
| One-shot        | `engine.Apply(rule, data)`                                  | Ad-hoc evaluation, one rule + one data shape          |
| Engine + config | `new Engine(templating)` / `Engine.Builder()…Build()`       | Templating mode, custom operators, evaluation config  |
| Compile once    | `engine.Compile(rule)` → `rule.Evaluate(data)`              | Same rule evaluated against many data inputs          |
| Parse once      | `DataHandle.Parse(json)` → `rule.Evaluate(dataHandle)`      | Same payload evaluated by many rules / many times     |
| Session         | `engine.OpenSession()` → `session.Evaluate(rule, data)`     | Hot loops: amortise arena reset across iterations     |
| Typed           | `session.EvaluateBool/Int64/Double/Truthy(rule, dataHandle)` | Predicates and scalars without JSON round-trips      |
| Batch           | `session.EvaluateBatch(rule, datas)` / `session.EvaluateMany(rules, data)` | Many evaluations per native call        |
| Traced          | `engine.OpenTracedSession()` → `session.Evaluate(rule, data)` | Step-by-step debugging; feeds the React debugger    |

## Custom operators

Register C#-implemented operators through the builder. Each callback
receives the operator's pre-evaluated arguments as a JSON-array string
and returns a JSON-value string; throwing signals an evaluation error
whose message bubbles back to the caller.

```csharp
using var engine = Engine.Builder()
    .AddOperator("double", argsJson =>
    {
        var n = System.Text.Json.Nodes.JsonNode.Parse(argsJson)![0]!.GetValue<double>();
        return (n * 2).ToString();
    })
    .Build();
Console.WriteLine(engine.Apply("""{"double":[21]}""", "{}"));  // "42"
```

**Built-ins win**: a custom registration of a built-in name (`+`, `if`,
`var`, ...) never dispatches at evaluation time; the built-in always
runs.

## Engine configuration

`Engine.Builder().SetConfigJson(json)` sets the evaluation semantics
from a JSON object string: an optional `preset` plus per-field
overrides. Unknown keys or values throw `EvaluateException` (error type
`ConfigurationError`), so typos fail loudly:

```csharp
using var lenient = Engine.Builder()
    .SetConfigJson("""{"division_by_zero":"return_null"}""")
    .Build();
lenient.Apply("""{"/":[1.5,0]}""", "{}");  // "null"

using var strict = Engine.Builder()
    .SetConfigJson("""{"preset":"strict"}""")
    .Build();
strict.Apply("""{"+":["",1]}""", "{}");    // throws: strict rejects non-numeric coercion
```

| Key | Values |
|-----|--------|
| `preset` | `"default"`, `"safe_arithmetic"`, `"strict"` |
| `arithmetic_nan_handling` | `"throw_error"`, `"ignore_value"`, `"coerce_to_zero"`, `"return_null"` |
| `division_by_zero` | `"return_saturated"`, `"throw_error"`, `"return_null"`, `"return_infinity"` |
| `loose_equality_errors` | `bool` |
| `truthy_evaluator` | `"javascript"`, `"python"`, `"strict_boolean"` |
| `numeric_coercion` | object of bools: `empty_string_to_zero`, `null_to_zero`, `bool_to_number`, `reject_non_numeric` |
| `max_recursion_depth` | integer >= 1 |

The `preset` applies first; the remaining keys override individual
fields on top of it. Every binding shares this JSON schema and parses it
with the same core code, so a config that works here works in the
Python, Node, and WASM bindings too. The full semantics of each knob are
documented on the Rust crate's
[`EvaluationConfig`](https://docs.rs/datalogic-rs/latest/datalogic_rs/struct.EvaluationConfig.html).

## Error handling

Everything the binding throws extends `DatalogicException`:

| Exception           | When                                                      |
|---------------------|-----------------------------------------------------------|
| `ParseException`    | Malformed rule or data JSON, or an unsupported operator   |
| `EvaluateException` | Operator failure at runtime, or a rejected engine config  |

The structured fields ride on the base class: `ErrorType` is the stable
engine tag (e.g. `"ParseError"`, `"Thrown"`, `"TypeMismatch"`,
`"InvalidArgument"`), `Operator` the outermost failing operator (e.g.
`"+"`), `PathJson` the root-to-leaf error path as a JSON array (each
`null` when not applicable), and `Status` the coarse
`EvaluationStatus` the native call returned (`ParseError`,
`EvaluationError`, `TypeMismatch`, `InvalidArgument`, `InternalError`).

```csharp
using var engine = new Engine();
try
{
    // arithmetic on a non-numeric string throws {"type":"NaN"}
    engine.Apply("""{"+":[{"var":"x"},1]}""", """{"x":"abc"}""");
}
catch (EvaluateException e)
{
    Console.WriteLine(e.Status);     // EvaluationError
    Console.WriteLine(e.ErrorType);  // "Thrown"
    Console.WriteLine(e.Operator);   // "+"
    Console.WriteLine(e.PathJson);   // JSON-array path through the compiled tree
}
```

## Threading

| Type         | Pattern                                  |
|--------------|-------------------------------------------|
| `Engine`     | Build once; share across threads          |
| `Rule`       | Compile once; share across threads        |
| `DataHandle` | Parse once; immutable, share across threads (and engines) |
| `Session`    | One per worker thread; never share        |

`TracedSession` is thread-safe as well. Rules passed to a session must
come from the engine that opened it (a foreign rule throws
`EvaluateException` with `Status == EvaluationStatus.InvalidArgument`).

## Tracing

```csharp
using var session = engine.OpenTracedSession();
var run = session.Evaluate("""{"+":[{"var":"x"},1]}""", """{"x":41}""");
Console.WriteLine(run.Result);          // 42
Console.WriteLine(run.Steps.Count);     // number of executed nodes
```

Same trace envelope as every other binding; the
[React debugger](https://github.com/GoPlasmatic/datalogic-rs/tree/main/ui)
consumes it directly. `TracedRun` exposes `Result`, `ExpressionTree`,
`Steps`, `Error`, and `StructuredError` (plus `IsSuccess`); runtime
failures surface inside the run rather than as exceptions. Tracing
disables the optimizer so every operator appears in the trace: use it
for debugging, not hot paths.

## Performance

<!-- canonical-bench v5.0 -->
Geomean across 50 operator benchmark suites (Apple M2 Pro, median of 3 runs; pairwise shared-suite ratios per the [methodology](https://github.com/GoPlasmatic/datalogic-rs/blob/main/tools/benchmark/BENCHMARK.md)): the native Rust core evaluates at **8.9 ns/op**, 7.9× faster than json-logic-engine (compiled, the fastest JS engine), 30.6× faster than jsonlogic-rs (the closest Rust alternative), and 104.2× faster than the json-logic-js reference implementation. The WASM build under Node measures 901.1 ns geomean (101× native); on Node servers, prefer `@goplasmatic/datalogic-node`.

The P/Invoke boundary adds a small per-call marshalling cost on top of
the core numbers.

## Building from source

The binding lives in
[`bindings/dotnet/`](https://github.com/GoPlasmatic/datalogic-rs/tree/main/bindings/dotnet).
At runtime the native library resolves in order: the
`DATALOGIC_NATIVE_LIB` env var (absolute path), NuGet's
`runtimes/<rid>/native/` layout, then the in-tree C ABI target dir. On
first use the binding asserts the resolved library speaks C ABI v2
(`datalogic_abi_version() == 2`) and fails loudly with a rebuild hint
if a stale library is picked up. So a fresh clone needs the C ABI built
once:

```bash
git clone https://github.com/GoPlasmatic/datalogic-rs
cd datalogic-rs/bindings/c && cargo build --release
cd ../dotnet
dotnet build
dotnet test
```

## Learn more

- [datalogic-rs repository](https://github.com/GoPlasmatic/datalogic-rs#readme)
- [Rust crate deep-dive](https://github.com/GoPlasmatic/datalogic-rs/tree/main/crates/datalogic-rs#readme)
- [.NET docs chapter](https://goplasmatic.github.io/datalogic-rs/dotnet.html)
- [Online playground](https://goplasmatic.github.io/datalogic-rs/playground/)
- [JSONLogic specification](https://jsonlogic.com)
- [C ABI internals](https://github.com/GoPlasmatic/datalogic-rs/tree/main/bindings/c#readme)

## License

Apache-2.0. See the
[main repository](https://github.com/GoPlasmatic/datalogic-rs) for
source and contribution guidelines.
