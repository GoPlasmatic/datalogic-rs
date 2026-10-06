// SPDX-License-Identifier: Apache-2.0
//
// The cross-binding scenarios in bindings/scenarios/api.json, through the
// .NET API. Every binding runs the same file (see bindings/BINDINGS.md).

using System.Text.Json.Nodes;
using Xunit;

using Goplasmatic.Datalogic;

namespace Goplasmatic.Datalogic.Tests;

public class ScenarioTests
{
    public static IEnumerable<object[]> Cases()
    {
        var dir = new DirectoryInfo(AppContext.BaseDirectory);
        while (dir != null && !File.Exists(Path.Combine(dir.FullName, "Cargo.toml")))
        {
            dir = dir.Parent;
        }
        var all = JsonNode.Parse(File.ReadAllText(Path.Combine(dir!.FullName, "bindings/scenarios/api.json")))!.AsArray();
        foreach (var c in all)
        {
            if (c is JsonObject o)
            {
                yield return new object[] { $"{o["call"]}: {o["description"]}", o.ToJsonString() };
            }
        }
    }

    private static Engine EngineFor(JsonNode c)
    {
        var o = c["engine"];
        var b = Engine.Builder();
        if (o?["templating"]?.GetValue<bool>() == true) b.WithTemplating(true);
        if (o?["template_key_escape"] is JsonNode esc) b.WithTemplateKeyEscape(esc.GetValue<string>()[0]);
        if (o?["config"] is JsonNode cfg) b.SetConfigJson(cfg.ToJsonString());
        if (o?["families"] is JsonArray fams)
        {
            b.WithFamilies(fams.Select(f => (string)f!).ToArray());
        }
        return b.Build();
    }

    private static CompileMode Mode(JsonNode c) => (string?)c["mode"] switch
    {
        "strict" => CompileMode.Strict,
        "template" => CompileMode.Template,
        _ => CompileMode.Engine,
    };

    private static JsonNode? Run(JsonNode c)
    {
        using var e = EngineFor(c);
        var rule = c["rule"]?.ToJsonString() ?? "null";
        var data = c["data"]?.ToJsonString() ?? "null";
        switch ((string)c["call"]!)
        {
            case "check":
            {
                var pairs = new JsonArray();
                foreach (var d in JsonNode.Parse(e.Check(rule, Mode(c)))!.AsArray())
                {
                    pairs.Add(new JsonArray(d!["code"]!.DeepClone(), d["pointer"]!.DeepClone()));
                }
                return pairs;
            }
            case "truthy":
                return JsonValue.Create(e.Truthy(c["value"]!.ToJsonString()));
            case "facts":
            {
                using var r = e.Compile(rule);
                return JsonNode.Parse(r.Facts());
            }
            case "trace":
            {
                using var t = e.OpenTracedSession();
                var run = t.Evaluate(rule, data);
                var pointers = new JsonArray();
                foreach (var p in (run.Pointers ?? new JsonObject())
                             .Select(kv => (string)kv.Value!)
                             .Distinct()
                             .OrderBy(p => p, StringComparer.Ordinal))
                {
                    pointers.Add(p);
                }
                return new JsonObject { ["result"] = run.Result?.DeepClone(), ["pointers"] = pointers };
            }
            case "metered":
            {
                using var r = e.Compile(rule);
                using var s = e.OpenSession();
                return JsonNode.Parse(s.EvaluateMetered(r, data, c["budget"]!.GetValue<ulong>()).Value);
            }
            default:
            {
                using var r = (string)c["call"]! switch
                {
                    "compile_template" => e.CompileTemplate(rule),
                    "compile_strict" => e.CompileStrict(rule),
                    "compile_checked" => e.CompileChecked(rule),
                    _ => e.Compile(rule),
                };
                return JsonNode.Parse(r.Evaluate(data));
            }
        }
    }

    [Theory]
    [MemberData(nameof(Cases))]
    public void Scenario(string name, string json)
    {
        var c = JsonNode.Parse(json)!;
        if (c["error"] is JsonNode err)
        {
            var ex = Assert.ThrowsAny<DatalogicException>(() => Run(c));
            Assert.Equal((string?)err, ex.ErrorType);
            return;
        }
        var got = Run(c);
        if (c["diagnostics"] is JsonNode diags)
        {
            Assert.True(JsonNode.DeepEquals(diags, got), $"{name}: {got?.ToJsonString()}");
        }
        else if (c["trace"] is JsonNode trace)
        {
            Assert.True(JsonNode.DeepEquals(trace, got), $"{name}: {got?.ToJsonString()}");
        }
        else if (c["facts"] is JsonObject facts)
        {
            foreach (var (k, v) in facts)
            {
                Assert.True(JsonNode.DeepEquals(v, got![k]), $"{name}: {k} = {got[k]?.ToJsonString()}");
            }
        }
        else
        {
            Assert.True(JsonNode.DeepEquals(c["result"], got), $"{name}: {got?.ToJsonString()}");
        }
    }
}
