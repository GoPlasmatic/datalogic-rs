package datalogic

// Custom operator support via the C-ABI builder. Routes Go callbacks
// through a C trampoline; the user_data slot carries a `cgo.Handle` so
// we can fan out to many distinct Go closures per engine.
//
// v2 callback contract: the trampoline receives the pre-evaluated
// arguments as a borrowed (ptr, len) JSON-array byte range, writes its
// outcome through datalogic_op_result_set_json / _set_error (both copy
// immediately, so Go string bytes can be passed zero-copy), and returns
// 0 for success / non-zero for failure. No allocator crosses the
// boundary in either direction.
//
// Threading note: the engine may invoke a registered operator from any
// goroutine/thread that calls Engine.Apply / Rule.Evaluate /
// Session.Evaluate. Go callbacks themselves are goroutine-safe (the
// runtime serialises the cgo crossing); user code inside the callback
// is responsible for its own synchronisation.

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
extern __declspec(dllexport) int32_t goDatalogicOpTrampoline(uint8_t* args_json, size_t args_len, void* user_data, datalogic_op_result* out);
#else
extern int32_t goDatalogicOpTrampoline(uint8_t* args_json, size_t args_len, void* user_data, datalogic_op_result* out);
#endif

// cgo can't construct a C function-pointer value directly from Go, so
// wrap the trampoline address in a tiny helper returning the ABI's
// callback typedef.
static datalogic_op_fn datalogic_go_get_trampoline(void) {
    // The cgo-generated declaration uses non-const `uint8_t*` while the
    // C ABI typedef uses `const uint8_t*`. The trampoline never writes
    // through the pointer, so the cast is safe; the compiler just
    // can't see that from the signature.
    return (datalogic_op_fn)goDatalogicOpTrampoline;
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
	err     error        // first registration error; surfaced by Build
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
	cp, cl := strBytes(configJSON)
	var cerr *C.datalogic_error
	rc := C.datalogic_engine_builder_set_config_json(b.ptr, cp, cl, &cerr)
	if rc != C.DATALOGIC_STATUS_OK {
		return takeError(cerr)
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
// Engine.Close. A failed registration (e.g. a name that is not valid
// UTF-8) is remembered and surfaced by Build.
func (b *EngineBuilder) AddOperator(name string, fn OperatorFunc) *EngineBuilder {
	np, nl := strBytes(name)
	hb := &handleBox{h: cgo.NewHandle(fn)}
	b.handles = append(b.handles, hb)
	var cerr *C.datalogic_error
	rc := C.datalogic_engine_builder_add_operator(
		b.ptr,
		np, nl,
		C.datalogic_go_get_trampoline(),
		unsafe.Pointer(hb),
		&cerr,
	)
	if rc != C.DATALOGIC_STATUS_OK {
		err := takeError(cerr)
		if b.err == nil {
			b.err = err
		}
	}
	return b
}

// Build consumes the builder and returns a configured Engine. Calling
// the builder after Build is a no-op (Build is idempotent in that
// subsequent calls return nil + an error).
func (b *EngineBuilder) Build() (*Engine, error) {
	if b.err != nil {
		err := b.err
		b.err = nil
		b.release()
		return nil, err
	}
	ePtr := C.datalogic_engine_builder_build(b.ptr)
	if ePtr == nil {
		// v2 returns NULL only for a nil or already-drained builder —
		// there is no error handle to read, so synthesise one.
		b.release()
		return nil, &Error{
			Message: "engine builder is nil or was already built",
			Type:    "InvalidArgument",
		}
	}
	C.datalogic_engine_builder_free(b.ptr)
	handles := b.handles
	b.ptr = nil
	b.handles = nil
	e := &Engine{ptr: ePtr, opHandles: handles}
	runtime.SetFinalizer(e, (*Engine).Close)
	return e, nil
}

// release frees the native builder and reclaims callback handles the
// engine never picked up. Used on the Build failure paths.
func (b *EngineBuilder) release() {
	for _, hb := range b.handles {
		hb.h.Delete()
	}
	b.handles = nil
	C.datalogic_engine_builder_free(b.ptr)
	b.ptr = nil
}

//export goDatalogicOpTrampoline
func goDatalogicOpTrampoline(argsJSON *C.uint8_t, argsLen C.size_t, userData unsafe.Pointer, out *C.datalogic_op_result) C.int32_t {
	hb := (*handleBox)(userData)
	fn, ok := hb.h.Value().(OperatorFunc)
	if !ok {
		setOpError(out, "internal: operator handle had wrong type")
		return 1
	}
	args := goStringN(argsJSON, argsLen)
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
		setOpError(out, err.Error())
		return 1
	}
	p, n := strBytes(result)
	C.datalogic_op_result_set_json(out, p, n)
	return 0
}

// setOpError writes msg through datalogic_op_result_set_error, which
// copies immediately — the Go string bytes only need to live for the
// duration of the call.
func setOpError(out *C.datalogic_op_result, msg string) {
	p, n := strBytes(msg)
	C.datalogic_op_result_set_error(out, p, n)
}
