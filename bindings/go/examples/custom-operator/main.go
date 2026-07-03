// custom-operator: register a Go `double` operator and call it from a rule.
// Custom operators receive their pre-evaluated arguments as a JSON-array
// string and return a JSON-value string; a returned error becomes an
// evaluation error for the caller. Built-in names always win.
//
// Run from bindings/go/ (build first: make build):
//
//	go run ./examples/custom-operator

package main

import (
	"encoding/json"
	"errors"
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
			if len(args) == 0 {
				return "", errors.New("double expects one numeric argument")
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

	// Custom operators compose with built-ins.
	out, err = engine.Apply(`{"map": [{"var": "xs"}, {"double": [{"var": ""}]}]}`, `{"xs": [1, 2, 3]}`)
	if err != nil {
		log.Fatal(err)
	}
	fmt.Println(out) // [2,4,6]

	// The operator's error path surfaces as a regular evaluation error.
	_, err = engine.Apply(`{"double": []}`, `{}`)
	fmt.Println(err) // datalogic: Custom: ... double expects one numeric argument
}
