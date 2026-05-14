# `Goplasmatic.Datalogic` — .NET binding for [`datalogic-rs`](../../crates/datalogic-rs)

P/Invoke wrapper over the shared [`bindings/c`](../c) C ABI. Targets
`net8.0` with `LibraryImport` source-generated stubs, so the assembly is
NativeAOT-ready out of the box.

## Install

```bash
dotnet add package Goplasmatic.Datalogic
```

The NuGet package ships platform binaries under
`runtimes/<rid>/native/`; `dotnet publish` picks the right one for the
target RID automatically.

## Quick start

```csharp
using Goplasmatic.Datalogic;

using var engine = new Engine();
var result = engine.Apply("""{"+":[1,2]}""", "{}");  // "3"
```

Reusing a compiled rule:

```csharp
using var engine = new Engine();
using var rule = engine.Compile("""{"var":"x"}""");
foreach (var x in new[] { 1, 2, 3 })
{
    Console.WriteLine(rule.Evaluate($"{{\"x\":{x}}}"));
}
```

Hot-loop session (arena reuse):

```csharp
using var session = engine.OpenSession();
foreach (var data in inputs)
{
    var result = session.Evaluate(rule, data);
}
```

Traced evaluation:

```csharp
using var session = engine.OpenTracedSession();
var run = session.Evaluate("""{"+":[{"var":"x"},1]}""", """{"x":41}""");
Console.WriteLine(run.Result);          // 42
Console.WriteLine(run.Steps.Count);     // number of executed nodes
```

Custom operator:

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

## Build & test (development)

The native library is resolved at runtime in this order:

1. `DATALOGIC_NATIVE_LIB` env var (absolute path).
2. NuGet's `runtimes/<rid>/native/` layout.
3. The C ABI's cargo target dir (`bindings/c/target/release/`) — for
   in-tree dev.

So a fresh clone needs the C ABI built once:

```bash
cd ../c && cargo build --release
cd ../dotnet
dotnet build
dotnet test
```

## Layout

```
bindings/dotnet/
├── Datalogic.sln
├── src/Datalogic/
│   ├── Datalogic.csproj           # NuGet metadata; targets net8.0
│   ├── Engine.cs                  # public API
│   ├── EngineBuilder.cs           # custom operators
│   ├── Rule.cs
│   ├── Session.cs
│   ├── TracedSession.cs
│   ├── DatalogicException.cs
│   └── Native/
│       ├── NativeMethods.cs       # hand-written LibraryImport stubs
│       └── NativeLibraryResolver.cs
└── tests/Datalogic.Tests/         # xUnit
```

## Threading & memory

- `Engine`, `Rule`, `TracedSession` are thread-safe — share freely.
- `Session` is NOT thread-safe — open one per thread.
- Every public type implements `IDisposable`. The finalizer is a
  best-effort fallback for callers who forget `using` / `Dispose`.
