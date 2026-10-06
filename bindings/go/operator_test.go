package datalogic

import (
	"encoding/json"
	"errors"
	"runtime"
	"runtime/cgo"
	"runtime/debug"
	"strings"
	"testing"
	"time"
)

func TestBuilderNoCustomOps(t *testing.T) {
	e, err := NewEngineBuilder().Build()
	if err != nil {
		t.Fatal(err)
	}
	defer e.Close()
	got, err := e.Apply(`{"+":[1,2]}`, `{}`)
	if err != nil {
		t.Fatal(err)
	}
	if got != "3" {
		t.Fatalf("want 3, got %q", got)
	}
}

func TestBuilderScalarCustomOp(t *testing.T) {
	e, err := NewEngineBuilder().AddOperator("double", func(argsJSON string) (string, error) {
		var args []float64
		if err := json.Unmarshal([]byte(argsJSON), &args); err != nil {
			return "", err
		}
		out, _ := json.Marshal(args[0] * 2)
		return string(out), nil
	}).Build()
	if err != nil {
		t.Fatal(err)
	}
	defer e.Close()

	got, err := e.Apply(`{"double":[21]}`, `{}`)
	if err != nil {
		t.Fatal(err)
	}
	if got != "42" {
		t.Fatalf("want 42, got %q", got)
	}
}

func TestBuilderStringCustomOp(t *testing.T) {
	e, _ := NewEngineBuilder().AddOperator("upper", func(argsJSON string) (string, error) {
		var args []string
		if err := json.Unmarshal([]byte(argsJSON), &args); err != nil {
			return "", err
		}
		out, _ := json.Marshal(strings.ToUpper(args[0]))
		return string(out), nil
	}).Build()
	defer e.Close()

	got, err := e.Apply(`{"upper":["hello"]}`, `{}`)
	if err != nil {
		t.Fatal(err)
	}
	if got != `"HELLO"` {
		t.Fatalf("want \"HELLO\", got %q", got)
	}
}

func TestBuilderComposesWithBuiltins(t *testing.T) {
	e, _ := NewEngineBuilder().AddOperator("double", func(argsJSON string) (string, error) {
		var args []float64
		json.Unmarshal([]byte(argsJSON), &args)
		out, _ := json.Marshal(args[0] * 2)
		return string(out), nil
	}).Build()
	defer e.Close()

	got, err := e.Apply(`{"map":[{"var":"xs"},{"double":[{"var":""}]}]}`, `{"xs":[1,2,3]}`)
	if err != nil {
		t.Fatal(err)
	}
	if got != "[2,4,6]" {
		t.Fatalf("want [2,4,6], got %q", got)
	}
}

func TestBuilderMultipleArgsCustomOp(t *testing.T) {
	e, _ := NewEngineBuilder().AddOperator("clamp", func(argsJSON string) (string, error) {
		var args []float64
		if err := json.Unmarshal([]byte(argsJSON), &args); err != nil {
			return "", err
		}
		v, lo, hi := args[0], args[1], args[2]
		if v < lo {
			v = lo
		}
		if v > hi {
			v = hi
		}
		out, _ := json.Marshal(v)
		return string(out), nil
	}).Build()
	defer e.Close()

	for _, c := range []struct{ rule, want string }{
		{`{"clamp":[5,0,3]}`, "3"},
		{`{"clamp":[-5,0,3]}`, "0"},
		{`{"clamp":[2,0,3]}`, "2"},
	} {
		got, err := e.Apply(c.rule, `{}`)
		if err != nil {
			t.Fatalf("%s: %v", c.rule, err)
		}
		if got != c.want {
			t.Errorf("%s: want %q, got %q", c.rule, c.want, got)
		}
	}
}

func TestBuilderErrorPropagates(t *testing.T) {
	e, _ := NewEngineBuilder().AddOperator("boom", func(argsJSON string) (string, error) {
		return "", errors.New("custom-failure")
	}).Build()
	defer e.Close()

	_, err := e.Apply(`{"boom":[]}`, `{}`)
	if err == nil {
		t.Fatal("expected error, got nil")
	}
	if !strings.Contains(err.Error(), "custom-failure") {
		t.Errorf("want 'custom-failure' in error, got %q", err.Error())
	}
}

func TestBuilderInvalidJSONReturnPropagates(t *testing.T) {
	e, _ := NewEngineBuilder().AddOperator("bad", func(argsJSON string) (string, error) {
		return "this is not json", nil
	}).Build()
	defer e.Close()

	_, err := e.Apply(`{"bad":[]}`, `{}`)
	if err == nil {
		t.Fatal("expected error from invalid JSON return, got nil")
	}
}

func TestBuilderBuiltinWinsOverCustom(t *testing.T) {
	e, _ := NewEngineBuilder().AddOperator("+", func(argsJSON string) (string, error) {
		return `"hijacked"`, nil
	}).Build()
	defer e.Close()

	got, err := e.Apply(`{"+":[1,2]}`, `{}`)
	if err != nil {
		t.Fatal(err)
	}
	if got != "3" {
		t.Fatalf("built-in should still apply; got %q", got)
	}
}

func TestBuilderRuleSurvivesAcrossEvaluations(t *testing.T) {
	e, _ := NewEngineBuilder().AddOperator("add5", func(argsJSON string) (string, error) {
		var args []float64
		json.Unmarshal([]byte(argsJSON), &args)
		out, _ := json.Marshal(args[0] + 5)
		return string(out), nil
	}).Build()
	defer e.Close()

	rule, err := e.Compile(`{"add5":[{"var":"x"}]}`)
	if err != nil {
		t.Fatal(err)
	}
	defer rule.Close()

	for _, c := range []struct {
		x    int
		want string
	}{
		{10, "15"},
		{100, "105"},
		{0, "5"},
	} {
		got, err := rule.Evaluate(`{"x":` + itoa(c.x) + `}`)
		if err != nil {
			t.Fatal(err)
		}
		if got != c.want {
			t.Errorf("x=%d: want %q, got %q", c.x, c.want, got)
		}
	}
}

func TestBuilderSetConfigJSON(t *testing.T) {
	// Default config: null coerces to 0 and the sum evaluates.
	got, err := Apply(`{"+":[null,1]}`, `{}`)
	if err != nil {
		t.Fatal(err)
	}
	if got != "1" {
		t.Fatalf("default config: want 1, got %q", got)
	}

	// Strict preset: the same rule rejects the non-numeric null.
	b := NewEngineBuilder()
	if err := b.SetConfigJSON(`{"preset":"strict"}`); err != nil {
		t.Fatal(err)
	}
	e, err := b.Build()
	if err != nil {
		t.Fatal(err)
	}
	defer e.Close()

	_, err = e.Apply(`{"+":[null,1]}`, `{}`)
	if err == nil {
		t.Fatal("strict config should reject null operand, got nil error")
	}
}

func TestBuilderSetConfigJSONInvalid(t *testing.T) {
	b := NewEngineBuilder()

	// Malformed JSON surfaces the parser's message.
	err := b.SetConfigJSON("not-json{{")
	if err == nil {
		t.Fatal("expected error for malformed config JSON, got nil")
	}
	derr, ok := err.(*Error)
	if !ok {
		t.Fatalf("want *Error, got %T (%v)", err, err)
	}
	if derr.Message == "" {
		t.Error("expected non-empty error message")
	}

	// Unknown enum values fail loudly instead of being ignored.
	err = b.SetConfigJSON(`{"preset":"bogus"}`)
	if err == nil {
		t.Fatal("expected error for unknown preset, got nil")
	}
	if !strings.Contains(err.Error(), "bogus") {
		t.Errorf("want 'bogus' in error, got %q", err.Error())
	}

	// A failed SetConfigJSON leaves the builder usable.
	e, err := b.Build()
	if err != nil {
		t.Fatal(err)
	}
	defer e.Close()
	got, err := e.Apply(`{"+":[1,2]}`, `{}`)
	if err != nil {
		t.Fatal(err)
	}
	if got != "3" {
		t.Fatalf("want 3, got %q", got)
	}
}

func TestBuilderTemplating(t *testing.T) {
	e, err := NewEngineBuilder().Templating(true).Build()
	if err != nil {
		t.Fatal(err)
	}
	defer e.Close()

	got, err := e.Apply(`{"sum":{"+":[1,2]},"label":"static"}`, `{}`)
	if err != nil {
		t.Fatal(err)
	}
	if !strings.Contains(got, `"sum":3`) || !strings.Contains(got, `"label":"static"`) {
		t.Errorf("template output unexpected: %s", got)
	}
}

// tripleEngine builds an engine whose "triple" operator multiplies its
// first argument by three.
func tripleEngine(t *testing.T) *Engine {
	t.Helper()
	e, err := NewEngineBuilder().AddOperator("triple", func(argsJSON string) (string, error) {
		var args []float64
		if err := json.Unmarshal([]byte(argsJSON), &args); err != nil {
			return "", err
		}
		out, _ := json.Marshal(args[0] * 3)
		return string(out), nil
	}).Build()
	if err != nil {
		t.Fatal(err)
	}
	return e
}

// forceGC runs several collections, giving the finalizer goroutine a
// moment between them so finalizers queued by one cycle get to run
// (and free what they own) before the next.
func forceGC() {
	for i := 0; i < 5; i++ {
		runtime.GC()
		debug.FreeOSMemory()
		time.Sleep(time.Millisecond)
	}
}

// checkTripleSurvives evaluates the "triple" operator through a Rule, a
// Session and a TracedSession whose Engine is already gone.
func checkTripleSurvives(t *testing.T, rule *Rule, s *Session, ts *TracedSession) {
	t.Helper()
	for i := 0; i < 50; i++ {
		if i%10 == 0 {
			forceGC()
		}
		got, err := rule.Evaluate(`{"n":14}`)
		if err != nil || got != "42" {
			t.Fatalf("rule: want 42, got %q (err %v)", got, err)
		}
		got, err = s.Evaluate(rule, `{"n":14}`)
		if err != nil || got != "42" {
			t.Fatalf("session: want 42, got %q (err %v)", got, err)
		}
		env, err := ts.Evaluate(`{"triple":[{"var":"n"}]}`, `{"n":14}`)
		if err != nil {
			t.Fatal(err)
		}
		var trace struct {
			Result json.RawMessage `json:"result"`
		}
		if err := json.Unmarshal([]byte(env), &trace); err != nil || string(trace.Result) != "42" {
			t.Fatalf("traced session: want result 42, got %s", env)
		}
	}
}

// Rules, Sessions and TracedSessions hold their own reference on the
// native engine, so closing the Engine and letting it be collected must
// not free the Go callback state they dispatch into.
func TestCustomOperatorSurvivesEngineCloseAndGC(t *testing.T) {
	rule, s, ts := func() (*Rule, *Session, *TracedSession) {
		e := tripleEngine(t)
		rule, err := e.Compile(`{"triple":[{"var":"n"}]}`)
		if err != nil {
			t.Fatal(err)
		}
		s, ts := e.Session(), e.TracedSession()
		e.Close()
		return rule, s, ts
	}()
	defer rule.Close()
	defer s.Close()
	defer ts.Close()
	checkTripleSurvives(t, rule, s, ts)
}

// Same as above, but the Engine is never closed — its GC finalizer is
// what runs.
func TestCustomOperatorSurvivesEngineFinalizer(t *testing.T) {
	rule, s, ts := func() (*Rule, *Session, *TracedSession) {
		e := tripleEngine(t)
		rule, err := e.Compile(`{"triple":[{"var":"n"}]}`)
		if err != nil {
			t.Fatal(err)
		}
		return rule, e.Session(), e.TracedSession()
	}()
	defer rule.Close()
	defer s.Close()
	defer ts.Close()
	checkTripleSurvives(t, rule, s, ts)
}

// Rules from the mode and checked compile paths keep the registry too.
func TestCustomOperatorSurvivesEngineCloseForEveryCompilePath(t *testing.T) {
	rules := func() []*Rule {
		e := tripleEngine(t)
		defer e.Close()
		var out []*Rule
		for _, compile := range []func(string) (*Rule, error){
			e.Compile,
			e.CompileTemplate,
			e.CompileStrict,
			e.CompileChecked,
		} {
			r, err := compile(`{"triple":[{"var":"n"}]}`)
			if err != nil {
				t.Fatal(err)
			}
			out = append(out, r)
		}
		return out
	}()
	for i := 0; i < 30; i++ {
		if i%10 == 0 {
			forceGC()
		}
		for j, r := range rules {
			got, err := r.Evaluate(`{"n":14}`)
			if err != nil || got != "42" {
				t.Fatalf("rule %d: want 42, got %q (err %v)", j, got, err)
			}
		}
	}
	for _, r := range rules {
		r.Close()
	}
}

// handleLive reports whether h still resolves (Value panics once the
// handle is deleted).
func handleLive(h cgo.Handle) (live bool) {
	defer func() {
		if recover() != nil {
			live = false
		}
	}()
	h.Value()
	return true
}

// The callback handles are released once the Engine and everything
// derived from it are unreachable, not leaked.
func TestOperatorRegistryFreedAfterLastUser(t *testing.T) {
	h := func() cgo.Handle {
		e := tripleEngine(t)
		rule, err := e.Compile(`{"triple":[{"var":"n"}]}`)
		if err != nil {
			t.Fatal(err)
		}
		s := e.Session()
		h := e.reg.handles[0]
		e.Close()
		rule.Close()
		s.Close()
		return h
	}()
	for i := 0; i < 50 && handleLive(h); i++ {
		forceGC()
	}
	if handleLive(h) {
		t.Fatal("operator handle still live after every user was collected")
	}
}

// A failed Build releases the callback handles straight away.
func TestBuilderFailureReleasesHandles(t *testing.T) {
	b := NewEngineBuilder().AddOperator("\xff", func(string) (string, error) { return "1", nil })
	h := b.reg.handles[0]
	if _, err := b.Build(); err == nil {
		t.Fatal("want an error for a non-UTF-8 operator name")
	}
	if handleLive(h) {
		t.Fatal("handle still live after a failed Build")
	}
}

// itoa avoids fmt.Sprintf for trivial int formatting in hot test loops.
func itoa(n int) string {
	if n == 0 {
		return "0"
	}
	neg := n < 0
	if neg {
		n = -n
	}
	var buf [20]byte
	i := len(buf)
	for n > 0 {
		i--
		buf[i] = byte('0' + n%10)
		n /= 10
	}
	if neg {
		i--
		buf[i] = '-'
	}
	return string(buf[i:])
}
