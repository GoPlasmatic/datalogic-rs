package datalogic

import (
	"encoding/json"
	"fmt"
	"strings"
	"sync"
	"testing"
)

func TestVersion(t *testing.T) {
	v := Version()
	if v == "" {
		t.Fatal("Version() returned empty string")
	}
}

func TestApplyOneShot(t *testing.T) {
	got, err := Apply(`{"+":[1,2]}`, `{}`)
	if err != nil {
		t.Fatal(err)
	}
	if got != "3" {
		t.Fatalf("want %q, got %q", "3", got)
	}
}

func TestCompileAndEvaluate(t *testing.T) {
	e := NewEngine()
	defer e.Close()

	rule, err := e.Compile(`{"var":"x"}`)
	if err != nil {
		t.Fatal(err)
	}
	defer rule.Close()

	cases := []struct{ data, want string }{
		{`{"x":1}`, "1"},
		{`{"x":42}`, "42"},
		{`{"x":"hi"}`, `"hi"`},
		{`{"x":[1,2,3]}`, `[1,2,3]`},
	}
	for _, c := range cases {
		got, err := rule.Evaluate(c.data)
		if err != nil {
			t.Fatalf("data=%s: %v", c.data, err)
		}
		if got != c.want {
			t.Errorf("data=%s: want %q, got %q", c.data, c.want, got)
		}
	}
}

func TestSessionArenaReuse(t *testing.T) {
	e := NewEngine()
	defer e.Close()

	rule, err := e.Compile(`{"*":[{"var":"x"},2]}`)
	if err != nil {
		t.Fatal(err)
	}
	defer rule.Close()

	s := e.Session()
	defer s.Close()

	for x := 1; x <= 5; x++ {
		data := fmt.Sprintf(`{"x":%d}`, x)
		got, err := s.Evaluate(rule, data)
		if err != nil {
			t.Fatal(err)
		}
		want := fmt.Sprintf("%d", x*2)
		if got != want {
			t.Errorf("x=%d: want %q, got %q", x, want, got)
		}
	}

	if s.AllocatedBytes() == 0 {
		t.Error("expected non-zero arena allocation after evaluations")
	}
	s.Reset()
}

func TestParseError(t *testing.T) {
	e := NewEngine()
	defer e.Close()

	_, err := e.Compile("definitely-not-json{{")
	if err == nil {
		t.Fatal("expected parse error, got nil")
	}
	derr, ok := err.(*Error)
	if !ok {
		t.Fatalf("want *Error, got %T (%v)", err, err)
	}
	if derr.Type != "ParseError" {
		t.Errorf("want Type=ParseError, got %q", derr.Type)
	}
	if derr.Message == "" {
		t.Error("expected non-empty error message")
	}
}

func TestEvaluateErrorCarriesPath(t *testing.T) {
	e := NewEngine()
	defer e.Close()

	rule, err := e.Compile(`{"throw":"boom"}`)
	if err != nil {
		t.Fatal(err)
	}
	defer rule.Close()

	_, err = rule.Evaluate(`{}`)
	if err == nil {
		t.Fatal("expected error from throw, got nil")
	}
	derr := err.(*Error)
	if derr.Type != "Thrown" {
		t.Errorf("want Type=Thrown, got %q", derr.Type)
	}
	if !strings.HasPrefix(derr.PathJSON, "[") {
		t.Errorf("want PathJSON to be a JSON array, got %q", derr.PathJSON)
	}
}

// Rule is documented as goroutine-safe. Exercise that lightly to catch
// race conditions / shared-state regressions. Each goroutine compiles
// its own deterministic data input and verifies the result.
func TestRuleEvaluateConcurrent(t *testing.T) {
	e := NewEngine()
	defer e.Close()

	rule, err := e.Compile(`{"*":[{"var":"x"},10]}`)
	if err != nil {
		t.Fatal(err)
	}
	defer rule.Close()

	const goroutines = 16
	const iters = 50
	var wg sync.WaitGroup
	errs := make(chan error, goroutines*iters)

	for g := 0; g < goroutines; g++ {
		wg.Add(1)
		go func(base int) {
			defer wg.Done()
			for i := 0; i < iters; i++ {
				x := base*iters + i
				got, err := rule.Evaluate(fmt.Sprintf(`{"x":%d}`, x))
				if err != nil {
					errs <- err
					return
				}
				want := fmt.Sprintf("%d", x*10)
				if got != want {
					errs <- fmt.Errorf("x=%d: want %q, got %q", x, want, got)
					return
				}
			}
		}(g)
	}
	wg.Wait()
	close(errs)
	for err := range errs {
		t.Error(err)
	}
}

func TestTracedSessionEvaluate(t *testing.T) {
	e := NewEngine()
	defer e.Close()

	ts := e.TracedSession()
	defer ts.Close()

	out, err := ts.Evaluate(`{"+":[{"var":"x"},1]}`, `{"x":41}`)
	if err != nil {
		t.Fatal(err)
	}

	var envelope struct {
		Result         json.RawMessage   `json:"result"`
		ExpressionTree json.RawMessage   `json:"expression_tree"`
		Steps          []json.RawMessage `json:"steps"`
		Error          *string           `json:"error"`
	}
	if err := json.Unmarshal([]byte(out), &envelope); err != nil {
		t.Fatalf("trace envelope is not JSON: %v\n%s", err, out)
	}
	if string(envelope.Result) != "42" {
		t.Errorf("want result 42, got %s", envelope.Result)
	}
	if len(envelope.Steps) == 0 {
		t.Error("expected non-empty steps for a non-trivial rule")
	}
	if len(envelope.ExpressionTree) == 0 {
		t.Error("expected an expression_tree node")
	}
	if envelope.Error != nil {
		t.Errorf("unexpected error in envelope: %s", *envelope.Error)
	}
}

func TestTracedSessionSurfacesEngineErrorInEnvelope(t *testing.T) {
	e := NewEngine()
	defer e.Close()

	ts := e.TracedSession()
	defer ts.Close()

	// Engine errors land inside the envelope, not in the Go error return.
	out, err := ts.Evaluate(`{"throw":"boom"}`, `{}`)
	if err != nil {
		t.Fatalf("engine errors should not surface as Go errors, got %v", err)
	}

	var envelope struct {
		Result          json.RawMessage `json:"result"`
		Error           *string         `json:"error"`
		StructuredError json.RawMessage `json:"structured_error"`
	}
	if err := json.Unmarshal([]byte(out), &envelope); err != nil {
		t.Fatalf("trace envelope is not JSON: %v\n%s", err, out)
	}
	if string(envelope.Result) != "null" {
		t.Errorf("want null result on error, got %s", envelope.Result)
	}
	if envelope.Error == nil || *envelope.Error == "" {
		t.Error("expected non-empty envelope error message")
	}
	if len(envelope.StructuredError) == 0 {
		t.Error("expected structured_error in envelope")
	}
}

func TestTemplatingEngine(t *testing.T) {
	e := NewTemplatingEngine()
	defer e.Close()

	// In templating mode a multi-key object becomes an output-shaping
	// template — the keys are emitted literally, values are evaluated.
	rule, err := e.Compile(`{"sum": {"+":[1,2]}, "label": "static"}`)
	if err != nil {
		t.Fatal(err)
	}
	defer rule.Close()

	got, err := rule.Evaluate(`{}`)
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(got, `"sum":3`) || !strings.Contains(got, `"label":"static"`) {
		t.Errorf("template output unexpected: %s", got)
	}
}
