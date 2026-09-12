// SPDX-License-Identifier: Apache-2.0

using System.Linq;

using Xunit;

using Goplasmatic.Datalogic;

namespace Goplasmatic.Datalogic.Tests;

/// <summary>
/// The operation budget reaches this binding through the config wire
/// format alone — there is no per-call budget entry point across the C
/// ABI — so these pin that the key is accepted, that it actually bounds
/// evaluation, and that the failure carries its own error type.
/// </summary>
public class BudgetTests
{
    private const string Rule = """{"map":[{"var":"xs"},{"*":[{"var":""},2]}]}""";

    /// <summary><c>{"xs":[0,1,...,n-1]}</c></summary>
    private static string Items(int n) =>
        $$"""{"xs":[{{string.Join(",", Enumerable.Range(0, n))}}]}""";

    [Fact]
    public void Ops_budget_bounds_evaluation()
    {
        // 100 is comfortably above the three-item run and comfortably
        // below the 200-item one.
        using var engine = Engine.Builder().SetConfigJson("""{"ops_budget":100}""").Build();
        Assert.Equal("[0,2,4]", engine.Apply(Rule, Items(3)));

        var tooBig = Assert.Throws<EvaluateException>(() => engine.Apply(Rule, Items(200)));
        Assert.Equal("BudgetExceeded", tooBig.ErrorType);
        Assert.Contains("budget", tooBig.Message);
    }

    [Fact]
    public void Try_cannot_recover_from_an_exhausted_budget()
    {
        using var engine = Engine.Builder().SetConfigJson("""{"ops_budget":10}""").Build();
        var ex = Assert.Throws<EvaluateException>(
            () => engine.Apply($$"""{"try":[{{Rule}},"fallback"]}""", Items(200)));
        Assert.Equal("BudgetExceeded", ex.ErrorType);
    }

    [Fact]
    public void A_null_budget_is_unbounded()
    {
        using var engine = Engine.Builder().SetConfigJson("""{"ops_budget":null}""").Build();
        Assert.StartsWith("[0,2,4,", engine.Apply(Rule, Items(200)));
    }

    [Theory]
    [InlineData("0")]
    [InlineData("-1")]
    [InlineData("\"many\"")]
    public void An_invalid_budget_is_a_configuration_error(string bad)
    {
        var ex = Assert.Throws<EvaluateException>(
            () => Engine.Builder().SetConfigJson($$"""{"ops_budget":{{bad}}}"""));
        Assert.Equal("ConfigurationError", ex.ErrorType);
    }

    [Fact]
    public void Tensor_operators_are_priced_by_the_elements_they_move()
    {
        // `zeros` allocates 256 elements from a three-node rule: the node
        // count alone would price this at nothing.
        using var engine = Engine.Builder().SetConfigJson("""{"ops_budget":100}""").Build();
        var ex = Assert.Throws<EvaluateException>(
            () => engine.Apply("""{"zeros":[[16,16],"f32"]}""", "{}"));
        Assert.Equal("BudgetExceeded", ex.ErrorType);
    }

    /// <summary>
    /// The tensor family crosses this binding as JSON like any other
    /// value — the tagged form — so no P/Invoke change was needed for it.
    /// </summary>
    [Fact]
    public void Tensor_round_trips_as_tagged_json()
    {
        using var engine = new Engine();
        var emitted = engine.Apply("""{"tensor":[[1,2,3],"u8"]}""", "{}");
        Assert.Equal("""{"tensor":{"dtype":"u8","shape":[3],"data":"AQID"}}""", emitted);
        // And the emitted form is accepted back as a rule.
        Assert.Equal("[1,2,3]", engine.Apply($$"""{"to_list":[{{emitted}}]}""", "{}"));
    }
}
