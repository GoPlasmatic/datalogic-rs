// compile-once-evaluate-many: compile a rule once, evaluate it against
// ~100k payloads, and print the last result plus a rough per-evaluation cost.
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
    }
}
