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

    // Rules, sessions and traced sessions hold their own reference on the
    // native engine, so disposing the Engine (or letting it be finalized)
    // must not free the callback delegates they dispatch into.
    [Fact]
    public void Custom_operator_survives_engine_dispose_and_gc()
        => AssertTripleSurvives(dispose: true);

    [Fact]
    public void Custom_operator_survives_engine_finalizer()
        => AssertTripleSurvives(dispose: false);

    private static void AssertTripleSurvives(bool dispose)
    {
        var (rule, session, traced) = OpenOnDroppedEngine(dispose);
        try
        {
            for (var i = 0; i < 50; i++)
            {
                if (i % 10 == 0) ForceGc();
                Assert.Equal("42", rule.Evaluate("""{"n":14}"""));
                Assert.Equal("42", session.Evaluate(rule, """{"n":14}"""));
                var run = traced.Evaluate("""{"triple":[{"var":"n"}]}""", """{"n":14}""");
                Assert.Null(run.Error);
                Assert.Equal("42", run.Result?.ToJsonString());
            }
        }
        finally
        {
            traced.Dispose();
            session.Dispose();
            rule.Dispose();
        }
    }

    // Kept out of line so no reference to the Engine survives on the
    // caller's stack.
    [System.Runtime.CompilerServices.MethodImpl(System.Runtime.CompilerServices.MethodImplOptions.NoInlining)]
    private static (Rule, Session, TracedSession) OpenOnDroppedEngine(bool dispose)
    {
        var engine = Engine.Builder()
            .AddOperator("triple", argsJson =>
            {
                var n = JsonNode.Parse(argsJson)!.AsArray()[0]!.GetValue<double>();
                return JsonValue.Create(n * 3).ToJsonString();
            })
            .Build();
        var rule = engine.Compile("""{"triple":[{"var":"n"}]}""");
        var session = engine.OpenSession();
        var traced = engine.OpenTracedSession();
        if (dispose) engine.Dispose();
        return (rule, session, traced);
    }

    private static void ForceGc()
    {
        for (var i = 0; i < 3; i++)
        {
            GC.Collect();
            GC.WaitForPendingFinalizers();
            GC.Collect();
        }
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

public class LifecycleTests
{
    // Exactly one of the concurrent Dispose calls frees each handle; a
    // double free would crash the test host.
    [Fact]
    public void Dispose_is_safe_from_many_threads_at_once()
    {
        for (var round = 0; round < 20; round++)
        {
            var engine = new Engine();
            var rule = engine.Compile("""{"var":"x"}""");
            var data = DataHandle.Parse("""{"x":1}""");
            var session = engine.OpenSession();
            var traced = engine.OpenTracedSession();
            using var start = new ManualResetEventSlim(false);
            var threads = Enumerable.Range(0, 8).Select(_ => new Thread(() =>
            {
                start.Wait();
                traced.Dispose();
                session.Dispose();
                data.Dispose();
                rule.Dispose();
                engine.Dispose();
            })).ToList();
            threads.ForEach(t => t.Start());
            start.Set();
            threads.ForEach(t => t.Join());
            Assert.Throws<ObjectDisposedException>(() => rule.Evaluate("{}"));
        }
    }
}
