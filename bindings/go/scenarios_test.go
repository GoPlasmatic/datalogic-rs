package datalogic

// The cross-binding scenarios in bindings/scenarios/api.json, through the
// Go API. Every binding runs the same file (see bindings/BINDINGS.md).

import (
	"encoding/json"
	"os"
	"reflect"
	"testing"
)

type scenario struct {
	Description string          `json:"description"`
	Call        string          `json:"call"`
	Engine      map[string]any  `json:"engine"`
	Rule        json.RawMessage `json:"rule"`
	Data        json.RawMessage `json:"data"`
	Value       json.RawMessage `json:"value"`
	Mode        string          `json:"mode"`
	Budget      uint64          `json:"budget"`
	Result      json.RawMessage `json:"result"`
	Error       *string         `json:"error"`
	Diagnostics json.RawMessage `json:"diagnostics"`
	Facts       map[string]any  `json:"facts"`
}

func loadScenarios(t *testing.T) []scenario {
	raw, err := os.ReadFile("../scenarios/api.json")
	if err != nil {
		t.Fatal(err)
	}
	var items []json.RawMessage
	if err := json.Unmarshal(raw, &items); err != nil {
		t.Fatal(err)
	}
	var out []scenario
	for _, it := range items {
		var s scenario
		if json.Unmarshal(it, &s) == nil && s.Call != "" {
			out = append(out, s)
		}
	}
	return out
}

func scenarioEngine(t *testing.T, s scenario) *Engine {
	b := NewEngineBuilder()
	if v, _ := s.Engine["templating"].(bool); v {
		b.Templating(true)
	}
	if v, ok := s.Engine["template_key_escape"].(string); ok {
		b.TemplateKeyEscape([]rune(v)[0])
	}
	if cfg, ok := s.Engine["config"]; ok && cfg != nil {
		raw, _ := json.Marshal(cfg)
		if err := b.SetConfigJSON(string(raw)); err != nil {
			t.Fatal(err)
		}
	}
	e, err := b.Build()
	if err != nil {
		t.Fatal(err)
	}
	return e
}

func scenarioMode(m string) Mode {
	switch m {
	case "strict":
		return ModeStrict
	case "template":
		return ModeTemplate
	}
	return ModeEngine
}

func decode(s string) any {
	var v any
	if err := json.Unmarshal([]byte(s), &v); err != nil {
		panic(err)
	}
	return v
}

// runScenario returns the call's value, or the error type as an *Error.
func runScenario(t *testing.T, s scenario) (any, error) {
	e := scenarioEngine(t, s)
	defer e.Close()
	switch s.Call {
	case "check":
		out, err := e.Check(string(s.Rule), scenarioMode(s.Mode))
		if err != nil {
			return nil, err
		}
		pairs := []any{}
		for _, d := range decode(out).([]any) {
			m := d.(map[string]any)
			pairs = append(pairs, []any{m["code"], m["pointer"]})
		}
		return pairs, nil
	case "truthy":
		return e.Truthy(string(s.Value))
	case "facts":
		r, err := e.Compile(string(s.Rule))
		if err != nil {
			return nil, err
		}
		defer r.Close()
		out, err := r.Facts()
		if err != nil {
			return nil, err
		}
		return decode(out), nil
	case "metered":
		r, err := e.Compile(string(s.Rule))
		if err != nil {
			return nil, err
		}
		defer r.Close()
		sess := e.Session()
		defer sess.Close()
		out, _, err := sess.EvaluateMetered(r, string(s.Data), s.Budget)
		if err != nil {
			return nil, err
		}
		return decode(out), nil
	}
	compile := map[string]func(string) (*Rule, error){
		"evaluate":         e.Compile,
		"compile_template": e.CompileTemplate,
		"compile_strict":   e.CompileStrict,
		"compile_checked":  e.CompileChecked,
	}[s.Call]
	r, err := compile(string(s.Rule))
	if err != nil {
		return nil, err
	}
	defer r.Close()
	out, err := r.Evaluate(string(s.Data))
	if err != nil {
		return nil, err
	}
	return decode(out), nil
}

func TestScenarios(t *testing.T) {
	cases := loadScenarios(t)
	if len(cases) < 25 {
		t.Fatalf("only %d scenarios", len(cases))
	}
	for _, s := range cases {
		t.Run(s.Call+": "+s.Description, func(t *testing.T) {
			got, err := runScenario(t, s)
			if s.Error != nil {
				derr, ok := err.(*Error)
				if !ok || derr.Type != *s.Error {
					t.Fatalf("want error %s, got %v / %#v", *s.Error, got, err)
				}
				return
			}
			if err != nil {
				t.Fatal(err)
			}
			switch {
			case s.Diagnostics != nil:
				if !reflect.DeepEqual(got, decode(string(s.Diagnostics))) {
					t.Fatalf("diagnostics %v", got)
				}
			case s.Facts != nil:
				m := got.(map[string]any)
				for k, v := range s.Facts {
					if !reflect.DeepEqual(m[k], v) {
						t.Fatalf("%s: got %v want %v", k, m[k], v)
					}
				}
			default:
				if !reflect.DeepEqual(got, decode(string(s.Result))) {
					t.Fatalf("got %v want %s", got, s.Result)
				}
			}
		})
	}
}
