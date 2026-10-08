# .NET / C# (P/Invoke)

The .NET binding `Goplasmatic.Datalogic` is a P/Invoke wrapper over the shared C ABI. It ships **.NET 8.0** and **.NET 10.0** builds and uses source-generated `LibraryImport` stubs, so it is **NativeAOT-ready**.

## Installation

Add the NuGet package to your project:

```bash
dotnet add package Goplasmatic.Datalogic
```

or reference it in the project file:

```xml
<PackageReference Include="Goplasmatic.Datalogic" Version="5.8.1" />
```

The package ships precompiled shared libraries (`.so`, `.dylib`, `.dll`) under NuGet's standard `runtimes/` structure, and MSBuild picks the one for the target runtime identifier (RID) during publish. .NET 8 and 9 apps use the `net8.0` build; .NET 10 and newer use `net10.0`.

## Quick Start

### One-Shot Evaluation

```csharp
using Goplasmatic.Datalogic;

using var engine = new Engine();
var result = engine.Apply("""{"+": [1, 2, 3]}""", "{}");
Console.WriteLine(result); // "6"
```

### Reusable Compiled Rules

Compile rules that you evaluate repeatedly. Use C#'s `using var` syntax or `using` blocks to dispose of native engine and rule memory:

```csharp
using Goplasmatic.Datalogic;

using var engine = new Engine();
using var rule = engine.Compile("""{"if": [{ ">": [{"var": "score"}, 50] }, "pass", "fail"]}""");

Console.WriteLine(rule.Evaluate("""{"score": 75}""")); // "pass"
Console.WriteLine(rule.Evaluate("""{"score": 30}""")); // "fail"
```

### Arena Recycling with `Session`

To recycle memory allocations in hot loops, open a `Session`:

```csharp
using Goplasmatic.Datalogic;

using var engine = new Engine();
using var rule = engine.Compile("""{"var": "user.name"}""");

using var session = engine.OpenSession();
foreach (var input in dataset)
{
    // Reuses the session's arena, reset at the start of each call
    var name = session.Evaluate(rule, input);
    Console.WriteLine(name);
}
```

## Checking and Inspecting Rules

The engine can report problems in a rule before it runs, and describe a compiled rule:

```csharp
var diagnostics = engine.Check("""{"if": [true, {"vr": "x"}]}""");
// [{"code":"UnknownOperator","severity":"error",
//   "message":"unknown operator `vr`; did you mean `var`?","pointer":"/if/1","operator":"vr"}]

try
{
    using var checkedRule = engine.CompileChecked(ruleJson);
    var facts = checkedRule.Facts();  // {"reads":[["user","age"]], "operators":[...], "deterministic":true, ...}
}
catch (ParseException e) when (e.ErrorType == "CompileError")
{
    Console.WriteLine(e.DiagnosticsJson);  // every problem, each with a JSON Pointer into the rule
}
```

| Method | Returns |
|--------|---------|
| `engine.Check(rule, mode = CompileMode.Engine)` | Every problem the engine can see, as a JSON array of `{code, severity, message, pointer, operator}`; it does not throw for a bad rule |
| `engine.CompileChecked(rule)` | A `Rule`, or `ParseException` with `ErrorType == "CompileError"` if `Check` finds an error |
| `engine.CompileTemplate(rule)` / `CompileStrict(rule)` | A `Rule` compiled with templating on or off, whatever the engine's own mode |
| `rule.Facts()` | The data paths the rule reads, the operators it calls, and whether it is deterministic, as JSON |
| `engine.Operators()` | The operator catalogue, in the schema of [`operators.json`](https://github.com/GoPlasmatic/datalogic-rs/blob/main/docs/src/operators/operators.json) |
| `engine.Truthy(valueJson)` | Whether a JSON value is truthy under the engine's configured truthiness |

`Engine.Builder()` also takes `WithFamilies("ExtString", ...)`, which keeps the engine to the JSONLogic core plus the operator families you name, and `WithStrictOperatorNames(true)`, which makes `AddOperator` throw for a name a built-in operator answers to. The engine config accepts `"missing_var": "error"`, which turns a `var` read that finds nothing into a `VariableNotFound` error.

## Concurrency

*   `Engine` and `Rule` instances are thread-safe; you can share them globally.
*   `Session` instances are **not** thread-safe; keep each one local to a single thread.
*   The native-handle types (`Engine`, `Rule`, `Session`, `TracedSession`, `DataHandle`) implement `IDisposable`. `Dispose()` frees the handle exactly once, even when several threads call it. If you never call it, a finalizer releases the native memory; dispose anyway, so native memory does not wait for the garbage collector.
*   `EngineBuilder` is not disposable. `Build()` releases its native builder, and a finalizer frees a builder you drop before `Build()`.
*   Rules, sessions and traced sessions keep the engine's custom-operator callbacks alive, so custom operators keep working after the `Engine` is disposed or finalized.

## Going deeper

- [C ABI internals: memory management & thread safety](c-abi.md): the native-heap ownership rules every FFI binding shares
- [Engine configuration semantics](advanced/configuration.md)
- [Rule analysis](advanced/rule-analysis.md): diagnostic codes, the facts fields and the operator catalogue
- [Package README on NuGet](https://github.com/GoPlasmatic/datalogic-rs/tree/main/bindings/dotnet#readme): full API surface, error types, and platform table
