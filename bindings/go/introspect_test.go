package datalogic

import (
	"encoding/json"
	"os"
	"reflect"
	"strings"
	"testing"
)

func TestCheckAndCompileChecked(t *testing.T) {
	e := NewEngine()
	defer e.Close()
	out, err := e.Check(`{"if": [true, {"vr": "x"}, {"map": [1]}]}`, ModeEngine)
	if err != nil {
		t.Fatal(err)
	}
	var diags []map[string]any
	if err := json.Unmarshal([]byte(out), &diags); err != nil {
		t.Fatal(err)
	}
	if len(diags) != 2 || diags[0]["code"] != "UnknownOperator" || diags[0]["pointer"] != "/if/1" {
		t.Fatalf("diagnostics %v", diags)
	}
	_, err = e.CompileChecked(`{"if": [{"bogus": 1}, {"map": [1]}]}`)
	derr, ok := err.(*Error)
	if !ok || derr.Type != "CompileError" || !strings.Contains(derr.DiagnosticsJSON, "UnknownOperator") {
		t.Fatalf("CompileChecked error %#v", err)
	}
	r, err := e.CompileChecked(`{"+": [1, {"var": "x"}]}`)
	if err != nil {
		t.Fatal(err)
	}
	if got, _ := r.Evaluate(`{"x": 2}`); got != "3" {
		t.Fatalf("got %s", got)
	}
}

func TestCompileModesAndFacts(t *testing.T) {
	e := NewEngine()
	defer e.Close()
	tpl := `{"user": {"var": "name"}, "n": 1}`
	if _, err := e.Compile(tpl); err == nil {
		t.Fatal("strict compile accepted a template")
	}
	r, err := e.CompileTemplate(tpl)
	if err != nil {
		t.Fatal(err)
	}
	if got, _ := r.Evaluate(`{"name": "ana"}`); got != `{"user":"ana","n":1}` {
		t.Fatalf("got %s", got)
	}
	facts, err := r.Facts()
	if err != nil || !strings.Contains(facts, `"reads":[["name"]]`) {
		t.Fatalf("facts %s %v", facts, err)
	}
	if _, err := NewTemplatingEngine().CompileStrict(tpl); err == nil {
		t.Fatal("CompileStrict accepted a template")
	}
}

func TestOperatorsAndTruthy(t *testing.T) {
	e := NewEngine()
	defer e.Close()
	got, err := e.Operators()
	if err != nil {
		t.Fatal(err)
	}
	docs, err := os.ReadFile("../../docs/src/operators/operators.json")
	if err != nil {
		t.Fatal(err)
	}
	var a, b any
	json.Unmarshal([]byte(got), &a)
	json.Unmarshal(docs, &b)
	if !reflect.DeepEqual(a, b) {
		t.Fatal("Operators() differs from operators.json")
	}
	for value, want := range map[string]bool{`{}`: false, `[]`: false, `{"a":1}`: true, `"0"`: true} {
		if got, err := e.Truthy(value); err != nil || got != want {
			t.Fatalf("Truthy(%s) = %v, %v", value, got, err)
		}
	}
}

func TestEvaluateMetered(t *testing.T) {
	e := NewEngine()
	defer e.Close()
	r, _ := e.Compile(`{"map": [{"var": "xs"}, {"+": [{"var": ""}, 1]}]}`)
	s := e.Session()
	defer s.Close()
	out, ops, err := s.EvaluateMetered(r, `{"xs": [1, 2, 3]}`, 0)
	if err != nil || out != "[2,3,4]" || ops == 0 {
		t.Fatalf("%s %d %v", out, ops, err)
	}
	_, _, err = s.EvaluateMetered(r, `{"xs": [1, 2, 3]}`, 2)
	if derr, ok := err.(*Error); !ok || derr.Type != "BudgetExceeded" {
		t.Fatalf("budget error %#v", err)
	}
}

func TestBuilderEscapeAndStrictNames(t *testing.T) {
	one := func(string) (string, error) { return "1", nil }
	_, err := NewEngineBuilder().StrictOperatorNames(true).AddOperator("length", one).Build()
	if derr, ok := err.(*Error); !ok || derr.Type != "ConfigurationError" {
		t.Fatalf("strict names error %#v", err)
	}
	e, err := NewEngineBuilder().Templating(true).TemplateKeyEscape('$').
		StrictOperatorNames(true).AddOperator("uno", one).Build()
	if err != nil {
		t.Fatal(err)
	}
	defer e.Close()
	if got, _ := e.Apply(`{"$type": {"uno": []}, "k": 2}`, `null`); got != `{"type":1,"k":2}` {
		t.Fatalf("got %s", got)
	}
}

func TestErrorNodeIDs(t *testing.T) {
	e := NewEngine()
	defer e.Close()
	_, err := e.Apply(`{"+": ["a", 1]}`, `null`)
	if derr, ok := err.(*Error); !ok || derr.NodeIDsJSON == "" {
		t.Fatalf("node ids %#v", err)
	}
}
