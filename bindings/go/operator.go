package datalogic

// Custom operator support via the C-ABI builder. Routes Go callbacks
// through a C trampoline; the user_data slot carries a `cgo.Handle` so
// we can fan out to many distinct Go closures per engine.
//
// Threading note: the engine may invoke a registered operator from any
// thread that calls Engine.Apply / Rule.Evaluate / Session.Evaluate.
// Go callbacks themselves are goroutine-safe (the runtime serialises
// the cgo crossing); user code inside the callback is responsible for
// its own synchronisation.

/*
#cgo CFLAGS: -I${SRCDIR}/include

#include <stdlib.h>
#include "datalogic.h"

// Declared in operator.go via //export — cgo emits the matching
// declaration in _cgo_export.h; here we just need the symbol to be
// resolvable in datalogic_go_get_trampoline below. Match the cgo-
// generated signature (no `const` qualifiers — cgo doesn't emit them).
//
// On Windows, cgo annotates exported symbols with __declspec(dllexport)
// in its generated header. clang (used for the windows/arm64 gnullvm
// target) refuses to add `dllexport` to a previously-declared symbol,
// so our forward decl must carry the same attribute up front. mingw-gcc
// (windows/amd64) is lenient about this; clang is not.
#if defined(_WIN32)
extern __declspec(dllexport) char* goDatalogicOpTrampoline(char* args_json, void* user_data, char** error_out);
#else
extern char* goDatalogicOpTrampoline(char* args_json, void* user_data, char** error_out);
#endif

// cgo treats `datalogic_op_callback` (a Rust `Option<fn ptr>`) as an
// opaque pointer-sized type from Go; wrap the function-pointer return
// in a tiny helper so we don't have to construct the typedef value in
// Go directly.
static datalogic_op_callback datalogic_go_get_trampoline(void) {
    // The cgo-generated declaration uses non-const `char*` while the
    // C ABI typedef uses `const char*`. The trampoline never writes
    // through the pointer, so the cast is safe; the compiler just
    // can't see that from the signature.
    return (datalogic_op_callback)goDatalogicOpTrampoline;
}
*/
import "C"

import (
	"errors"
	"runtime"
	"runtime/cgo"
	"unsafe"
)

// OperatorFunc is the contract for a custom operator. argsJSON is a
// JSON-array string of pre-evaluated arguments (e.g. `"[1, 2, \"x\"]"`).
// Return either:
//
//   - a JSON-value string and nil error (success), or
//   - any string and a non-nil error (error path); the error message
//     bubbles back to the caller as part of the evaluation error.
type OperatorFunc func(argsJSON string) (string, error)

// handleBox wraps a cgo.Handle inside an addressable struct so we can
// pass a real Go heap pointer through `void* user_data` instead of
// coercing the handle's `uintptr` value into an `unsafe.Pointer` — the
// latter trips `go vet`'s `unsafeptr` check because the integer-to-
// pointer conversion is indistinguishable from a synthesised pointer.
// The trampoline recovers the `cgo.Handle` via a normal pointer cast.
type handleBox struct {
	h cgo.Handle
}

// EngineBuilder accumulates engine configuration. Call Build to produce
// an Engine; the builder is consumed in the process.
//
// Builders are NOT goroutine-safe — construct from a single goroutine
// and call Build before sharing the resulting Engine.
type EngineBuilder struct {
	ptr     *C.datalogic_engine_builder
	handles []*handleBox // freed when the consuming Engine is closed
}

// NewEngineBuilder creates a fresh, empty builder.
func NewEngineBuilder() *EngineBuilder {
	return &EngineBuilder{ptr: C.datalogic_engine_builder_new()}
}

// Templating toggles the engine's templating mode (multi-key objects
// in compiled rules become output-shaping templates). Mirrors
// NewTemplatingEngine on the simple constructor path.
func (b *EngineBuilder) Templating(on bool) *EngineBuilder {
	var v C.int32_t
	if on {
		v = 1
	}
	C.datalogic_engine_builder_set_templating(b.ptr, v)
	return b
}

// SetConfigJSON sets the engine's evaluation configuration from a JSON
// object string, parsed by the core crate's shared config parser (the
// same wire format every binding uses). All keys are optional; an
// optional "preset" ("default", "safe_arithmetic", or "strict") selects
// the starting point and the remaining keys override individual fields
// on top of it:
//
//   - arithmetic_nan_handling: "throw_error" | "ignore_value" |
//     "coerce_to_zero" | "return_null"
//   - division_by_zero: "return_saturated" | "throw_error" |
//     "return_null" | "return_infinity"
//   - loose_equality_errors: bool
//   - truthy_evaluator: "javascript" | "python" | "strict_boolean"
//   - numeric_coercion: object with bool keys empty_string_to_zero,
//     null_to_zero, bool_to_number, reject_non_numeric
//   - max_recursion_depth: integer >= 1
//
// Unknown keys, unknown enum strings, and type mismatches are rejected
// with a *Error (Type "ConfigurationError") so typos fail loudly
// instead of being silently ignored. Each call replaces the builder's
// entire evaluation config; templating and registered operators are
// unaffected.
func (b *EngineBuilder) SetConfigJSON(configJSON string) error {
	cConfig := C.CString(configJSON)
	defer C.free(unsafe.Pointer(cConfig))
	// Keep the call and the thread-local last-error read on one OS thread.
	runtime.LockOSThread()
	rc := C.datalogic_engine_builder_set_config_json(b.ptr, cConfig)
	var err error
	if rc != 0 {
		err = lastError()
	}
	runtime.UnlockOSThread()
	if rc != 0 {
		return err
	}
	return nil
}

// AddOperator registers a custom JSONLogic operator under `name`.
// Registering a name that collides with a built-in (`+`, `if`, `var`,
// …) silently does nothing at evaluation time — the built-in dispatches
// first. Multiple calls with the same name overwrite the prior
// registration.
//
// The callback is held by the resulting Engine; it stays alive until
// Engine.Close.
func (b *EngineBuilder) AddOperator(name string, fn OperatorFunc) *EngineBuilder {
	cName := C.CString(name)
	defer C.free(unsafe.Pointer(cName))
	hb := &handleBox{h: cgo.NewHandle(fn)}
	b.handles = append(b.handles, hb)
	C.datalogic_engine_builder_add_operator(
		b.ptr,
		cName,
		C.datalogic_go_get_trampoline(),
		unsafe.Pointer(hb),
	)
	return b
}

// Build consumes the builder and returns a configured Engine. Calling
// the builder after Build is a no-op (Build is idempotent in that
// subsequent calls return nil + an error from the underlying C API).
func (b *EngineBuilder) Build() (*Engine, error) {
	ePtr := C.datalogic_engine_builder_build(b.ptr)
	if ePtr == nil {
		// Reclaim handles — the engine never picked them up.
		for _, hb := range b.handles {
			hb.h.Delete()
		}
		b.handles = nil
		C.datalogic_engine_builder_free(b.ptr)
		b.ptr = nil
		return nil, lastError()
	}
	C.datalogic_engine_builder_free(b.ptr)
	handles := b.handles
	b.ptr = nil
	b.handles = nil
	e := &Engine{ptr: ePtr, opHandles: handles}
	runtime.SetFinalizer(e, (*Engine).Close)
	return e, nil
}

//export goDatalogicOpTrampoline
func goDatalogicOpTrampoline(argsJSON *C.char, userData unsafe.Pointer, errorOut **C.char) *C.char {
	hb := (*handleBox)(userData)
	fn, ok := hb.h.Value().(OperatorFunc)
	if !ok {
		if errorOut != nil {
			*errorOut = C.CString("internal: operator handle had wrong type")
		}
		return nil
	}
	args := C.GoString(argsJSON)
	// Recover panics so we don't unwind across the cgo boundary.
	var (
		result string
		err    error
	)
	func() {
		defer func() {
			if r := recover(); r != nil {
				err = errors.New("panic in custom operator")
			}
		}()
		result, err = fn(args)
	}()
	if err != nil {
		if errorOut != nil {
			*errorOut = C.CString(err.Error())
		}
		return nil
	}
	return C.CString(result)
}
