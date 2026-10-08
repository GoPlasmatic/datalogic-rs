# Goplasmatic.Datalogic

[![NuGet](https://img.shields.io/nuget/v/Goplasmatic.Datalogic)](https://www.nuget.org/packages/Goplasmatic.Datalogic)
[![CI](https://github.com/GoPlasmatic/datalogic-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/GoPlasmatic/datalogic-rs/actions/workflows/ci.yml)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

Part of [datalogic-rs](https://github.com/GoPlasmatic/datalogic-rs): one engine, every runtime.

.NET bindings for [datalogic-rs](https://github.com/GoPlasmatic/datalogic-rs),
the JSONLogic rules engine with one Rust core and official bindings for
Rust, Node.js, the browser (WASM), Python, Go, Java, .NET, and PHP.
Compile a rule once and evaluate it many times, natively in .NET. Same
rules, same semantics: every binding runs the same core and passes the
same 2,128-case conformance battery (66 suites).

For the cross-runtime overview and the API-tier model every binding
implements, see the
[repo README](https://github.com/GoPlasmatic/datalogic-rs#readme).

> **New in v5.** This package is new: there is no v4 .NET artifact. If
> you are coming from the v4 Rust crate or the v4
> `@goplasmatic/datalogic` WASM package, see
> [MIGRATION.md](https://github.com/GoPlasmatic/datalogic-rs/blob/main/MIGRATION.md)
> for the engine's v4 → v5 changes.

## Install

```bash
dotnet add package Goplasmatic.Datalogic
```

or in the project file:

```xml
<PackageReference Include="Goplasmatic.Datalogic" Version="5.8.1" />
```

The binding is a P/Invoke wrapper over the engine's C ABI, built on
`LibraryImport` source-generated stubs, so the assembly is
NativeAOT-ready. The NuGet package ships the native library under
`runtimes/<rid>/native/` for every supported platform, and
`dotnet publish` picks the one for the target RID. You need no Rust
toolchain.

| Platform | RIDs                   |
|----------|------------------------|
| Linux    | linux-x64, linux-arm64 |
| macOS    | osx-x64, osx-arm64     |
| Windows  | win-x64, win-arm64     |

The package ships `net8.0` and `net10.0` builds: .NET 8 and 9 apps use
`net8.0`, .NET 10 and newer use `net10.0`. The `net10.0` build uses the
in-box `System.Text.Json`; the `net8.0` build depends on the
`System.Text.Json` package.

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

### Compile modes and checked compiles

`Compile` reads a rule in the engine's own mode. `CompileTemplate` and
`CompileStrict` choose templating for one compile, so one engine (with
one set of custom operators) can compile both conditions and output
templates. `CompileMode(rule, mode)` takes the mode as a `CompileMode`
(`Engine`, `Strict`, `Template`).

```csharp
using var shape = engine.CompileTemplate("""{"user": {"var": "name"}, "n": 1}""");
shape.Evaluate("""{"name": "ana"}""");  // {"user":"ana","n":1}
```

`Check(rule, mode = CompileMode.Engine)` reports every problem the
engine can see before the rule runs, as a JSON array of `{code,
severity, message, pointer, operator}`, where `pointer` is an RFC 6901
JSON Pointer into the rule. Errors are what will fail (an unknown
operator, with a suggestion one edit away; an argument count the
operator rejects; a timezone that does not exist); warnings run but are
probably mistakes. A bad rule does not make `Check` throw.
`CompileChecked(rule)` compiles only a rule with no error and otherwise
throws `ParseException` with `ErrorType == "CompileError"`, whose
`DiagnosticsJson` holds the same array:

```csharp
engine.Check("""{"if": [{">": [{"var": "age"}, 17]}, "adult", {"vr": "x"}]}""");
// [{"code":"UnknownOperator","severity":"error","message":"unknown operator `vr`; did you mean `var`?",
//   "pointer":"/if/2","operator":"vr"}]

try
{
    using var rule = engine.CompileChecked(ruleJson);
    // use the rule
}
catch (ParseException e) when (e.ErrorType == "CompileError")
{
    Console.WriteLine(e.DiagnosticsJson);  // every diagnostic, errors and warnings
}
```

### What a rule reads

`rule.Facts()` returns, as JSON, the data paths the rule reads (each as
its segments), the operators it calls, and whether its result depends
only on its data:

```csharp
using var rule = engine.Compile(
    """{"and": [{">=": [{"var": "user.age"}, 18]}, {"in": [{"var": "user.plan"}, ["pro", "team"]]}]}""");
rule.Facts();
// {"reads":[["user","age"],["user","plan"]],"computed_reads":false,"reads_complete":true,
//  "reads_data":true,"operators":[">=","and","in","val"],"custom_operators":[],
//  "deterministic":true}
```

`reads_complete` is `false` when the rule builds a path at runtime or
calls a custom operator, and `deterministic` is `false` for `now` and
for any custom operator. The facts describe the compiled rule, so a
branch the optimizer removed is neither read nor listed.

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
native-handle type (`Engine`, `Rule`, `Session`, `TracedSession`,
`DataHandle`) implements `IDisposable`, with a finalizer as fallback;
prefer `using` so the handle is freed when the scope ends.

## Parsed data handles

When the same payload feeds many evaluations, parse it once into a
`DataHandle` and skip the per-call JSON parse:

```csharp
using var data = DataHandle.Parse("""{"user":{"age":25,"plan":"pro"}}""");
var a = rule.Evaluate(data);            // thread-safe one-shot
var b = session.Evaluate(rule, data);   // session hot path
```

A `DataHandle` is immutable, thread-safe, and engine-independent: one
handle can feed rules compiled by different engines, and evaluation
never consumes it. Dispose it after the last evaluation that uses it.

## Typed results

For predicate- and scalar-shaped rules, the typed session variants
return the value as a .NET scalar, with no JSON serialization on the
native side. They take a `DataHandle` (the flows that want typed results are
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
result is not of the requested type. `EvaluateTruthy` never mismatches:
it collapses any result through the engine's configured truthiness
rules (the same coercion `if` / `and` / `or` apply).

`engine.Truthy(valueJson)` applies the same truthiness to a value you
already hold, so a host check agrees with the engine. The argument is
JSON text: `engine.Truthy("[]")` and `engine.Truthy("{}")` are `false`
under the default rules, and a configured `truthy_evaluator` applies.

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
any item failure as exceptional. An item whose error the binding cannot
decode reads as `ErrorTag` `"InternalError"` with the raw item JSON as
`ErrorMessage`, as in the Go, JVM and PHP bindings. The batch call
itself throws only for argument-level problems: a rule compiled by a
different engine, or a `null` element (`ArgumentException`).

## API surface

The binding mirrors the Rust engine's
[API tier model](https://github.com/GoPlasmatic/datalogic-rs#one-api-shape-every-binding).
Rules and results cross the boundary as JSON strings; data crosses as a
JSON string or a pre-parsed `DataHandle`, and the typed session
variants return .NET scalars.

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
| Metered         | `session.EvaluateMetered(rule, data, budget)` → `MeteredResult(Value, Ops)` | What an evaluation costs, under an optional cap |
| Checked         | `engine.Check(rule, mode)` / `engine.CompileChecked(rule)`  | Reject a bad rule before it runs                      |
| Introspection   | `engine.Operators()` / `rule.Facts()` / `engine.Truthy(json)` | Tooling: operator catalogue, what a rule reads      |

## Custom operators

Register C#-implemented operators through the builder. Each callback
receives the operator's pre-evaluated arguments as a JSON-array string
and returns a JSON-value string. If the callback throws, the evaluation
fails with error type `"Custom"` and a message that names the operator
and carries yours. The callback runs on whichever thread evaluates the
rule, so make it thread-safe if you share the engine.

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

**Built-ins win**: registering a name that one of the engine's built-in
operators answers to (`+`, `if`, `var`, an alias such as `?:`) has no
effect at evaluation time; the built-in runs. Call
`WithStrictOperatorNames(true)` before `AddOperator` to make such a
registration throw `EvaluateException` (`ErrorType ==
"ConfigurationError"`) instead:

```csharp
Engine.Builder()
    .WithStrictOperatorNames(true)
    .AddOperator("length", _ => "0");  // throws: `length` is a built-in
```

A name from an operator family the engine leaves out (see
[Operator families](#operator-families)) is free for a custom operator.
Rules, sessions and traced sessions share the engine's callbacks, so
custom operators keep working after the `Engine` is disposed or
finalized.

## Engine configuration

`Engine.Builder().SetConfigJson(json)` sets the evaluation semantics
from a JSON object string: an optional `preset` plus per-field
overrides. An unknown key or value makes `SetConfigJson` throw
`EvaluateException` (error type `ConfigurationError`), so a typo fails
before the engine exists:

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
| `missing_var` | `"null"` (default: a missing variable reads as `null`), `"error"` (raises `VariableNotFound`) |
| `truthy_evaluator` | `"javascript"`, `"python"`, `"strict_boolean"` |
| `numeric_coercion` | object of bools: `empty_string_to_zero`, `null_to_zero`, `bool_to_number`, `reject_non_numeric` |
| `max_recursion_depth` | integer >= 1 |
| `ops_budget` | integer >= 1, or `null` for unbounded (caps the work one evaluation may do; crossing it raises `BudgetExceeded`) |

The `preset` applies first; the remaining keys override individual
fields on top of it. Every binding shares this JSON schema and parses it
with the same core code, so a config that works here works in the
Python, Node, and WASM bindings too. The Rust crate's
[`EvaluationConfig`](https://docs.rs/datalogic-rs/latest/datalogic_rs/struct.EvaluationConfig.html)
documents the full semantics of each knob.

With `"missing_var": "error"`, a `var` / `val` read that finds nothing
throws `EvaluateException` with `ErrorType == "VariableNotFound"`
naming the path, so a typo in a path fails instead of evaluating as
`null`. A default (`{"var": ["x", 0]}`), a present `null`, `missing`,
`missing_some` and `exists` are not misses, and `try` in the rule
catches the error.

The builder also takes `WithTemplating(bool)`,
`WithTemplateKeyEscape(...)` (a prefix that marks a template key as an
output field: with `'$'`, `{"$type": ...}` emits the key `type`; it
takes a `char`, a `Rune`, or a one-character `string`, so an escape
outside the Basic Multilingual Plane works too),
`WithStrictOperatorNames(bool)` (see [Custom operators](#custom-operators))
and `WithFamilies(params string[])` (below).

### Metering: what a rule costs

`session.EvaluateMetered(rule, dataJson, budget = 0)` returns a
`MeteredResult`: the result JSON and the operations the evaluation
charged. A `budget` of `0` uses the engine's `ops_budget` (unbounded if
it has none); any other value caps that one call.

```csharp
using var session = engine.OpenSession();
var (value, ops) = session.EvaluateMetered(rule, """{"xs": [1, 2, 3]}""");
```

One operation is one node the engine dispatches, one item an iterator
walks, or what an operator charges for the data it moves. Crossing the
budget throws `EvaluateException` with `ErrorType == "BudgetExceeded"`
before the work is done, and a `try` in the rule cannot catch it.

### Operator families

`engine.Operators()` returns the operator catalogue as a JSON array,
one object per built-in operator the engine evaluates (`name`,
`aliases`, `family`, `feature`, `min_args`, `max_args`,
`reads_context`, `effect`, `cost`, `scoped_arg`), in the schema of
[`operators.json`](https://github.com/GoPlasmatic/datalogic-rs/blob/main/docs/src/operators/operators.json).

`WithFamilies(...)` keeps an engine to the JSONLogic core plus the
families you name: `ExtString`, `ExtArray`, `ExtObject`, `ExtControl`,
`ExtMath`, `ErrorHandling`, `DateTime`, `Tensor`, `Flagd`. By default an
engine has every family. A name from a family left out compiles as an
unknown operator, which `Check` reports and evaluation rejects with
`ErrorType == "InvalidOperator"`:

```csharp
using var strings = Engine.Builder().WithFamilies("ExtString").Build();
strings.Apply("""{"length": "abc"}""", "null");  // "3"
strings.Apply("""{"now": []}""", "null");         // throws: InvalidOperator
```

An unknown family name throws `EvaluateException` with
`ErrorType == "ConfigurationError"`. With strict operator names on, call
`WithFamilies` before `AddOperator`.

## Error handling

Engine errors are thrown as subclasses of `DatalogicException`:

| Exception           | When                                                      |
|---------------------|-----------------------------------------------------------|
| `ParseException`    | Malformed rule or data JSON (`"ParseError"`), or a rule `CompileChecked` refused (`"CompileError"`) |
| `EvaluateException` | Everything else: a failure while evaluating, an unknown operator (`"InvalidOperator"`, also raised at compile for a multi-key object outside templating), a rejected config or builder option (`"ConfigurationError"`), a typed-result mismatch (`"TypeMismatch"`), a rule from a different engine (`"InvalidArgument"`) |

Misuse of the .NET API throws the standard exceptions:
`ObjectDisposedException` for a disposed handle, `ArgumentException` /
`ArgumentNullException` for a bad argument, and
`InvalidOperationException` for a builder that already built.

The structured fields ride on the base class: `ErrorType` is the stable
engine tag (for example `"Thrown"`, `"TypeError"`, `"VariableNotFound"`,
`"BudgetExceeded"`, or the binding-level `"TypeMismatch"` /
`"InvalidArgument"`), `Operator` the innermost failing operator (custom
operators included), `PathJson` the root-to-leaf error path as a JSON
array, `NodeIdsJson` the compiled-node ids from leaf to root (each
`null` when not applicable), and `Status` the coarse `EvaluationStatus`
the native call returned (`ParseError`, `EvaluationError`,
`TypeMismatch`, `InvalidArgument`, `InternalError`). `DiagnosticsJson`
is set only for `"CompileError"`.

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

`Dispose` is safe to call more than once and from several threads at
once: exactly one call frees the native handle. A finalizer frees a
handle you never dispose, including an `EngineBuilder` dropped before
`Build()`. Disposing a handle while another thread is still using it is
not supported; finish that work first.

## Tracing

```csharp
using var session = engine.OpenTracedSession();
var run = session.Evaluate("""{"+":[{"var":"x"},1]}""", """{"x":41}""");
Console.WriteLine(run.Result);          // 42
Console.WriteLine(run.Steps.Count);     // number of execution steps
Console.WriteLine((string?)run.Pointers!["3"]);  // /+/1: node 3 is the literal 1
```

Same trace envelope as every other binding; the
[React debugger](https://github.com/GoPlasmatic/datalogic-rs/tree/main/ui)
reads it as is. `TracedRun` exposes `Result`, `ExpressionTree`,
`Steps`, `Error`, `StructuredError` and `Pointers` (plus `IsSuccess`);
runtime failures surface inside the run rather than as exceptions.
`Pointers` maps each node id to the RFC 6901 JSON Pointer of the rule
value that node was compiled from, so a debugger can place each step in
the rule as written; it is `null` when the rule does not compile.
`session.Evaluate(rule, data, mode)` compiles the rule in a
`CompileMode`, so a template traces as one. Tracing disables the
optimizer so every operator appears in the trace: use it for
debugging, not hot paths.

## Performance

<!-- canonical-bench v5.1 -->
Geomean across 51 operator benchmark suites (Apple M2 Pro, median of 3 runs; pairwise shared-suite ratios per the [methodology](https://github.com/GoPlasmatic/datalogic-rs/blob/main/tools/benchmark/BENCHMARK.md)): the native Rust core evaluates at **10.3 ns/op**, 7.0× faster than json-logic-engine (compiled, the fastest JS engine), 28.1× faster than jsonlogic-rs (the closest Rust alternative), and 83.6× faster than the json-logic-js reference implementation. The WASM build under Node measures 900.5 ns geomean (88× native); on Node servers, prefer `@goplasmatic/datalogic-node`.

The P/Invoke boundary adds a small per-call marshalling cost on top of
the core numbers.

## Building from source

The binding lives in
[`bindings/dotnet/`](https://github.com/GoPlasmatic/datalogic-rs/tree/main/bindings/dotnet).
At runtime the native library resolves in order: the
`DATALOGIC_NATIVE_LIB` env var (absolute path), NuGet's
`runtimes/<rid>/native/` layout, then the in-tree C ABI target dir. On
first use the binding checks that the resolved library implements C ABI
v2.1 or a later v2 minor (`datalogic_abi_version() == 2`,
`datalogic_abi_minor() >= 1`) and fails with a rebuild hint if it picks
up a stale library. A fresh clone needs the C ABI built once. The .NET
10 SDK builds both target frameworks; SDK 8 or 9 builds `net8.0` only:

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
