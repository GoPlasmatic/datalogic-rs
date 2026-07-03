package datalogic

// Tests for the C ABI v2 additions: data handles (incl. concurrent
// use), typed session evaluations, batch evaluation with per-item
// failures, error-field fidelity, and (ptr, len) edge cases such as
// empty-string inputs.

import (
	"fmt"
	"strings"
	"sync"
	"testing"
)

func TestParseDataAndEvaluate(t *testing.T) {
	e := NewEngine()
	defer e.Close()

	rule, err := e.Compile(`{"*":[{"var":"x"},2]}`)
	if err != nil {
		t.Fatal(err)
	}
	defer rule.Close()

	data, err := ParseData(`{"x":21}`)
	if err != nil {
		t.Fatal(err)
	}
	defer data.Close()

	// Rule tier (owned result, pooled arena).
	got, err := rule.EvaluateData(data)
	if err != nil {
		t.Fatal(err)
	}
	if got != "42" {
		t.Errorf("Rule.EvaluateData: want 42, got %q", got)
	}

	// Session tier (borrowed result, copied out immediately). The
	// handle is not consumed — evaluate through it repeatedly.
	s := e.Session()
	defer s.Close()
	for i := 0; i < 3; i++ {
		got, err := s.EvaluateData(rule, data)
		if err != nil {
			t.Fatal(err)
		}
		if got != "42" {
			t.Errorf("Session.EvaluateData: want 42, got %q", got)
		}
	}

	if data.AllocatedBytes() == 0 {
		t.Error("expected non-zero DataHandle.AllocatedBytes")
	}

	// Close is idempotent, and a closed handle degrades to an error,
	// not a crash.
	data.Close()
	data.Close()
	if _, err := s.EvaluateData(rule, data); err == nil {
		t.Error("expected error evaluating against a closed DataHandle")
	}
}

func TestParseDataError(t *testing.T) {
	_, err := ParseData(`{definitely-not-json`)
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

// DataHandle is documented as safe for concurrent use: share one handle
// (and one rule) between goroutines, each with its own Session, plus
// the goroutine-safe Rule.EvaluateData path.
func TestDataHandleConcurrent(t *testing.T) {
	e := NewEngine()
	defer e.Close()

	rule, err := e.Compile(`{"+":[{"var":"a"},{"var":"b"}]}`)
	if err != nil {
		t.Fatal(err)
	}
	defer rule.Close()

	data, err := ParseData(`{"a":40,"b":2}`)
	if err != nil {
		t.Fatal(err)
	}
	defer data.Close()

	const goroutines = 2
	const iters = 200
	var wg sync.WaitGroup
	errs := make(chan error, goroutines*2*iters)

	for g := 0; g < goroutines; g++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			s := e.Session() // sessions are per-goroutine
			defer s.Close()
			for i := 0; i < iters; i++ {
				if got, err := s.EvaluateData(rule, data); err != nil {
					errs <- err
					return
				} else if got != "42" {
					errs <- fmt.Errorf("session: want 42, got %q", got)
					return
				}
				if got, err := rule.EvaluateData(data); err != nil {
					errs <- err
					return
				} else if got != "42" {
					errs <- fmt.Errorf("rule: want 42, got %q", got)
					return
				}
			}
		}()
	}
	wg.Wait()
	close(errs)
	for err := range errs {
		t.Error(err)
	}
}

func TestTypedEvaluations(t *testing.T) {
	e := NewEngine()
	defer e.Close()
	s := e.Session()
	defer s.Close()

	data, err := ParseData(`{"n":6,"f":1.5,"s":"hi","empty":""}`)
	if err != nil {
		t.Fatal(err)
	}
	defer data.Close()

	compile := func(rule string) *Rule {
		t.Helper()
		r, err := e.Compile(rule)
		if err != nil {
			t.Fatal(err)
		}
		return r
	}

	boolRule := compile(`{">":[{"var":"n"},5]}`)
	defer boolRule.Close()
	if got, err := s.EvaluateBool(boolRule, data); err != nil || got != true {
		t.Errorf("EvaluateBool: want true/nil, got %v/%v", got, err)
	}

	intRule := compile(`{"*":[{"var":"n"},7]}`)
	defer intRule.Close()
	if got, err := s.EvaluateInt64(intRule, data); err != nil || got != 42 {
		t.Errorf("EvaluateInt64: want 42/nil, got %v/%v", got, err)
	}

	floatRule := compile(`{"var":"f"}`)
	defer floatRule.Close()
	if got, err := s.EvaluateFloat64(floatRule, data); err != nil || got != 1.5 {
		t.Errorf("EvaluateFloat64: want 1.5/nil, got %v/%v", got, err)
	}
	// Integer results satisfy the float accessor too.
	if got, err := s.EvaluateFloat64(intRule, data); err != nil || got != 42.0 {
		t.Errorf("EvaluateFloat64(int result): want 42/nil, got %v/%v", got, err)
	}

	// Truthy coerces any result and never type-mismatches.
	strRule := compile(`{"var":"s"}`)
	defer strRule.Close()
	if got, err := s.EvaluateTruthy(strRule, data); err != nil || got != true {
		t.Errorf(`EvaluateTruthy("hi"): want true/nil, got %v/%v`, got, err)
	}
	emptyRule := compile(`{"var":"empty"}`)
	defer emptyRule.Close()
	if got, err := s.EvaluateTruthy(emptyRule, data); err != nil || got != false {
		t.Errorf(`EvaluateTruthy(""): want false/nil, got %v/%v`, got, err)
	}
}

func TestTypedEvaluationTypeMismatch(t *testing.T) {
	e := NewEngine()
	defer e.Close()
	s := e.Session()
	defer s.Close()

	data, err := ParseData(`{"f":1.5,"s":"hi"}`)
	if err != nil {
		t.Fatal(err)
	}
	defer data.Close()

	strRule, err := e.Compile(`{"var":"s"}`)
	if err != nil {
		t.Fatal(err)
	}
	defer strRule.Close()
	floatRule, err := e.Compile(`{"var":"f"}`)
	if err != nil {
		t.Fatal(err)
	}
	defer floatRule.Close()

	assertMismatch := func(name string, err error) {
		t.Helper()
		if err == nil {
			t.Fatalf("%s: expected TypeMismatch error, got nil", name)
		}
		derr, ok := err.(*Error)
		if !ok {
			t.Fatalf("%s: want *Error, got %T (%v)", name, err, err)
		}
		if derr.Type != "TypeMismatch" {
			t.Errorf("%s: want Type=TypeMismatch, got %q", name, derr.Type)
		}
		if derr.Message == "" {
			t.Errorf("%s: expected non-empty message", name)
		}
	}

	_, err = s.EvaluateBool(strRule, data) // "hi" is not a strict bool
	assertMismatch("EvaluateBool", err)
	_, err = s.EvaluateInt64(floatRule, data) // 1.5 is not an integer
	assertMismatch("EvaluateInt64", err)
	_, err = s.EvaluateFloat64(strRule, data) // "hi" is not a number
	assertMismatch("EvaluateFloat64", err)
}

func TestEvaluateBatchPerItemFailure(t *testing.T) {
	e := NewEngine()
	defer e.Close()
	s := e.Session()
	defer s.Close()

	rule, err := e.Compile(`{"if":[{"var":"fail"},{"throw":"boom"},{"var":"x"}]}`)
	if err != nil {
		t.Fatal(err)
	}
	defer rule.Close()

	parse := func(j string) *DataHandle {
		t.Helper()
		d, err := ParseData(j)
		if err != nil {
			t.Fatal(err)
		}
		return d
	}
	d0 := parse(`{"x":1}`)
	defer d0.Close()
	d1 := parse(`{"fail":true}`)
	defer d1.Close()
	d2 := parse(`{"x":3}`)
	defer d2.Close()

	results, err := s.EvaluateBatch(rule, []*DataHandle{d0, d1, d2})
	if err != nil {
		t.Fatal(err)
	}
	if len(results) != 3 {
		t.Fatalf("want 3 results, got %d", len(results))
	}
	if results[0].Err != nil || results[0].Value != "1" {
		t.Errorf("item 0: want Value=1, got %+v", results[0])
	}
	if results[2].Err != nil || results[2].Value != "3" {
		t.Errorf("item 2: want Value=3, got %+v", results[2])
	}
	// The middle item failed — its neighbours are unaffected, its error
	// decodes into the binding's *Error with the engine tag preserved.
	if results[1].Err == nil {
		t.Fatalf("item 1: expected error, got %+v", results[1])
	}
	if results[1].Value != "" {
		t.Errorf("item 1: want empty Value alongside Err, got %q", results[1].Value)
	}
	derr, ok := results[1].Err.(*Error)
	if !ok {
		t.Fatalf("item 1: want *Error, got %T", results[1].Err)
	}
	if derr.Type != "Thrown" {
		t.Errorf("item 1: want Type=Thrown, got %q", derr.Type)
	}
	if derr.Message == "" {
		t.Error("item 1: expected non-empty message")
	}

	// A nil handle in the slice is a per-item failure, not a call failure.
	results, err = s.EvaluateBatch(rule, []*DataHandle{d0, nil})
	if err != nil {
		t.Fatal(err)
	}
	if results[0].Err != nil || results[0].Value != "1" {
		t.Errorf("nil-handle batch item 0: got %+v", results[0])
	}
	if results[1].Err == nil {
		t.Error("nil-handle batch item 1: expected error")
	}

	// Empty input short-circuits without touching the native layer.
	results, err = s.EvaluateBatch(rule, nil)
	if err != nil || results != nil {
		t.Errorf("empty batch: want (nil, nil), got (%v, %v)", results, err)
	}
}

func TestEvaluateManyPerItemFailure(t *testing.T) {
	e := NewEngine()
	defer e.Close()
	s := e.Session()
	defer s.Close()

	okRule, err := e.Compile(`{"+":[{"var":"x"},1]}`)
	if err != nil {
		t.Fatal(err)
	}
	defer okRule.Close()
	throwRule, err := e.Compile(`{"throw":"nope"}`)
	if err != nil {
		t.Fatal(err)
	}
	defer throwRule.Close()
	varRule, err := e.Compile(`{"var":"x"}`)
	if err != nil {
		t.Fatal(err)
	}
	defer varRule.Close()

	data, err := ParseData(`{"x":41}`)
	if err != nil {
		t.Fatal(err)
	}
	defer data.Close()

	results, err := s.EvaluateMany([]*Rule{okRule, throwRule, varRule}, data)
	if err != nil {
		t.Fatal(err)
	}
	if len(results) != 3 {
		t.Fatalf("want 3 results, got %d", len(results))
	}
	if results[0].Err != nil || results[0].Value != "42" {
		t.Errorf("item 0: want Value=42, got %+v", results[0])
	}
	if results[1].Err == nil {
		t.Fatal("item 1: expected error from throw rule")
	}
	if derr := results[1].Err.(*Error); derr.Type != "Thrown" {
		t.Errorf("item 1: want Type=Thrown, got %q", derr.Type)
	}
	if results[2].Err != nil || results[2].Value != "41" {
		t.Errorf("item 2: want Value=41, got %+v", results[2])
	}

	// A rule from a different engine fails per-item with InvalidArgument.
	other := NewEngine()
	defer other.Close()
	foreignRule, err := other.Compile(`{"var":"x"}`)
	if err != nil {
		t.Fatal(err)
	}
	defer foreignRule.Close()

	results, err = s.EvaluateMany([]*Rule{okRule, foreignRule}, data)
	if err != nil {
		t.Fatal(err)
	}
	if results[0].Err != nil {
		t.Errorf("item 0: unexpected error %v", results[0].Err)
	}
	if results[1].Err == nil {
		t.Fatal("item 1: expected cross-engine error")
	}
	if derr := results[1].Err.(*Error); derr.Type != "InvalidArgument" {
		t.Errorf("item 1: want Type=InvalidArgument, got %q", derr.Type)
	}
}

// A rule compiled by one engine cannot run on a session opened on
// another — the C ABI rejects the pair with InvalidArgument.
func TestSessionRejectsForeignRule(t *testing.T) {
	e1 := NewEngine()
	defer e1.Close()
	e2 := NewEngine()
	defer e2.Close()

	rule, err := e1.Compile(`{"var":"x"}`)
	if err != nil {
		t.Fatal(err)
	}
	defer rule.Close()

	s := e2.Session()
	defer s.Close()

	_, err = s.Evaluate(rule, `{"x":1}`)
	if err == nil {
		t.Fatal("expected cross-engine error, got nil")
	}
	derr, ok := err.(*Error)
	if !ok {
		t.Fatalf("want *Error, got %T (%v)", err, err)
	}
	if derr.Type != "InvalidArgument" {
		t.Errorf("want Type=InvalidArgument, got %q", derr.Type)
	}
	if !strings.Contains(derr.Message, "different engine") {
		t.Errorf("want mention of engine mismatch, got %q", derr.Message)
	}
}

// Empty strings cross the boundary as (nil, 0) — the ABI reads them as
// "" and surfaces a parse error, never a crash.
func TestEmptyStringInputs(t *testing.T) {
	e := NewEngine()
	defer e.Close()

	if _, err := e.Apply("", ""); err == nil {
		t.Error("Apply(\"\", \"\"): expected parse error, got nil")
	} else if derr := err.(*Error); derr.Type != "ParseError" {
		t.Errorf("Apply empty rule: want Type=ParseError, got %q", derr.Type)
	}

	if _, err := e.Compile(""); err == nil {
		t.Error("Compile(\"\"): expected parse error, got nil")
	}

	rule, err := e.Compile(`{"var":"x"}`)
	if err != nil {
		t.Fatal(err)
	}
	defer rule.Close()
	if _, err := rule.Evaluate(""); err == nil {
		t.Error("Evaluate(\"\"): expected parse error, got nil")
	}

	s := e.Session()
	defer s.Close()
	if _, err := s.Evaluate(rule, ""); err == nil {
		t.Error("Session.Evaluate(\"\"): expected parse error, got nil")
	}

	if _, err := ParseData(""); err == nil {
		t.Error("ParseData(\"\"): expected parse error, got nil")
	}

	// Empty string as a *value* inside the JSON still round-trips.
	got, err := e.Apply(`{"var":"s"}`, `{"s":""}`)
	if err != nil {
		t.Fatal(err)
	}
	if got != `""` {
		t.Errorf("want \"\" result, got %q", got)
	}
}

// Error fields survive the v2 status-code + error-handle round trip.
func TestErrorFieldsThroughV2(t *testing.T) {
	e := NewEngine()
	defer e.Close()

	rule, err := e.Compile(`{"+":[1,{"throw":"kaput"}]}`)
	if err != nil {
		t.Fatal(err)
	}
	defer rule.Close()

	_, err = rule.Evaluate(`{}`)
	if err == nil {
		t.Fatal("expected error, got nil")
	}
	derr := err.(*Error)
	if derr.Type != "Thrown" {
		t.Errorf("want Type=Thrown, got %q", derr.Type)
	}
	if !strings.Contains(derr.Message, "kaput") {
		t.Errorf("want message to carry the thrown value, got %q", derr.Message)
	}
	if derr.Operator == "" {
		t.Error("expected non-empty Operator for a nested throw")
	}
	if !strings.HasPrefix(derr.PathJSON, "[") {
		t.Errorf("want PathJSON to be a JSON array, got %q", derr.PathJSON)
	}
	if !strings.Contains(derr.Error(), "Thrown") {
		t.Errorf("Error() should include the tag, got %q", derr.Error())
	}
}
