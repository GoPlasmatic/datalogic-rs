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
	// "Thrown", "NaN", "Custom", "TypeMismatch", "InvalidArgument",
	// "InternalError", etc. Match on this for programmatic error
	// handling; Message is for humans.
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

// takeError converts an owned C error handle (stored by a failing call
// into its `datalogic_error **` out-param) into a Go *Error and
// releases the handle. The accessors return borrowed (ptr, len) bytes
// that die with datalogic_error_free, so everything is copied first.
//
// Called with the handle a non-OK status left behind; a nil handle
// (the C side could not allocate detail) degrades to a generic error.
func takeError(cerr *C.datalogic_error) *Error {
	if cerr == nil {
		// Defensive — a non-OK status stores a handle whenever the
		// caller asked for capture, which this binding always does.
		return &Error{Message: "unknown error (no error detail captured)"}
	}
	defer C.datalogic_error_free(cerr)
	e := &Error{}
	var n C.size_t
	if p := C.datalogic_error_message(cerr, &n); p != nil {
		e.Message = goStringN(p, n)
	}
	if p := C.datalogic_error_tag(cerr, &n); p != nil {
		e.Type = goStringN(p, n)
	}
	if p := C.datalogic_error_operator(cerr, &n); p != nil {
		e.Operator = goStringN(p, n)
	}
	if p := C.datalogic_error_path_json(cerr, &n); p != nil {
		e.PathJSON = goStringN(p, n)
	}
	return e
}
