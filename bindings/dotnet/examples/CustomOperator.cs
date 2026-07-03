// custom-operator: register a C# `double` operator and call it from a rule.
// Custom operators receive their pre-evaluated arguments as a JSON-array
// string and return a JSON-value string. Built-in names always win.
//
// Run from bindings/dotnet/:
//   dotnet run --project examples -- custom-operator

using System.Globalization;
using System.Text.Json.Nodes;

namespace Goplasmatic.Datalogic.Examples;

internal static class CustomOperatorExample
{
    internal static void Run()
    {
        using var engine = Engine.Builder()
            .AddOperator("double", argsJson =>
            {
                var n = JsonNode.Parse(argsJson)![0]!.GetValue<double>();
                return (n * 2).ToString(CultureInfo.InvariantCulture);
            })
            .Build();

        Console.WriteLine(engine.Apply("""{"double": [21]}""", "{}")); // 42
    }
}
