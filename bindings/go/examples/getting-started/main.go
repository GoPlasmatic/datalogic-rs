// getting-started: one-shot JSONLogic evaluation with the datalogic Go binding.
//
// Run from bindings/go/ (build first: make build):
//
//	go run ./examples/getting-started

package main

import (
	"fmt"
	"log"

	datalogic "github.com/GoPlasmatic/datalogic-rs/bindings/go/v5"
)

func main() {
	rule := `{"and": [{">=": [{"var": "age"}, 18]}, {"==": [{"var": "status"}, "active"]}]}`
	data := `{"age": 25, "status": "active"}`

	out, err := datalogic.Apply(rule, data)
	if err != nil {
		log.Fatal(err)
	}
	fmt.Println(out) // true
}
