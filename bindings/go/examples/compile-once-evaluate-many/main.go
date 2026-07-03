// compile-once-evaluate-many: compile a rule once, evaluate it against
// ~100k payloads, and print the last result plus a rough per-evaluation
// cost — first via JSON strings, then via pre-parsed data handles (the
// hot path: zero parse work per call), and finally as one batch call.
//
// Run from bindings/go/ (build first: make build):
//
//	go run ./examples/compile-once-evaluate-many

package main

import (
	"fmt"
	"log"
	"time"

	datalogic "github.com/GoPlasmatic/datalogic-rs/bindings/go/v5"
)

const iterations = 100_000

func main() {
	engine := datalogic.NewEngine()
	defer engine.Close()

	rule, err := engine.Compile(`{"*": [{"var": "price"}, {"-": [1, {"var": "discount"}]}]}`)
	if err != nil {
		log.Fatal(err)
	}
	defer rule.Close()

	// Tier 1 — compiled rule, JSON-string data (re-parsed per call).
	var last string
	start := time.Now()
	for i := 0; i < iterations; i++ {
		last, err = rule.Evaluate(fmt.Sprintf(`{"price": %d, "discount": 0.2}`, 100+i%100))
		if err != nil {
			log.Fatal(err)
		}
	}
	elapsed := time.Since(start)
	fmt.Printf("string data:  last result %s, %d evaluations, ~%d ns/op\n",
		last, iterations, elapsed.Nanoseconds()/iterations)

	// Tier 2 — session + pre-parsed data handles: parse each distinct
	// payload once, then every evaluation skips JSON parsing entirely.
	handles := make([]*datalogic.DataHandle, 100)
	for i := range handles {
		h, err := datalogic.ParseData(fmt.Sprintf(`{"price": %d, "discount": 0.2}`, 100+i))
		if err != nil {
			log.Fatal(err)
		}
		defer h.Close()
		handles[i] = h
	}
	session := engine.Session()
	defer session.Close()

	start = time.Now()
	for i := 0; i < iterations; i++ {
		last, err = session.EvaluateData(rule, handles[i%100])
		if err != nil {
			log.Fatal(err)
		}
	}
	elapsed = time.Since(start)
	fmt.Printf("data handles: last result %s, %d evaluations, ~%d ns/op\n",
		last, iterations, elapsed.Nanoseconds()/iterations)

	// Tier 3 — one native call for the whole set: per-item results (and
	// per-item errors) come back in order.
	results, err := session.EvaluateBatch(rule, handles)
	if err != nil {
		log.Fatal(err)
	}
	fmt.Printf("batch:        %d results in one call, first %s, last %s\n",
		len(results), results[0].Value, results[len(results)-1].Value)
}
