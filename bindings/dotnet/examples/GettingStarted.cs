// getting-started: one-shot JSONLogic evaluation with the datalogic .NET binding.
//
// Run from bindings/dotnet/:
//   dotnet run --project examples -- getting-started

namespace Goplasmatic.Datalogic.Examples;

internal static class GettingStarted
{
    internal static void Run()
    {
        const string rule =
            """{"and": [{">=": [{"var": "age"}, 18]}, {"==": [{"var": "status"}, "active"]}]}""";
        const string data = """{"age": 25, "status": "active"}""";

        using var engine = new Engine();
        Console.WriteLine(engine.Apply(rule, data)); // true
    }
}
