// Boundary-benchmark runner for the .NET binding (`runtime: "dotnet"`).
//
// Exercises the binding's public API on the ABI-v2 tiers. Surface used:
//
//   - DataHandle.Parse(string json) : IDisposable
//   - string Session.Evaluate(Rule rule, DataHandle data)   (overload)
//   - EvaluationResult[] Session.EvaluateMany(IReadOnlyList<Rule>, DataHandle)
//     — EvaluationResult.Value returns the JSON or throws on item failure.
//
// Modes: session-evaluate, session-evaluate-data,
// session-evaluate-many-100 (ns_op per evaluation: call/100),
// rule-evaluate, engine-apply-oneshot.
//
// Timing discipline (BINDINGS-OVERHEAD.md appendix): warmup 2,000
// iterations, pilot to ~250 ms per sample, median of 5, results
// consumed into a sink.
//
// Build/run:
//   dotnet run -c Release --project runner-dotnet -- <workloads-dir> \
//     [--modes=a,b] [--workloads=x,y]
// The binding resolves libdatalogic_c from bindings/c/target/release in
// dev trees (or set DATALOGIC_NATIVE_LIB).

using System.Diagnostics;
using Goplasmatic.Datalogic;

const string RuntimeName = "dotnet";
const int Warmup = 2_000;
const double TargetSampleNs = 250e6;
const double PilotMinNs = 10e6;
const int Samples = 5;
const int ManyN = 100;

long globalSink = 0;

double Measure(Func<long, long> batch)
{
    globalSink += batch(Warmup);

    long n = 32;
    double perOp;
    while (true)
    {
        var sw = Stopwatch.StartNew();
        globalSink += batch(n);
        sw.Stop();
        double elapsedNs = sw.Elapsed.TotalMilliseconds * 1e6;
        if (elapsedNs >= PilotMinNs)
        {
            perOp = elapsedNs / n;
            break;
        }
        n *= 2;
    }

    long iters = Math.Max(1, (long)Math.Round(TargetSampleNs / perOp));
    var samples = new double[Samples];
    for (int s = 0; s < Samples; s++)
    {
        var sw = Stopwatch.StartNew();
        globalSink += batch(iters);
        sw.Stop();
        samples[s] = sw.Elapsed.TotalMilliseconds * 1e6 / iters;
    }
    Array.Sort(samples);
    return samples[Samples / 2];
}

void Emit(string mode, string workload, double nsOp) =>
    Console.WriteLine(
        $"{{\"runtime\": \"{RuntimeName}\", \"mode\": \"{mode}\", \"workload\": \"{workload}\", \"ns_op\": {nsOp:F3}}}");

void Verify(string mode, string workload, string got, string expected)
{
    if (got != expected)
    {
        Console.Error.WriteLine(
            $"runner-dotnet: verification failed for mode={mode} workload={workload}\n" +
            $"  expected: {expected}\n  got:      {got}");
        Environment.Exit(1);
    }
}

// ---- CLI ----
string? dir = null;
string[]? modeFilter = null;
string[]? workloadFilter = null;
foreach (var arg in args)
{
    if (arg.StartsWith("--modes=")) modeFilter = arg["--modes=".Length..].Split(',');
    else if (arg.StartsWith("--workloads=")) workloadFilter = arg["--workloads=".Length..].Split(',');
    else dir = arg;
}
if (dir is null)
{
    Console.Error.WriteLine("usage: boundary-dotnet <workloads-dir> [--modes=a,b] [--workloads=x,y]");
    return 1;
}

using var engine = new Engine();

foreach (var name in new[] { "simple", "eligibility", "array100" })
{
    if (workloadFilter is not null && !workloadFilter.Contains(name)) continue;

    string Read(string suffix) => File.ReadAllText(Path.Combine(dir, $"{name}.{suffix}.json"));
    string ruleJson = Read("rule"), dataJson = Read("data"), expected = Read("expected");

    using var rule = engine.Compile(ruleJson);
    using var session = engine.OpenSession();

    // v2: parse-once data handle.
    using var dataHandle = DataHandle.Parse(dataJson);

    // 100 identical rules, compiled separately (a rule-set of identical
    // rules — separate compiles so the batch doesn't flatter one hot
    // compiled tree).
    var manyRules = new Rule[ManyN];
    for (int i = 0; i < ManyN; i++) manyRules[i] = engine.Compile(ruleJson);

    var modes = new (string Name, Action Verify, Func<long, long> Batch, double PerCallEvals)[]
    {
        ("session-evaluate",
            () => Verify("session-evaluate", name, session.Evaluate(rule, dataJson), expected),
            n => { long sink = 0; for (long i = 0; i < n; i++) sink += session.Evaluate(rule, dataJson).Length; return sink; },
            1.0),
        ("session-evaluate-data",
            () => Verify("session-evaluate-data", name, session.Evaluate(rule, dataHandle), expected),
            n => { long sink = 0; for (long i = 0; i < n; i++) sink += session.Evaluate(rule, dataHandle).Length; return sink; },
            1.0),
        ("session-evaluate-many-100",
            () =>
            {
                // v2: N rules x one data handle; .Value throws on any
                // item failure, which is exactly what verification wants.
                foreach (var r in session.EvaluateMany(manyRules, dataHandle))
                    Verify("session-evaluate-many-100", name, r.Value, expected);
            },
            n =>
            {
                long sink = 0;
                for (long i = 0; i < n; i++)
                {
                    var results = session.EvaluateMany(manyRules, dataHandle);
                    sink += results[0].Value.Length + results[ManyN - 1].Value.Length;
                }
                return sink;
            },
            ManyN),
        ("rule-evaluate",
            () => Verify("rule-evaluate", name, rule.Evaluate(dataJson), expected),
            n => { long sink = 0; for (long i = 0; i < n; i++) sink += rule.Evaluate(dataJson).Length; return sink; },
            1.0),
        ("engine-apply-oneshot",
            () => Verify("engine-apply-oneshot", name, engine.Apply(ruleJson, dataJson), expected),
            n => { long sink = 0; for (long i = 0; i < n; i++) sink += engine.Apply(ruleJson, dataJson).Length; return sink; },
            1.0),
    };

    foreach (var (modeName, verify, batch, perCallEvals) in modes)
    {
        if (modeFilter is not null && !modeFilter.Contains(modeName)) continue;
        verify();
        Emit(modeName, name, Measure(batch) / perCallEvals);
    }

    foreach (var r in manyRules) r.Dispose();
}

Console.Error.WriteLine($"runner-dotnet: sink={globalSink}");
return 0;
