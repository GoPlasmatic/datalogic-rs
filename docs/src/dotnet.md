# .NET / C# (P/Invoke)

The .NET binding `Goplasmatic.Datalogic` is a P/Invoke wrapper over the shared C ABI. It targets **.NET 8.0** and uses source-generated `LibraryImport` stubs, making it fully **NativeAOT-ready**.

## Installation

Add the NuGet package to your project:

```bash
dotnet add package Goplasmatic.Datalogic
```

The package ships precompiled shared libraries (`.so`, `.dylib`, `.dll`) under NuGet's standard `runtimes/` structure. MSBuild picks the correct target runtime identifier (RID) during publish.

## Quick Start

### One-Shot Evaluation

```csharp
using Goplasmatic.Datalogic;

using var engine = new Engine();
var result = engine.Apply("""{"+": [1, 2, 3]}""", "{}");
Console.WriteLine(result); // "6"
```

### Reusable Compiled Rules

Always compile rules when executing them repeatedly. Use C#'s `using var` syntax or `using` blocks to dispose of native engine and rule memory:

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
    // Reuses the session's memory arena; does not allocate fresh memory
    var name = session.Evaluate(rule, input);
    Console.WriteLine(name);
}
```

## Concurrency

*   `Engine` and `Rule` instances are thread-safe and can be shared globally.
*   `Session` instances are **not** thread-safe and must be kept local to individual threads.
*   All public types implement `IDisposable`. If a developer forgets to call `Dispose()`, the wrappers contain finalizers to release native memory as a best-effort fallback. However, explicit disposal is highly recommended to prevent resource starvation.

## Going deeper

- [C ABI internals: memory management & thread safety](c-abi.md) — the native-heap ownership rules every FFI binding shares
- [Engine configuration semantics](advanced/configuration.md)
- [Package README on NuGet](https://github.com/GoPlasmatic/datalogic-rs/tree/main/bindings/dotnet#readme) — full API surface, error types, and platform table
