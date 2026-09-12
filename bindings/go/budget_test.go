package datalogic

import (
	"encoding/json"
	"strconv"
	"strings"
	"testing"
)

// The operation budget reaches this binding through the config wire
// format alone — there is no per-call budget entry point across the C
// ABI — so these pin that the key is accepted, that it actually bounds
// evaluation, and that the failure carries its own tag.

const budgetRule = `{"map":[{"var":"xs"},{"*":[{"var":""},2]}]}`

func budgetEngine(t *testing.T, config string) *Engine {
	t.Helper()
	b := NewEngineBuilder()
	if err := b.SetConfigJSON(config); err != nil {
		t.Fatalf("config %s rejected: %v", config, err)
	}
	e, err := b.Build()
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(e.Close)
	return e
}

// items builds `{"xs":[0,1,...,n-1]}`.
func items(n int) string {
	parts := make([]string, n)
	for i := range parts {
		parts[i] = strconv.Itoa(i)
	}
	return `{"xs":[` + strings.Join(parts, ",") + `]}`
}

func TestOpsBudgetBoundsEvaluation(t *testing.T) {
	// 100 is comfortably above the three-item run and comfortably below
	// the 200-item one.
	e := budgetEngine(t, `{"ops_budget":100}`)

	got, err := e.Apply(budgetRule, items(3))
	if err != nil {
		t.Fatalf("a budget that fits should not refuse: %v", err)
	}
	if got != "[0,2,4]" {
		t.Fatalf("want [0,2,4], got %q", got)
	}

	_, err = e.Apply(budgetRule, items(200))
	if err == nil {
		t.Fatal("expected the 200-item payload to exceed a budget of 100")
	}
	derr, ok := err.(*Error)
	if !ok {
		t.Fatalf("want *Error, got %T", err)
	}
	if derr.Type != "BudgetExceeded" {
		t.Fatalf("want type BudgetExceeded, got %q (%s)", derr.Type, derr.Message)
	}
	if !strings.Contains(derr.Message, "budget") {
		t.Fatalf("message should name the budget: %q", derr.Message)
	}
}

func TestOpsBudgetSurvivesTry(t *testing.T) {
	e := budgetEngine(t, `{"ops_budget":10}`)
	rule := `{"try":[` + budgetRule + `,"fallback"]}`
	_, err := e.Apply(rule, items(200))
	if err == nil {
		t.Fatal("try must not recover from an exhausted budget")
	}
	if derr, ok := err.(*Error); !ok || derr.Type != "BudgetExceeded" {
		t.Fatalf("want BudgetExceeded, got %v", err)
	}
}

func TestOpsBudgetNullIsUnbounded(t *testing.T) {
	e := budgetEngine(t, `{"ops_budget":null}`)
	got, err := e.Apply(budgetRule, items(200))
	if err != nil {
		t.Fatalf("null budget should be unbounded: %v", err)
	}
	var out []int
	if err := json.Unmarshal([]byte(got), &out); err != nil {
		t.Fatal(err)
	}
	if len(out) != 200 {
		t.Fatalf("want 200 results, got %d", len(out))
	}
}

func TestOpsBudgetRejectsAnInvalidValue(t *testing.T) {
	b := NewEngineBuilder()
	defer func() {
		if e, err := b.Build(); err == nil {
			e.Close()
		}
	}()
	for _, bad := range []string{`{"ops_budget":0}`, `{"ops_budget":-1}`, `{"ops_budget":"many"}`} {
		err := b.SetConfigJSON(bad)
		if err == nil {
			t.Fatalf("config %s should be rejected", bad)
		}
		if derr, ok := err.(*Error); !ok || derr.Type != "ConfigurationError" {
			t.Fatalf("want ConfigurationError for %s, got %v", bad, err)
		}
	}
}

// The tensor family crosses this binding as JSON like any other value —
// the tagged `{"tensor": {...}}` form — so no ABI change was needed for
// it. This is the check that stays true.
func TestTensorRoundTripsAsTaggedJSON(t *testing.T) {
	got, err := Apply(`{"tensor":[[1,2,3],"u8"]}`, `{}`)
	if err != nil {
		t.Fatal(err)
	}
	want := `{"tensor":{"dtype":"u8","shape":[3],"data":"AQID"}}`
	if got != want {
		t.Fatalf("want %s, got %s", want, got)
	}

	// And the emitted form is accepted back as a rule.
	got, err = Apply(`{"to_list":[`+want+`]}`, `{}`)
	if err != nil {
		t.Fatal(err)
	}
	if got != "[1,2,3]" {
		t.Fatalf("want [1,2,3], got %s", got)
	}
}

func TestTensorOperatorsArePricedByElementsMoved(t *testing.T) {
	// `zeros` allocates 256 elements from a three-node rule: the node
	// count alone would price this at nothing.
	e := budgetEngine(t, `{"ops_budget":100}`)
	_, err := e.Apply(`{"zeros":[[16,16],"f32"]}`, `{}`)
	if err == nil {
		t.Fatal("a 256-element tensor should not fit a budget of 100")
	}
	if derr, ok := err.(*Error); !ok || derr.Type != "BudgetExceeded" {
		t.Fatalf("want BudgetExceeded, got %v", err)
	}
}
