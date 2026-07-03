// compile-once-evaluate-many: compile a rule once, evaluate it against
// ~100k payloads, and print the last result plus a rough per-evaluation cost.
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

	var last string
	start := time.Now()
	for i := 0; i < iterations; i++ {
		last, err = rule.Evaluate(fmt.Sprintf(`{"price": %d, "discount": 0.2}`, 100+i%100))
		if err != nil {
			log.Fatal(err)
		}
	}
	elapsed := time.Since(start)

	fmt.Printf("last result: %s\n", last)
	fmt.Printf("%d evaluations, ~%d ns/op\n", iterations, elapsed.Nanoseconds()/iterations)
}
