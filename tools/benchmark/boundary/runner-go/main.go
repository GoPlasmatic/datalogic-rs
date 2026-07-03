// Boundary-benchmark runner for the Go binding (`runtime: "go"`).
//
// Exercises the binding's public API on the ABI-v2 tiers. Surface used:
//
//   - datalogic.ParseData(json string) (*datalogic.DataHandle, error),
//     (*DataHandle).Close()
//   - (*Session).EvaluateData(rule *Rule, data *DataHandle) (string, error)
//   - (*Session).EvaluateMany(rules []*Rule, data *DataHandle)
//     ([]datalogic.BatchResult, error) — BatchResult{Value string; Err error}
//
// Modes: session-evaluate, session-evaluate-data,
// session-evaluate-many-100 (ns_op reported per evaluation: call/100),
// rule-evaluate, engine-apply-oneshot.
//
// Timing discipline (BINDINGS-OVERHEAD.md appendix): warmup 2,000
// iterations (native tier), pilot to ~250 ms per sample, median of 5,
// results consumed into a sink.
//
// Usage: go run . <workloads-dir> [--modes=a,b] [--workloads=x,y]
package main

import (
	"fmt"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"time"

	datalogic "github.com/GoPlasmatic/datalogic-rs/bindings/go/v5"
)

const (
	runtimeName    = "go"
	warmup         = 2000
	targetSampleNs = 250e6
	pilotMinNs     = 10e6
	samples        = 5
	manyN          = 100
)

var globalSink uint64

// measure runs warmup + pilot + median-of-5 over batch(n) -> sink.
func measure(batch func(n uint64) uint64) float64 {
	globalSink += batch(warmup)

	n := uint64(32)
	var perOp float64
	for {
		t0 := time.Now()
		globalSink += batch(n)
		elapsed := float64(time.Since(t0).Nanoseconds())
		if elapsed >= pilotMinNs {
			perOp = elapsed / float64(n)
			break
		}
		n *= 2
	}

	iters := uint64(targetSampleNs / perOp)
	if iters < 1 {
		iters = 1
	}
	out := make([]float64, samples)
	for s := 0; s < samples; s++ {
		t0 := time.Now()
		globalSink += batch(iters)
		out[s] = float64(time.Since(t0).Nanoseconds()) / float64(iters)
	}
	sort.Float64s(out)
	return out[samples/2]
}

func emit(mode, workload string, nsOp float64) {
	fmt.Printf("{\"runtime\": %q, \"mode\": %q, \"workload\": %q, \"ns_op\": %.3f}\n",
		runtimeName, mode, workload, nsOp)
}

func verify(mode, workload, got, expected string) {
	if got != expected {
		fmt.Fprintf(os.Stderr,
			"runner-go: verification failed for mode=%s workload=%s\n  expected: %s\n  got:      %s\n",
			mode, workload, expected, got)
		os.Exit(1)
	}
}

func must(err error, what string) {
	if err != nil {
		fmt.Fprintf(os.Stderr, "runner-go: %s: %v\n", what, err)
		os.Exit(1)
	}
}

func selected(filter []string, name string) bool {
	if filter == nil {
		return true
	}
	for _, f := range filter {
		if f == name {
			return true
		}
	}
	return false
}

func main() {
	var dir string
	var modeFilter, workloadFilter []string
	for _, arg := range os.Args[1:] {
		switch {
		case strings.HasPrefix(arg, "--modes="):
			modeFilter = strings.Split(strings.TrimPrefix(arg, "--modes="), ",")
		case strings.HasPrefix(arg, "--workloads="):
			workloadFilter = strings.Split(strings.TrimPrefix(arg, "--workloads="), ",")
		default:
			dir = arg
		}
	}
	if dir == "" {
		fmt.Fprintln(os.Stderr, "usage: runner-go <workloads-dir> [--modes=a,b] [--workloads=x,y]")
		os.Exit(1)
	}

	engine := datalogic.NewEngine()
	defer engine.Close()

	for _, name := range []string{"simple", "eligibility", "array100"} {
		if !selected(workloadFilter, name) {
			continue
		}
		read := func(suffix string) string {
			b, err := os.ReadFile(filepath.Join(dir, name+"."+suffix+".json"))
			must(err, "read workload "+name+"."+suffix)
			return string(b)
		}
		ruleJSON, dataJSON, expected := read("rule"), read("data"), read("expected")

		rule, err := engine.Compile(ruleJSON)
		must(err, "compile")
		session := engine.Session()

		// v2: parse-once data handle.
		dataHandle, err := datalogic.ParseData(dataJSON)
		must(err, "ParseData")

		// 100 identical rules, compiled separately (a rule-set of
		// identical rules — separate compiles so the batch doesn't
		// flatter one hot compiled tree).
		manyRules := make([]*datalogic.Rule, manyN)
		for i := range manyRules {
			r, err := engine.Compile(ruleJSON)
			must(err, "compile (many)")
			manyRules[i] = r
		}

		type spec struct {
			verify func()
			batch  func(n uint64) uint64
			// evaluations performed per batch iteration (ns_op divisor).
			perCallEvals float64
		}
		modes := map[string]spec{
			"session-evaluate": {
				verify: func() {
					got, err := session.Evaluate(rule, dataJSON)
					must(err, "session.Evaluate")
					verify("session-evaluate", name, got, expected)
				},
				batch: func(n uint64) uint64 {
					var sink uint64
					for i := uint64(0); i < n; i++ {
						out, err := session.Evaluate(rule, dataJSON)
						if err != nil {
							must(err, "session.Evaluate (timed)")
						}
						sink += uint64(len(out))
					}
					return sink
				},
				perCallEvals: 1,
			},
			"session-evaluate-data": {
				verify: func() {
					got, err := session.EvaluateData(rule, dataHandle)
					must(err, "session.EvaluateData")
					verify("session-evaluate-data", name, got, expected)
				},
				batch: func(n uint64) uint64 {
					var sink uint64
					for i := uint64(0); i < n; i++ {
						out, err := session.EvaluateData(rule, dataHandle)
						if err != nil {
							must(err, "session.EvaluateData (timed)")
						}
						sink += uint64(len(out))
					}
					return sink
				},
				perCallEvals: 1,
			},
			"session-evaluate-many-100": {
				verify: func() {
					// v2: N rules x one data handle; per-item outcomes.
					results, err := session.EvaluateMany(manyRules, dataHandle)
					must(err, "session.EvaluateMany")
					for _, r := range results {
						must(r.Err, "session.EvaluateMany item")
						verify("session-evaluate-many-100", name, r.Value, expected)
					}
				},
				batch: func(n uint64) uint64 {
					var sink uint64
					for i := uint64(0); i < n; i++ {
						results, err := session.EvaluateMany(manyRules, dataHandle)
						if err != nil {
							must(err, "session.EvaluateMany (timed)")
						}
						sink += uint64(len(results[0].Value)) + uint64(len(results[manyN-1].Value))
					}
					return sink
				},
				perCallEvals: manyN,
			},
			"rule-evaluate": {
				verify: func() {
					got, err := rule.Evaluate(dataJSON)
					must(err, "rule.Evaluate")
					verify("rule-evaluate", name, got, expected)
				},
				batch: func(n uint64) uint64 {
					var sink uint64
					for i := uint64(0); i < n; i++ {
						out, err := rule.Evaluate(dataJSON)
						if err != nil {
							must(err, "rule.Evaluate (timed)")
						}
						sink += uint64(len(out))
					}
					return sink
				},
				perCallEvals: 1,
			},
			"engine-apply-oneshot": {
				verify: func() {
					got, err := engine.Apply(ruleJSON, dataJSON)
					must(err, "engine.Apply")
					verify("engine-apply-oneshot", name, got, expected)
				},
				batch: func(n uint64) uint64 {
					var sink uint64
					for i := uint64(0); i < n; i++ {
						out, err := engine.Apply(ruleJSON, dataJSON)
						if err != nil {
							must(err, "engine.Apply (timed)")
						}
						sink += uint64(len(out))
					}
					return sink
				},
				perCallEvals: 1,
			},
		}

		// Stable emission order.
		for _, mode := range []string{
			"session-evaluate", "session-evaluate-data", "session-evaluate-many-100",
			"rule-evaluate", "engine-apply-oneshot",
		} {
			if !selected(modeFilter, mode) {
				continue
			}
			m := modes[mode]
			m.verify()
			emit(mode, name, measure(m.batch)/m.perCallEvals)
		}

		for _, r := range manyRules {
			r.Close()
		}
		dataHandle.Close()
		session.Close()
		rule.Close()
	}

	fmt.Fprintf(os.Stderr, "runner-go: sink=%d\n", globalSink)
}
