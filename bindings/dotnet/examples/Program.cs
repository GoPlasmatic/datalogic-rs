// Examples entry point. Dispatches on the first argument.
//
// Run from bindings/dotnet/ (needs the C ABI built once:
// `cargo build --release --manifest-path ../c/Cargo.toml`):
//   dotnet run --project examples -- getting-started
//   dotnet run --project examples -- compile-once
//   dotnet run --project examples -- custom-operator

namespace Goplasmatic.Datalogic.Examples;

internal static class Program
{
    private static int Main(string[] args)
    {
        switch (args.Length == 1 ? args[0] : null)
        {
            case "getting-started":
                GettingStarted.Run();
                return 0;
            case "compile-once":
                CompileOnceEvaluateMany.Run();
                return 0;
            case "custom-operator":
                CustomOperatorExample.Run();
                return 0;
            default:
                Console.Error.WriteLine(
                    "usage: dotnet run --project examples -- <getting-started|compile-once|custom-operator>");
                return 2;
        }
    }
}
