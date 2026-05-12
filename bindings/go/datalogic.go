// Package datalogic provides Go bindings for the datalogic-rs JSONLogic
// engine.
//
// The binding routes through the shared C ABI in bindings/c/ and links
// libdatalogic_c.a statically. Run `make build` once after cloning to
// produce the static library that cgo links against.
//
// # Quick start
//
//	result, err := datalogic.Apply(`{"+":[1,2]}`, `{}`)
//	// result == "3"
//
// # Reusing a compiled rule
//
//	e := datalogic.NewEngine()
//	defer e.Close()
//	rule, err := e.Compile(`{"var":"x"}`)
//	defer rule.Close()
//	out, err := rule.Evaluate(`{"x":42}`)  // "42"
//
// # Hot-loop session (arena reuse)
//
//	s := e.Session()
//	defer s.Close()
//	for _, d := range inputs {
//	    out, _ := s.Evaluate(rule, d)
//	    _ = out
//	}
//
// # Threading
//
// Engine and Rule are safe to share across goroutines. Session is NOT —
// each goroutine that wants the hot-loop arena should open its own
// Session via Engine.Session().
package datalogic

// The cgo LDFLAGS for linking libdatalogic_c.a live in per-platform
// files (cgo_{os}_{arch}.go) gated by //go:build tags, so the right
// static library and system libraries are picked up automatically. See
// those files for the supported (os, arch) matrix.

/*
#cgo CFLAGS: -I${SRCDIR}/include

#include <stdlib.h>
#include "datalogic.h"
*/
import "C"

import (
	"runtime"
	"runtime/cgo"
	"unsafe"
)

// Engine is a JSONLogic compile/evaluate engine.
//
// Construct one with NewEngine (no custom operators) or NewEngineBuilder
// (custom operators) and reuse it for the lifetime of the program —
// Engine caches no per-call state and is safe to share across
// goroutines. Close it explicitly when done (or rely on the GC
// finalizer, which is best-effort).
type Engine struct {
	ptr *C.datalogic_engine
	// opHandles retains cgo.Handle references for every registered
	// custom operator so the trampoline can still resolve them during
	// evaluation. Released on Close.
	opHandles []cgo.Handle
}

// NewEngine constructs an engine with default configuration.
func NewEngine() *Engine {
	return newEngine(0)
}

// NewTemplatingEngine constructs an engine with templating mode enabled.
// In templating mode, multi-key objects in a compiled rule become
// output-shaping templates rather than parse errors.
func NewTemplatingEngine() *Engine {
	return newEngine(1)
}

func newEngine(templating C.int32_t) *Engine {
	e := &Engine{ptr: C.datalogic_engine_new(templating)}
	// Finalizer is best-effort cleanup for callers who forget Close.
	// Explicit Close clears the finalizer so we never double-free.
	runtime.SetFinalizer(e, (*Engine).Close)
	return e
}

// Close releases the underlying engine handle. Safe to call multiple
// times. Any Rule or Session derived from this Engine continues to work
// after Close — they hold their own refcount on the underlying engine.
func (e *Engine) Close() {
	if e == nil || e.ptr == nil {
		return
	}
	C.datalogic_engine_free(e.ptr)
	e.ptr = nil
	for _, h := range e.opHandles {
		h.Delete()
	}
	e.opHandles = nil
	runtime.SetFinalizer(e, nil)
}

// Compile parses a JSONLogic rule (as a JSON string) into a reusable
// Rule that can be evaluated against many data inputs without re-parsing.
func (e *Engine) Compile(ruleJSON string) (*Rule, error) {
	cRule := C.CString(ruleJSON)
	defer C.free(unsafe.Pointer(cRule))
	ptr := C.datalogic_engine_compile(e.ptr, cRule)
	runtime.KeepAlive(e)
	if ptr == nil {
		return nil, lastError()
	}
	r := &Rule{ptr: ptr}
	runtime.SetFinalizer(r, (*Rule).Close)
	return r, nil
}

// Apply compiles ruleJSON and evaluates it against dataJSON in one call,
// returning the result as a JSON string.
//
// For repeated evaluations of the same rule, prefer Compile + Rule.Evaluate
// — Apply re-parses the rule on every call.
func (e *Engine) Apply(ruleJSON, dataJSON string) (string, error) {
	cRule := C.CString(ruleJSON)
	defer C.free(unsafe.Pointer(cRule))
	cData := C.CString(dataJSON)
	defer C.free(unsafe.Pointer(cData))
	out := C.datalogic_engine_apply(e.ptr, cRule, cData)
	runtime.KeepAlive(e)
	if out == nil {
		return "", lastError()
	}
	defer C.datalogic_string_free(out)
	return C.GoString(out), nil
}

// Session opens a hot-loop session bound to this engine. The session
// reuses one bumpalo arena across evaluations and resets it at the
// start of every call to bound peak memory.
//
// Sessions are NOT goroutine-safe — open one per goroutine that needs it.
func (e *Engine) Session() *Session {
	s := &Session{ptr: C.datalogic_engine_session(e.ptr)}
	runtime.KeepAlive(e)
	runtime.SetFinalizer(s, (*Session).Close)
	return s
}

// Rule is a compiled JSONLogic rule.
//
// Rules are safe to share across goroutines — each Evaluate call uses
// its own short-lived arena. For tight loops, use a Session per
// goroutine instead.
type Rule struct {
	ptr *C.datalogic_rule
}

// Close releases the rule handle. Safe to call multiple times.
func (r *Rule) Close() {
	if r == nil || r.ptr == nil {
		return
	}
	C.datalogic_rule_free(r.ptr)
	r.ptr = nil
	runtime.SetFinalizer(r, nil)
}

// Evaluate runs the compiled rule against dataJSON and returns the
// result as a JSON string.
func (r *Rule) Evaluate(dataJSON string) (string, error) {
	cData := C.CString(dataJSON)
	defer C.free(unsafe.Pointer(cData))
	out := C.datalogic_rule_evaluate(r.ptr, cData)
	runtime.KeepAlive(r)
	if out == nil {
		return "", lastError()
	}
	defer C.datalogic_string_free(out)
	return C.GoString(out), nil
}

// Session is a hot-loop evaluation session bound to one Engine.
//
// Sessions reuse a single bumpalo arena across Evaluate calls; the
// arena resets at the start of every call so peak memory stays bounded.
// Sessions are NOT goroutine-safe.
type Session struct {
	ptr *C.datalogic_session
}

// Close releases the session handle. Safe to call multiple times.
func (s *Session) Close() {
	if s == nil || s.ptr == nil {
		return
	}
	C.datalogic_session_free(s.ptr)
	s.ptr = nil
	runtime.SetFinalizer(s, nil)
}

// Evaluate runs rule against dataJSON using this session's arena.
func (s *Session) Evaluate(rule *Rule, dataJSON string) (string, error) {
	cData := C.CString(dataJSON)
	defer C.free(unsafe.Pointer(cData))
	out := C.datalogic_session_evaluate(s.ptr, rule.ptr, cData)
	runtime.KeepAlive(s)
	runtime.KeepAlive(rule)
	if out == nil {
		return "", lastError()
	}
	defer C.datalogic_string_free(out)
	return C.GoString(out), nil
}

// Reset manually resets the session's arena. Optional — Evaluate already
// resets at the start of every call. Exposed for consumers that want to
// release memory between long idle periods.
func (s *Session) Reset() {
	C.datalogic_session_reset(s.ptr)
	runtime.KeepAlive(s)
}

// AllocatedBytes returns the bytes currently held by the session's arena
// (sum across all chunks). Useful for sizing and diagnostics.
func (s *Session) AllocatedBytes() uint64 {
	n := uint64(C.datalogic_session_allocated_bytes(s.ptr))
	runtime.KeepAlive(s)
	return n
}

// Apply is a top-level convenience equivalent to:
//
//	e := NewEngine(); defer e.Close(); return e.Apply(rule, data)
//
// Use it for ad-hoc one-shots; for repeated evaluations, hold an Engine.
func Apply(ruleJSON, dataJSON string) (string, error) {
	e := NewEngine()
	defer e.Close()
	return e.Apply(ruleJSON, dataJSON)
}

// Version returns the binding's version string (sourced from the
// underlying C ABI, which tracks datalogic-rs exactly).
func Version() string {
	return C.GoString(C.datalogic_version())
}
