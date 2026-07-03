// custom-operator: register a Go `double` operator and call it from a rule.
// Custom operators receive their pre-evaluated arguments as a JSON-array
// string and return a JSON-value string. Built-in names always win.
//
// Run from bindings/go/ (build first: make build):
//
//	go run ./examples/custom-operator

package main

import (
	"encoding/json"
	"fmt"
	"log"

	datalogic "github.com/GoPlasmatic/datalogic-rs/bindings/go/v5"
)

func main() {
	engine, err := datalogic.NewEngineBuilder().
		AddOperator("double", func(argsJSON string) (string, error) {
			var args []float64
			if err := json.Unmarshal([]byte(argsJSON), &args); err != nil {
				return "", err
			}
			out, err := json.Marshal(args[0] * 2)
			return string(out), err
		}).
		Build()
	if err != nil {
		log.Fatal(err)
	}
	defer engine.Close()

	out, err := engine.Apply(`{"double": [21]}`, `{}`)
	if err != nil {
		log.Fatal(err)
	}
	fmt.Println(out) // 42
}
