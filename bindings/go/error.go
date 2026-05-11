package datalogic

/*
#include "datalogic.h"
*/
import "C"

import "fmt"

// Error is the error type returned by every datalogic operation. It
// carries the engine's stable error tag (Type), the failing operator
// name (when applicable), and a JSON-encoded leaf-to-root path from
// the compiled rule's node tree (when the failure happened with a
// compiled Rule in scope).
type Error struct {
	// Message is the human-readable error string.
	Message string
	// Type is the engine's stable error tag — one of "ParseError",
	// "Thrown", "NaN", "Custom", "InternalError", etc. Match on this
	// for programmatic error handling; Message is for humans.
	Type string
	// Operator is the outermost failing operator's name (e.g. "+" or
	// "var"). Empty when the error didn't originate inside a named
	// operator (e.g. rule-parse failures).
	Operator string
	// PathJSON is the resolved root-to-leaf error path as a JSON array
	// string, matching the Python binding's `.path` attribute. Empty
	// when the failing call didn't have a compiled Rule in scope.
	PathJSON string
}

// Error implements the `error` interface.
func (e *Error) Error() string {
	if e.Type != "" {
		return fmt.Sprintf("datalogic: %s: %s", e.Type, e.Message)
	}
	return "datalogic: " + e.Message
}

// lastError pulls the thread-local last-error state from the C ABI
// into a Go `*Error`. Called from each binding entry point immediately
// after a NULL return — the C ABI guarantees that NULL implies the
// last-error block is populated on this thread.
func lastError() *Error {
	msg := C.datalogic_last_error_message()
	if msg == nil {
		// Defensive — shouldn't happen if the C ABI honours its contract.
		return &Error{Message: "unknown error (no last-error set)"}
	}
	e := &Error{Message: C.GoString(msg)}
	if t := C.datalogic_last_error_type(); t != nil {
		e.Type = C.GoString(t)
	}
	if o := C.datalogic_last_error_operator(); o != nil {
		e.Operator = C.GoString(o)
	}
	if p := C.datalogic_last_error_path_json(); p != nil {
		e.PathJSON = C.GoString(p)
	}
	return e
}
