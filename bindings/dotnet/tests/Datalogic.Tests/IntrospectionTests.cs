// SPDX-License-Identifier: Apache-2.0
//
// The ABI v2.1 surface: check, compile modes, operators, facts, truthy,
// metered sessions, builder escape and strict names, error node ids.

using System.Text.Json.Nodes;
using Xunit;

using Goplasmatic.Datalogic;

namespace Goplasmatic.Datalogic.Tests;

public class IntrospectionTests
{
    [Fact]
    public void Check_reports_problems_with_pointers()
    {
        using var e = new Engine();
        var diags = JsonNode.Parse(e.Check("""{"if": [true, {"vr": "x"}, {"map": [1]}]}"""))!.AsArray();
        Assert.Equal("UnknownOperator", (string?)diags[0]!["code"]);
        Assert.Equal("/if/1", (string?)diags[0]!["pointer"]);
        Assert.Equal("ArgumentCount", (string?)diags[1]!["code"]);
        Assert.Equal("[]", e.Check("""{"a": {"var": "x"}, "b": 1}""", CompileMode.Template));
    }

    [Fact]
    public void CompileChecked_carries_diagnostics()
    {
        using var e = new Engine();
        var ex = Assert.Throws<ParseException>(() => e.CompileChecked("""{"if": [{"bogus": 1}, {"map": [1]}]}"""));
        Assert.Equal("CompileError", ex.ErrorType);
        Assert.Equal(2, JsonNode.Parse(ex.DiagnosticsJson!)!.AsArray().Count);
        using var r = e.CompileChecked("""{"+": [1, {"var": "x"}]}""");
        Assert.Equal("3", r.Evaluate("""{"x": 2}"""));
    }

    [Fact]
    public void Compile_modes_and_facts()
    {
        using var e = new Engine();
        const string tpl = """{"user": {"var": "name"}, "n": 1}""";
        Assert.ThrowsAny<DatalogicException>(() => e.Compile(tpl));
        using var r = e.CompileTemplate(tpl);
        Assert.Equal("""{"user":"ana","n":1}""", r.Evaluate("""{"name": "ana"}"""));
        Assert.Equal("""[["name"]]""", JsonNode.Parse(r.Facts())!["reads"]!.ToJsonString());
        Assert.ThrowsAny<DatalogicException>(() => e.CompileStrict(tpl));
    }

    [Fact]
    public void Operators_is_the_documented_catalogue_and_truthy_follows_the_engine()
    {
        using var e = new Engine();
        var docs = JsonNode.Parse(File.ReadAllText(Path.Combine(RepoRoot(), "docs/src/operators/operators.json")));
        Assert.True(JsonNode.DeepEquals(docs, JsonNode.Parse(e.Operators())));
        Assert.False(e.Truthy("{}"));
        Assert.True(e.Truthy("""{"a": 1}"""));
    }

    [Fact]
    public void Metered_session()
    {
        using var e = new Engine();
        using var r = e.Compile("""{"map": [{"var": "xs"}, {"+": [{"var": ""}, 1]}]}""");
        using var s = e.OpenSession();
        var m = s.EvaluateMetered(r, """{"xs": [1, 2, 3]}""");
        Assert.Equal("[2,3,4]", m.Value);
        Assert.True(m.Ops > 0);
        var ex = Assert.Throws<EvaluateException>(() => s.EvaluateMetered(r, """{"xs": [1, 2, 3]}""", 2));
        Assert.Equal("BudgetExceeded", ex.ErrorType);
    }

    [Fact]
    public void Builder_escape_and_strict_names()
    {
        CustomOperator one = _ => "1";
        var ex = Assert.ThrowsAny<DatalogicException>(
            () => Engine.Builder().WithStrictOperatorNames(true).AddOperator("length", one));
        Assert.Equal("ConfigurationError", ex.ErrorType);
        using var e = Engine.Builder().WithTemplating(true).WithTemplateKeyEscape('$')
            .WithStrictOperatorNames(true).AddOperator("uno", one).Build();
        Assert.Equal("""{"type":1,"k":2}""", e.Apply("""{"$type": {"uno": []}, "k": 2}""", "null"));
    }

    [Fact]
    public void Escape_outside_the_bmp()
    {
        foreach (var escape in new[] { "😀", "€" })
        {
            using var e = Engine.Builder().WithTemplating(true).WithTemplateKeyEscape(escape).Build();
            Assert.Equal("""{"type":1,"k":2}""", e.Apply("{\"" + escape + "type\": 1, \"k\": 2}", "null"));
        }
        using var r = Engine.Builder().WithTemplating(true)
            .WithTemplateKeyEscape(new System.Text.Rune(0x1F600)).Build();
        Assert.Equal("""{"a":1}""", r.Apply("{\"\U0001F600a\": 1}", "null"));
        foreach (var bad in new[] { "", "ab", "\uD83D" })
        {
            Assert.Throws<ArgumentException>(() => Engine.Builder().WithTemplateKeyEscape(bad));
        }
    }

    [Fact]
    public void Errors_carry_node_ids()
    {
        using var e = new Engine();
        var ex = Assert.Throws<EvaluateException>(() => e.Apply("""{"+": ["a", 1]}""", "null"));
        Assert.NotNull(ex.NodeIdsJson);
    }

    private static string RepoRoot()
    {
        var dir = new DirectoryInfo(AppContext.BaseDirectory);
        while (dir != null && !File.Exists(Path.Combine(dir.FullName, "Cargo.toml")))
        {
            dir = dir.Parent;
        }
        return dir!.FullName;
    }
}
