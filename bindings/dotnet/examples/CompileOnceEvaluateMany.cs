// compile-once-evaluate-many: compile a rule once, evaluate it against
// ~100k payloads, and print the last result plus a rough per-evaluation cost.
// Then the C ABI v2 hot path: parse the data once into a DataHandle and
// evaluate via a session (typed results and batch included).
//
// Run from bindings/dotnet/:
//   dotnet run --project examples -- compile-once

using System.Diagnostics;

namespace Goplasmatic.Datalogic.Examples;

internal static class CompileOnceEvaluateMany
{
    private const int Iterations = 100_000;

    internal static void Run()
    {
        using var engine = new Engine();
        using var rule = engine.Compile(
            """{"*": [{"var": "price"}, {"-": [1, {"var": "discount"}]}]}""");

        var last = "";
        var stopwatch = Stopwatch.StartNew();
        for (var i = 0; i < Iterations; i++)
        {
            last = rule.Evaluate($$"""{"price": {{100 + i % 100}}, "discount": 0.2}""");
        }
        stopwatch.Stop();

        Console.WriteLine($"last result: {last}");
        Console.WriteLine(
            $"{Iterations} evaluations, ~{stopwatch.Elapsed.TotalNanoseconds / Iterations:F0} ns/op");

        // Hot path: parse the payload once into a DataHandle, then
        // evaluate with a per-thread session — zero JSON parse work per
        // call, and typed results skip serialization entirely.
        using var session = engine.OpenSession();
        using var data = DataHandle.Parse("""{"price": 100, "discount": 0.2}""");
        Console.WriteLine($"data-handle result: {session.Evaluate(rule, data)}");
        Console.WriteLine($"typed result: {session.EvaluateDouble(rule, data)}");

        // Batch: one rule x N pre-parsed payloads in a single native
        // call; per-item failures land in their EvaluationResult instead
        // of throwing.
        var handles = new List<DataHandle>();
        try
        {
            for (var i = 0; i < 3; i++)
            {
                handles.Add(DataHandle.Parse($$"""{"price": {{100 + i}}, "discount": 0.2}"""));
            }
            foreach (var item in session.EvaluateBatch(rule, handles))
            {
                Console.WriteLine($"batch item: {item.Value}");
            }
        }
        finally
        {
            foreach (var handle in handles) handle.Dispose();
        }
    }
}
