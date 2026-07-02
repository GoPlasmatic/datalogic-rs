// SPDX-License-Identifier: Apache-2.0

using System.Text.Json;
using System.Text.Json.Nodes;

using Xunit;

using Goplasmatic.Datalogic;

namespace Goplasmatic.Datalogic.Tests;

public class EngineTests
{
    [Fact]
    public void Version_matches_pkg_version()
    {
        Assert.False(string.IsNullOrEmpty(Engine.Version));
    }

    [Fact]
    public void Apply_one_shot_returns_json_result()
    {
        using var engine = new Engine();
        var result = engine.Apply("""{"+":[1,2]}""", "{}");
        Assert.Equal("3", result);
    }

    [Fact]
    public void Apply_returns_parsed_json_via_ApplyJson()
    {
        using var engine = new Engine();
        var node = engine.ApplyJson("""{"+":[1,2,3]}""", "{}");
        Assert.NotNull(node);
        Assert.Equal(6, node!.GetValue<int>());
    }

    [Fact]
    public void Compile_then_evaluate_reuses_rule()
    {
        using var engine = new Engine();
        using var rule = engine.Compile("""{"var":"x"}""");

        foreach (var x in new[] { 1, 7, 42 })
        {
            var result = rule.Evaluate($"{{\"x\":{x}}}");
            Assert.Equal(x.ToString(), result);
        }
    }

    [Fact]
    public void Session_reuses_arena_across_calls()
    {
        using var engine = new Engine();
        using var rule = engine.Compile("""{"*":[{"var":"x"},2]}""");
        using var session = engine.OpenSession();

        foreach (var x in new[] { 3, 5, 8 })
        {
            var result = session.Evaluate(rule, $"{{\"x\":{x}}}");
            Assert.Equal((x * 2).ToString(), result);
        }
        Assert.True(session.AllocatedBytes > 0);
    }

    [Fact]
    public void Parse_error_throws_ParseException()
    {
        using var engine = new Engine();
        var ex = Assert.Throws<ParseException>(() => engine.Compile("not-json{{"));
        Assert.Equal("ParseError", ex.ErrorType);
        Assert.False(string.IsNullOrEmpty(ex.Message));
    }

    [Fact]
    public void Evaluate_error_throws_with_operator_and_path()
    {
        using var engine = new Engine();
        using var rule = engine.Compile("""{"throw":"boom"}""");
        var ex = Assert.Throws<EvaluateException>(() => rule.Evaluate("{}"));
        Assert.Equal("Thrown", ex.ErrorType);
        Assert.NotNull(ex.PathJson);
        Assert.StartsWith("[", ex.PathJson);
    }

    [Fact]
    public void Templating_engine_constructs()
    {
        using var engine = new Engine(templating: true);
        Assert.False(string.IsNullOrEmpty(engine.Apply("""{"+":[1,1]}""", "{}")));
    }

    [Fact]
    public void Flagd_sem_ver_operator_is_available()
    {
        // Smoke test that the C ABI's flagd feature is wired up through .NET.
        using var engine = new Engine();
        var result = engine.Apply("""{"sem_ver":["1.2.3","<","2.0.0"]}""", "{}");
        Assert.Equal("true", result);
    }
}

public class TracedSessionTests
{
    [Fact]
    public void Evaluate_returns_result_and_steps()
    {
        using var engine = new Engine();
        using var session = engine.OpenTracedSession();
        var run = session.Evaluate("""{"+":[{"var":"x"},1]}""", """{"x":41}""");

        Assert.True(run.IsSuccess);
        Assert.NotNull(run.Result);
        Assert.Equal(42, run.Result!.GetValue<int>());
        Assert.NotEmpty(run.Steps);
        Assert.NotNull(run.ExpressionTree);
        Assert.Null(run.Error);
    }

    [Fact]
    public void Evaluate_surfaces_runtime_error_in_payload()
    {
        using var engine = new Engine();
        using var session = engine.OpenTracedSession();
        var run = session.Evaluate("""{"throw":"boom"}""", "{}");

        Assert.False(run.IsSuccess);
        Assert.NotNull(run.Error);
        Assert.NotNull(run.StructuredError);
    }
}

public class CustomOperatorTests
{
    [Fact]
    public void Builder_registers_custom_operator()
    {
        using var engine = Engine.Builder()
            .AddOperator("double", argsJson =>
            {
                var arr = JsonNode.Parse(argsJson)!.AsArray();
                var n = arr[0]!.GetValue<double>();
                return JsonValue.Create(n * 2).ToJsonString();
            })
            .Build();

        var result = engine.Apply("""{"double":[21]}""", "{}");
        Assert.Equal("42", result);
    }

    [Fact]
    public void Builder_custom_operator_error_propagates()
    {
        using var engine = Engine.Builder()
            .AddOperator("boom", _ => throw new InvalidOperationException("custom-failure"))
            .Build();

        var ex = Assert.Throws<EvaluateException>(() => engine.Apply("""{"boom":[]}""", "{}"));
        Assert.Contains("custom-failure", ex.Message);
    }
}

public class BuilderConfigTests
{
    [Fact]
    public void SetConfigJson_strict_preset_takes_effect()
    {
        // Default config: null coerces to 0 and the sum evaluates.
        using var lenient = new Engine();
        Assert.Equal("1", lenient.Apply("""{"+":[null,1]}""", "{}"));

        // Strict preset: the same rule rejects the non-numeric null.
        using var strict = Engine.Builder()
            .SetConfigJson("""{"preset":"strict"}""")
            .Build();
        Assert.Throws<EvaluateException>(() => strict.Apply("""{"+":[null,1]}""", "{}"));
    }

    [Fact]
    public void SetConfigJson_rejects_bad_input()
    {
        // Malformed JSON surfaces the parser's message.
        var malformed = Assert.Throws<EvaluateException>(
            () => Engine.Builder().SetConfigJson("not-json{{"));
        Assert.Equal("ConfigurationError", malformed.ErrorType);
        Assert.False(string.IsNullOrEmpty(malformed.Message));

        // Unknown enum values fail loudly instead of being ignored.
        var bogus = Assert.Throws<EvaluateException>(
            () => Engine.Builder().SetConfigJson("""{"preset":"bogus"}"""));
        Assert.Contains("bogus", bogus.Message);
    }

    [Fact]
    public void SetConfigJson_chains_with_templating()
    {
        using var engine = Engine.Builder()
            .WithTemplating(true)
            .SetConfigJson("""{"preset":"strict"}""")
            .Build();
        Assert.Equal("3", engine.Apply("""{"+":[1,2]}""", "{}"));
        Assert.Throws<EvaluateException>(() => engine.Apply("""{"+":[null,1]}""", "{}"));
    }
}
