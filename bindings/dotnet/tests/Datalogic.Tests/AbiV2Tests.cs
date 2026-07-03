// SPDX-License-Identifier: Apache-2.0
//
// Coverage for the C ABI v2 surface: the load-time ABI assert, parsed
// data handles, typed evaluations, batch/many, and the richer error
// mapping (status + tag + operator + path).

using Xunit;

using Goplasmatic.Datalogic;
using Goplasmatic.Datalogic.Native;

namespace Goplasmatic.Datalogic.Tests;

public class AbiVersionTests
{
    [Fact]
    public void Native_library_reports_abi_v2()
    {
        // Positive path of the load-time assert: initialisation succeeds
        // and the resolved native library speaks exactly the ABI
        // revision this binding is written against.
        NativeInit.EnsureLoaded();
        Assert.Equal(2u, NativeMethods.AbiVersion);
        Assert.Equal(NativeMethods.AbiVersion, NativeMethods.datalogic_abi_version());
    }

    [Fact]
    public void Public_entry_points_pass_the_abi_check()
    {
        // Engine construction and the Version getter both route through
        // NativeInit — reaching here without TypeInitializationException
        // is the assert's happy path.
        Assert.False(string.IsNullOrEmpty(Engine.Version));
        using var engine = new Engine();
        Assert.Equal("3", engine.Apply("""{"+":[1,2]}""", "{}"));
    }
}

public class DataHandleTests
{
    [Fact]
    public void Rule_evaluates_against_data_handle()
    {
        using var engine = new Engine();
        using var rule = engine.Compile("""{"*":[{"var":"x"},3]}""");
        using var data = DataHandle.Parse("""{"x":14}""");

        Assert.Equal("42", rule.Evaluate(data));
        Assert.Equal(42, rule.EvaluateJson(data)!.GetValue<int>());
    }

    [Fact]
    public void Session_evaluates_against_data_handle()
    {
        using var engine = new Engine();
        using var rule = engine.Compile("""{"+":[{"var":"a"},{"var":"b"}]}""");
        using var session = engine.OpenSession();
        using var data = DataHandle.Parse("""{"a":40,"b":2}""");

        // Handles are not consumed by evaluation — reuse across calls.
        Assert.Equal("42", session.Evaluate(rule, data));
        Assert.Equal("42", session.Evaluate(rule, data));
        Assert.Equal(42, session.EvaluateJson(rule, data)!.GetValue<int>());
    }

    [Fact]
    public void Parse_error_throws_ParseException()
    {
        var ex = Assert.Throws<ParseException>(() => DataHandle.Parse("not-json{{"));
        Assert.Equal("ParseError", ex.ErrorType);
        Assert.Equal(EvaluationStatus.ParseError, ex.Status);
        Assert.False(string.IsNullOrEmpty(ex.Message));
    }

    [Fact]
    public void AllocatedBytes_reports_arena_usage()
    {
        using var data = DataHandle.Parse("""{"xs":[1,2,3,4,5,6,7,8]}""");
        Assert.True(data.AllocatedBytes > 0);
    }

    [Fact]
    public void Disposed_handle_throws_ObjectDisposedException()
    {
        using var engine = new Engine();
        using var rule = engine.Compile("""{"var":"x"}""");
        var data = DataHandle.Parse("""{"x":1}""");
        data.Dispose();
        Assert.Throws<ObjectDisposedException>(() => rule.Evaluate(data));
    }

    [Fact]
    public void Handle_is_engine_independent()
    {
        // One parsed document can feed rules compiled by different
        // engines.
        using var data = DataHandle.Parse("""{"x":21}""");
        using var engine1 = new Engine();
        using var engine2 = Engine.Builder().SetConfigJson("""{"preset":"strict"}""").Build();
        using var rule1 = engine1.Compile("""{"*":[{"var":"x"},2]}""");
        using var rule2 = engine2.Compile("""{"+":[{"var":"x"},21]}""");

        Assert.Equal("42", rule1.Evaluate(data));
        Assert.Equal("42", rule2.Evaluate(data));
    }
}

public class TypedEvaluationTests
{
    [Fact]
    public void EvaluateBool_returns_strict_boolean()
    {
        using var engine = new Engine();
        using var rule = engine.Compile("""{">":[{"var":"age"},18]}""");
        using var session = engine.OpenSession();
        using var adult = DataHandle.Parse("""{"age":25}""");
        using var minor = DataHandle.Parse("""{"age":12}""");

        Assert.True(session.EvaluateBool(rule, adult));
        Assert.False(session.EvaluateBool(rule, minor));
    }

    [Fact]
    public void EvaluateBool_throws_TypeMismatch_for_non_boolean_result()
    {
        using var engine = new Engine();
        using var rule = engine.Compile("""{"+":[1,2]}""");
        using var session = engine.OpenSession();
        using var data = DataHandle.Parse("{}");

        var ex = Assert.Throws<EvaluateException>(() => session.EvaluateBool(rule, data));
        Assert.Equal(EvaluationStatus.TypeMismatch, ex.Status);
        Assert.Equal("TypeMismatch", ex.ErrorType);
        Assert.Contains("boolean", ex.Message);
    }

    [Fact]
    public void EvaluateInt64_returns_exact_integer()
    {
        using var engine = new Engine();
        using var rule = engine.Compile("""{"+":[{"var":"x"},2]}""");
        using var session = engine.OpenSession();
        using var data = DataHandle.Parse("""{"x":40}""");

        Assert.Equal(42L, session.EvaluateInt64(rule, data));
    }

    [Fact]
    public void EvaluateInt64_throws_TypeMismatch_for_fractional_result()
    {
        using var engine = new Engine();
        using var rule = engine.Compile("""{"+":[1.5,1]}""");
        using var session = engine.OpenSession();
        using var data = DataHandle.Parse("{}");

        var ex = Assert.Throws<EvaluateException>(() => session.EvaluateInt64(rule, data));
        Assert.Equal(EvaluationStatus.TypeMismatch, ex.Status);
        Assert.Equal("TypeMismatch", ex.ErrorType);
    }

    [Fact]
    public void EvaluateDouble_accepts_any_number()
    {
        using var engine = new Engine();
        using var rule = engine.Compile("""{"*":[{"var":"x"},2]}""");
        using var session = engine.OpenSession();
        using var fractional = DataHandle.Parse("""{"x":1.25}""");
        using var integral = DataHandle.Parse("""{"x":21}""");

        Assert.Equal(2.5, session.EvaluateDouble(rule, fractional));
        Assert.Equal(42.0, session.EvaluateDouble(rule, integral));
    }

    [Fact]
    public void EvaluateDouble_throws_TypeMismatch_for_string_result()
    {
        using var engine = new Engine();
        using var rule = engine.Compile("""{"cat":["a","b"]}""");
        using var session = engine.OpenSession();
        using var data = DataHandle.Parse("{}");

        var ex = Assert.Throws<EvaluateException>(() => session.EvaluateDouble(rule, data));
        Assert.Equal(EvaluationStatus.TypeMismatch, ex.Status);
    }

    [Fact]
    public void EvaluateTruthy_coerces_and_never_mismatches()
    {
        using var engine = new Engine();
        using var rule = engine.Compile("""{"var":"v"}""");
        using var session = engine.OpenSession();
        using var truthyString = DataHandle.Parse("""{"v":"abc"}""");
        using var emptyString = DataHandle.Parse("""{"v":""}""");
        using var zero = DataHandle.Parse("""{"v":0}""");
        using var number = DataHandle.Parse("""{"v":7}""");

        Assert.True(session.EvaluateTruthy(rule, truthyString));
        Assert.False(session.EvaluateTruthy(rule, emptyString));
        Assert.False(session.EvaluateTruthy(rule, zero));
        Assert.True(session.EvaluateTruthy(rule, number));
    }
}

public class BatchEvaluationTests
{
    [Fact]
    public void EvaluateBatch_reports_per_item_success_and_failure()
    {
        using var engine = new Engine();
        using var rule = engine.Compile(
            """{"if":[{"var":"boom"},{"throw":"kaboom"},{"var":"x"}]}""");
        using var session = engine.OpenSession();
        using var d0 = DataHandle.Parse("""{"x":1}""");
        using var d1 = DataHandle.Parse("""{"boom":true}""");
        using var d2 = DataHandle.Parse("""{"x":3}""");

        var results = session.EvaluateBatch(rule, new[] { d0, d1, d2 });

        Assert.Equal(3, results.Length);

        Assert.True(results[0].IsSuccess);
        Assert.Equal(EvaluationStatus.Ok, results[0].Status);
        Assert.Equal("1", results[0].Json);
        Assert.Equal("1", results[0].Value);

        Assert.False(results[1].IsSuccess);
        Assert.Equal(EvaluationStatus.EvaluationError, results[1].Status);
        Assert.Equal("Thrown", results[1].ErrorTag);
        Assert.Contains("kaboom", results[1].ErrorMessage);
        Assert.Null(results[1].Json);
        var ex = Assert.Throws<EvaluateException>(() => results[1].Value);
        Assert.Equal("Thrown", ex.ErrorType);

        Assert.True(results[2].IsSuccess);
        Assert.Equal("3", results[2].Value);
    }

    [Fact]
    public void EvaluateBatch_empty_input_returns_empty()
    {
        using var engine = new Engine();
        using var rule = engine.Compile("""{"var":"x"}""");
        using var session = engine.OpenSession();

        Assert.Empty(session.EvaluateBatch(rule, Array.Empty<DataHandle>()));
    }

    [Fact]
    public void EvaluateBatch_handles_larger_result_sets()
    {
        // Enough items to force the session's shared result buffer to
        // grow mid-batch — verifies the copied-out slices stay intact.
        using var engine = new Engine();
        using var rule = engine.Compile("""{"cat":["item-",{"var":"i"}]}""");
        using var session = engine.OpenSession();

        const int n = 64;
        var handles = new List<DataHandle>(n);
        try
        {
            for (var i = 0; i < n; i++)
            {
                handles.Add(DataHandle.Parse($$"""{"i":{{i}}}"""));
            }
            var results = session.EvaluateBatch(rule, handles);
            Assert.Equal(n, results.Length);
            for (var i = 0; i < n; i++)
            {
                Assert.True(results[i].IsSuccess);
                Assert.Equal($"\"item-{i}\"", results[i].Json);
            }
        }
        finally
        {
            foreach (var h in handles) h.Dispose();
        }
    }

    [Fact]
    public void EvaluateMany_reports_per_rule_results()
    {
        using var engine = new Engine();
        using var r0 = engine.Compile("""{"+":[1,1]}""");
        using var r1 = engine.Compile("""{"throw":"nope"}""");
        using var r2 = engine.Compile("""{"var":"x"}""");
        using var session = engine.OpenSession();
        using var data = DataHandle.Parse("""{"x":7}""");

        var results = session.EvaluateMany(new[] { r0, r1, r2 }, data);

        Assert.Equal(3, results.Length);
        Assert.Equal("2", results[0].Value);

        Assert.False(results[1].IsSuccess);
        Assert.Equal(EvaluationStatus.EvaluationError, results[1].Status);
        Assert.Equal("Thrown", results[1].ErrorTag);
        Assert.Contains("nope", results[1].ErrorMessage);

        Assert.Equal("7", results[2].Value);
    }

    [Fact]
    public void EvaluateMany_empty_input_returns_empty()
    {
        using var engine = new Engine();
        using var session = engine.OpenSession();
        using var data = DataHandle.Parse("{}");

        Assert.Empty(session.EvaluateMany(Array.Empty<Rule>(), data));
    }
}

public class ErrorMappingTests
{
    [Fact]
    public void Session_rejects_rule_from_another_engine()
    {
        using var engineA = new Engine();
        using var engineB = new Engine();
        using var foreignRule = engineB.Compile("""{"var":"x"}""");
        using var session = engineA.OpenSession();

        var ex = Assert.Throws<EvaluateException>(() => session.Evaluate(foreignRule, "{}"));
        Assert.Equal(EvaluationStatus.InvalidArgument, ex.Status);
        Assert.Equal("InvalidArgument", ex.ErrorType);
        Assert.Contains("different engine", ex.Message);
    }

    [Fact]
    public void Evaluation_error_carries_tag_operator_and_path()
    {
        using var engine = new Engine();
        // Data-dependent so the optimizer can't const-fold the failure
        // at compile time — the NaN must surface from the "+" at runtime.
        using var rule = engine.Compile("""{"+":[{"var":"s"},1]}""");

        var ex = Assert.Throws<EvaluateException>(() => rule.Evaluate("""{"s":"abc"}"""));
        Assert.Equal(EvaluationStatus.EvaluationError, ex.Status);
        // NaN arithmetic surfaces as the engine's canonical thrown
        // error object {"type":"NaN"} — tag "Thrown".
        Assert.Equal("Thrown", ex.ErrorType);
        Assert.Contains("NaN", ex.Message);
        Assert.Equal("+", ex.Operator);
        Assert.NotNull(ex.PathJson);
        Assert.StartsWith("[", ex.PathJson);
    }

    [Fact]
    public void Parse_error_carries_status()
    {
        using var engine = new Engine();
        var ex = Assert.Throws<ParseException>(() => engine.Compile("not-json{{"));
        Assert.Equal(EvaluationStatus.ParseError, ex.Status);
        Assert.Equal("ParseError", ex.ErrorType);
    }

    [Fact]
    public void Session_string_results_are_independent_copies()
    {
        using var engine = new Engine();
        using var rule = engine.Compile("""{"var":"s"}""");
        using var session = engine.OpenSession();

        var first = session.Evaluate(rule, """{"s":"first"}""");
        var second = session.Evaluate(rule, """{"s":"second"}""");

        // The native result is borrowed from a reused session buffer;
        // the binding must have copied it out before the second call.
        Assert.Equal("\"first\"", first);
        Assert.Equal("\"second\"", second);
    }
}
