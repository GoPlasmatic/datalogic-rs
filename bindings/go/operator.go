package datalogic

// Custom operator support via the C-ABI builder. Routes Go callbacks
// through a C trampoline; the user_data slot points at a C-allocated box
// holding a `cgo.Handle`, so we can fan out to many distinct Go closures
// per engine.
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
	"encoding/json"
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

// opRegistry owns the host-side state the native trampoline calls into:
// one `cgo.Handle` per registered operator, plus a C-allocated box (a
// single uintptr_t holding the handle value) that rides across the
// boundary as the callback's user_data. The boxes live in C memory
// because C retains user_data past the registering call, which the cgo
// pointer-passing rules forbid for Go pointers; and passing the box,
// rather than coercing the handle's integer into an `unsafe.Pointer`,
// keeps `go vet`'s `unsafeptr` check quiet.
//
// The native engine is refcounted: every Rule, Session and
// TracedSession holds its own reference and keeps dispatching into
// these callbacks after Engine.Close. So the Engine and everything
// derived from it point at the registry, and only the registry's own
// finalizer frees the handles and boxes, once the last of them is
// unreachable. Go runs finalizers in dependency order, so a Rule's
// finalizer releases the native rule before the registry it points at
// is finalized.
type opRegistry struct {
	handles []cgo.Handle
	boxes   []unsafe.Pointer // C memory, each holding one handle
}

// newOpRegistry allocates an empty registry with its cleanup finalizer
// attached.
func newOpRegistry() *opRegistry {
	r := &opRegistry{}
	runtime.SetFinalizer(r, (*opRegistry).free)
	return r
}

// add wraps fn in a fresh handle and returns the C box carrying it.
func (r *opRegistry) add(fn OperatorFunc) unsafe.Pointer {
	h := cgo.NewHandle(fn)
	box := C.malloc(C.size_t(unsafe.Sizeof(C.uintptr_t(0))))
	*(*C.uintptr_t)(box) = C.uintptr_t(h)
	r.handles = append(r.handles, h)
	r.boxes = append(r.boxes, box)
	return box
}

// free deletes every handle and releases every box. Runs as the
// registry's finalizer, or directly on the builder failure paths where
// no engine ever picked the callbacks up.
func (r *opRegistry) free() {
	for _, h := range r.handles {
		h.Delete()
	}
	for _, box := range r.boxes {
		C.free(box)
	}
	r.handles = nil
	r.boxes = nil
	runtime.SetFinalizer(r, nil)
}

// EngineBuilder accumulates engine configuration. Call Build to produce
// an Engine; the builder is consumed in the process.
//
// Builders are NOT goroutine-safe — construct from a single goroutine
// and call Build before sharing the resulting Engine.
type EngineBuilder struct {
	ptr *C.datalogic_engine_builder
	reg *opRegistry // callback state, handed to the Engine by Build
	err error       // first registration error; surfaced by Build
}

// fail records err as the builder's error unless one is already recorded.
func (b *EngineBuilder) fail(err error) {
	if b.err == nil {
		b.err = err
	}
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

// TemplateKeyEscape sets the character that marks a template key as a
// literal output field rather than an operator call: with '$',
// {"$type": ...} emits the key "type". Only meaningful with Templating.
func (b *EngineBuilder) TemplateKeyEscape(r rune) *EngineBuilder {
	var cerr *C.datalogic_error
	rc := C.datalogic_engine_builder_set_template_key_escape(b.ptr, C.uint32_t(r), &cerr)
	if rc != C.DATALOGIC_STATUS_OK {
		b.fail(takeError(cerr))
	}
	return b
}

// Families keeps the engine to the JSONLogic core and the operator
// families named here ("ExtString", "DateTime", ...: the "family" of each
// row of Engine.Operators). By default the engine has every family. A
// family left out is not there for the engine: its names compile as
// unknown operators, and a custom operator may take them. Call it before
// AddOperator when StrictOperatorNames is on. An unknown family name fails
// Build with Type "ConfigurationError".
func (b *EngineBuilder) Families(names ...string) *EngineBuilder {
	if names == nil {
		names = []string{}
	}
	js, err := json.Marshal(names)
	if err != nil {
		b.fail(err)
		return b
	}
	cp, cl := strBytes(string(js))
	var cerr *C.datalogic_error
	rc := C.datalogic_engine_builder_set_families(b.ptr, cp, cl, &cerr)
	if rc != C.DATALOGIC_STATUS_OK {
		b.fail(takeError(cerr))
	}
	return b
}

// StrictOperatorNames makes a later AddOperator with a name a built-in
// answers to (`length`, `var`, an alias such as `?:`) fail Build with
// Type "ConfigurationError" instead of registering an operator that
// would never run. Call it before AddOperator.
func (b *EngineBuilder) StrictOperatorNames(on bool) *EngineBuilder {
	var v C.int32_t
	if on {
		v = 1
	}
	C.datalogic_engine_builder_set_strict_operator_names(b.ptr, v)
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
//   - ops_budget: integer >= 1, or null for unbounded (the default)
//
// ops_budget is how a caller bounds the work a rule may do: a ceiling on
// the operations one evaluation may charge — one per node the engine
// dispatches, one per item an iterator walks, plus what operators charge
// for the data they move. Crossing it fails the evaluation with Type
// "BudgetExceeded" before the work is done, and a `try` in the rule
// cannot recover from it. Unlike a wall-clock timeout the count is
// deterministic, so the same rule and data are refused on every machine.
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
// The callback stays alive while the resulting Engine, or any Rule,
// Session or TracedSession derived from it, is reachable — Engine.Close
// alone does not release it. A failed registration (e.g. a name that is
// not valid UTF-8) is remembered and surfaced by Build.
func (b *EngineBuilder) AddOperator(name string, fn OperatorFunc) *EngineBuilder {
	np, nl := strBytes(name)
	if b.reg == nil {
		b.reg = newOpRegistry()
	}
	box := b.reg.add(fn)
	var cerr *C.datalogic_error
	rc := C.datalogic_engine_builder_add_operator(
		b.ptr,
		np, nl,
		C.datalogic_go_get_trampoline(),
		box,
		&cerr,
	)
	if rc != C.DATALOGIC_STATUS_OK {
		b.fail(takeError(cerr))
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
	reg := b.reg
	b.ptr = nil
	b.reg = nil
	e := &Engine{ptr: ePtr, reg: reg}
	runtime.SetFinalizer(e, (*Engine).Close)
	return e, nil
}

// release frees the native builder and reclaims callback handles the
// engine never picked up. Used on the Build failure paths.
func (b *EngineBuilder) release() {
	C.datalogic_engine_builder_free(b.ptr)
	b.ptr = nil
	if b.reg != nil {
		b.reg.free()
		b.reg = nil
	}
}

//export goDatalogicOpTrampoline
func goDatalogicOpTrampoline(argsJSON *C.uint8_t, argsLen C.size_t, userData unsafe.Pointer, out *C.datalogic_op_result) C.int32_t {
	args := goStringN(argsJSON, argsLen)
	// Recover panics so we don't unwind across the cgo boundary. The
	// handle lookup sits inside too: Handle.Value panics on a deleted
	// handle, which must surface as an operator error, not a crash.
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
		h := cgo.Handle(*(*C.uintptr_t)(userData))
		fn, ok := h.Value().(OperatorFunc)
		if !ok {
			err = errors.New("internal: operator handle had wrong type")
			return
		}
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
