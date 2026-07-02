package datalogic

// JSONLogic conformance suite, driven through the Go binding (and
// therefore the C ABI + cgo string marshalling underneath). Walks the
// same suites as the core crate's test_jsonlogic.rs runner, discovered
// via index.json, and mirrors its per-case semantics:
//
//   - "result" cases: Apply must succeed and the returned JSON must be
//     semantically equal to the expected value. Both sides are decoded
//     with encoding/json into interface{} and compared with
//     reflect.DeepEqual; encoding/json decodes every number as float64,
//     which makes int/float representation differences a non-issue.
//   - "error" cases: Apply must return a non-nil error with a non-empty
//     message. (The C ABI collapses the expected error object into the
//     last-error block, so we assert that an error surfaced, not its
//     exact shape.)
//
// The flagd/ suites run unconditionally: the C ABI static library is
// always built with the core crate's `flagd` feature enabled.

import (
	"bytes"
	"encoding/json"
	"os"
	"path/filepath"
	"reflect"
	"testing"
)

// suitesRoot is relative to this package directory, which is the cwd
// for `go test`.
const suitesRoot = "../../crates/datalogic-rs/tests/suites"

// normalizeJSON decodes raw JSON and re-encodes it with sorted object
// keys (numbers pass through verbatim via json.Number). The core
// runner (test_jsonlogic.rs) hands rule and data to the engine as
// serde_json::Value, whose objects are key-sorted BTreeMaps — and the
// object-iteration suites' expected arrays were written against that
// key order. encoding/json sorts map keys on Marshal, so one
// decode/encode round trip reproduces the same normalization.
func normalizeJSON(t *testing.T, raw json.RawMessage) string {
	t.Helper()
	dec := json.NewDecoder(bytes.NewReader(raw))
	dec.UseNumber()
	var v interface{}
	if err := dec.Decode(&v); err != nil {
		t.Fatalf("normalize: invalid JSON %s: %v", raw, err)
	}
	out, err := json.Marshal(v)
	if err != nil {
		t.Fatalf("normalize: re-encode failed: %v", err)
	}
	return string(out)
}

func TestJSONLogicConformance(t *testing.T) {
	indexRaw, err := os.ReadFile(filepath.Join(suitesRoot, "index.json"))
	if err != nil {
		t.Fatalf("read index.json: %v", err)
	}
	var index []string
	if err := json.Unmarshal(indexRaw, &index); err != nil {
		t.Fatalf("parse index.json: %v", err)
	}

	// Engines are stateless across evaluations — share one per
	// templating mode instead of rebuilding per case.
	plain := NewEngine()
	defer plain.Close()
	templating := NewTemplatingEngine()
	defer templating.Close()

	totalPassed, totalFailed := 0, 0
	for _, suiteFile := range index {
		path := filepath.Join(suitesRoot, suiteFile)
		if _, err := os.Stat(path); err != nil {
			// Mirror the core runner: a stale index entry is a warning,
			// not a failure.
			t.Logf("WARNING: skipping %s (file not found)", suiteFile)
			continue
		}
		passed, failed := runConformanceSuite(t, suiteFile, path, plain, templating)
		t.Logf("%s: %d passed, %d failed", suiteFile, passed, failed)
		totalPassed += passed
		totalFailed += failed
	}

	t.Logf("TOTAL (via Go binding): %d passed, %d failed", totalPassed, totalFailed)
	if totalPassed == 0 {
		t.Fatal("no conformance cases ran")
	}
	// Individual mismatches were already reported via t.Errorf inside
	// runConformanceSuite; totalFailed > 0 implies the test is failing.
}

func runConformanceSuite(t *testing.T, suiteFile, path string, plain, templating *Engine) (passed, failed int) {
	t.Helper()

	raw, err := os.ReadFile(path)
	if err != nil {
		t.Fatalf("read %s: %v", path, err)
	}
	var entries []json.RawMessage
	if err := json.Unmarshal(raw, &entries); err != nil {
		t.Fatalf("parse %s: %v", path, err)
	}

	for i, entry := range entries {
		// String entries are section headers, not cases.
		var header string
		if json.Unmarshal(entry, &header) == nil {
			continue
		}
		var c map[string]json.RawMessage
		if err := json.Unmarshal(entry, &c); err != nil {
			t.Fatalf("%s[%d] is neither string nor object: %v", suiteFile, i, err)
		}

		description := "No description"
		if d, ok := c["description"]; ok {
			var s string
			if json.Unmarshal(d, &s) == nil {
				description = s
			}
		}
		rule, ok := c["rule"]
		if !ok {
			t.Fatalf("%s[%d] missing 'rule'", suiteFile, i)
		}
		data := json.RawMessage(`{}`)
		if d, ok := c["data"]; ok {
			data = d
		}
		useTemplating := false
		if tv, ok := c["templating"]; ok {
			_ = json.Unmarshal(tv, &useTemplating)
		}
		expectedRaw, hasResult := c["result"]
		_, hasError := c["error"]
		if !hasResult && !hasError {
			t.Fatalf("%s[%d] missing 'result' or 'error'", suiteFile, i)
		}

		engine := plain
		if useTemplating {
			engine = templating
		}
		got, err := engine.Apply(normalizeJSON(t, rule), normalizeJSON(t, data))

		if hasError {
			if err == nil {
				failed++
				t.Errorf("%s[%d] %s: expected error %s, got result %s",
					suiteFile, i, description, c["error"], got)
				continue
			}
			if derr, ok := err.(*Error); ok && derr.Message == "" {
				failed++
				t.Errorf("%s[%d] %s: error surfaced but message is empty",
					suiteFile, i, description)
				continue
			}
			passed++
			continue
		}

		if err != nil {
			failed++
			t.Errorf("%s[%d] %s: expected %s, got error: %v",
				suiteFile, i, description, expectedRaw, err)
			continue
		}
		var gotVal, wantVal interface{}
		if err := json.Unmarshal([]byte(got), &gotVal); err != nil {
			failed++
			t.Errorf("%s[%d] %s: result is not valid JSON (%v): %s",
				suiteFile, i, description, err, got)
			continue
		}
		if err := json.Unmarshal(expectedRaw, &wantVal); err != nil {
			t.Fatalf("%s[%d] expected result is not valid JSON: %v", suiteFile, i, err)
		}
		if !reflect.DeepEqual(gotVal, wantVal) {
			failed++
			t.Errorf("%s[%d] %s: expected %s, got %s",
				suiteFile, i, description, expectedRaw, got)
			continue
		}
		passed++
	}

	return passed, failed
}
