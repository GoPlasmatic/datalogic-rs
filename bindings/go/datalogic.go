// Package datalogic implements a JSONLogic (json-logic) rules engine for
// Go, backed by the datalogic-rs native Rust core.
//
// One engine, identical semantics across Go, Node.js, WASM, Python, Java,
// .NET, PHP, and Rust: the binding routes through the shared C ABI (v2) in
// bindings/c/ and links libdatalogic_c.a statically. Run `make build` once
// after cloning to produce the static library that cgo links against.
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
// # Parsed data handles (parse once, evaluate many)
//
//	data, err := datalogic.ParseData(`{"x":42}`)
//	defer data.Close()
//	out, _ := s.EvaluateData(rule, data)     // zero parse work per call
//	ok, _ := s.EvaluateBool(boolRule, data)  // typed result, no JSON decode
//
// # Threading
//
// Engine, Rule, and DataHandle are safe to share across goroutines.
// Session is NOT — each goroutine that wants the hot-loop arena should
// open its own Session via Engine.Session().
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
	"fmt"
	"runtime"
	"unsafe"
)

// init asserts that the linked native library speaks C ABI v2. A
// mismatch means a stale libdatalogic_c.a was staged next to a newer
// binding (or vice versa) — fail loudly at load time instead of
// corrupting memory at call time.
func init() {
	if v := uint32(C.datalogic_abi_version()); v != uint32(C.DATALOGIC_ABI_VERSION) {
		panic(fmt.Sprintf(
			"datalogic: native libdatalogic_c reports ABI v%d, this binding requires v%d — restage the static library (run `make build` in bindings/go)",
			v, uint32(C.DATALOGIC_ABI_VERSION)))
	}
}

// =============== (ptr, len) marshalling helpers ===============

// strBytes exposes a Go string's bytes to C as a (pointer, length) pair
// — zero-copy. This is legal under the cgo pointer-passing rules: the
// pointer is only ever handed to C as a call argument (the Go memory is
// pinned for the duration of that call), and the v2 ABI never retains
// input pointers past the call. Empty strings pass (nil, 0), which the
// ABI documents as the empty string (unsafe.StringData is unspecified
// for empty strings).
func strBytes(s string) (*C.uint8_t, C.size_t) {
	if len(s) == 0 {
		return nil, 0
	}
	return (*C.uint8_t)(unsafe.Pointer(unsafe.StringData(s))), C.size_t(len(s))
}

// goStringN copies a borrowed (ptr, len) UTF-8 byte range from C into
// an owned Go string. Callers must invoke it before the borrow dies
// (for session results: before the next call touching that session).
func goStringN(ptr *C.uint8_t, n C.size_t) string {
	if n == 0 {
		return ""
	}
	return string(unsafe.Slice((*byte)(unsafe.Pointer(ptr)), int(n)))
}

// takeBuf copies an owned datalogic_buf into a Go string and releases
// the native allocation.
func takeBuf(buf C.datalogic_buf) string {
	s := goStringN(buf.ptr, buf.len)
	C.datalogic_buf_free(buf)
	return s
}

// =============== Engine ===============

// Engine is a JSONLogic compile/evaluate engine.
//
// Construct one with NewEngine (no custom operators) or NewEngineBuilder
// (custom operators) and reuse it for the lifetime of the program —
// Engine caches no per-call state and is safe to share across
// goroutines. Close it explicitly when done (or rely on the GC
// finalizer, which is best-effort).
type Engine struct {
	ptr *C.datalogic_engine
	// opHandles retains heap-allocated `cgo.Handle` boxes for every
	// registered custom operator so the trampoline can still resolve
	// them during evaluation. Released on Close.
	opHandles []*handleBox
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
	for _, hb := range e.opHandles {
		hb.h.Delete()
	}
	e.opHandles = nil
	runtime.SetFinalizer(e, nil)
}

// Compile parses a JSONLogic rule (as a JSON string) into a reusable
// Rule that can be evaluated against many data inputs without re-parsing.
func (e *Engine) Compile(ruleJSON string) (*Rule, error) {
	rp, rl := strBytes(ruleJSON)
	var rulePtr *C.datalogic_rule
	var cerr *C.datalogic_error
	rc := C.datalogic_engine_compile(e.ptr, rp, rl, &rulePtr, &cerr)
	runtime.KeepAlive(e)
	if rc != C.DATALOGIC_STATUS_OK {
		return nil, takeError(cerr)
	}
	r := &Rule{ptr: rulePtr}
	runtime.SetFinalizer(r, (*Rule).Close)
	return r, nil
}

// Apply compiles ruleJSON and evaluates it against dataJSON in one call,
// returning the result as a JSON string.
//
// For repeated evaluations of the same rule, prefer Compile + Rule.Evaluate
// — Apply re-parses the rule on every call.
func (e *Engine) Apply(ruleJSON, dataJSON string) (string, error) {
	rp, rl := strBytes(ruleJSON)
	dp, dl := strBytes(dataJSON)
	var out C.datalogic_buf
	var cerr *C.datalogic_error
	rc := C.datalogic_engine_apply(e.ptr, rp, rl, dp, dl, &out, &cerr)
	runtime.KeepAlive(e)
	if rc != C.DATALOGIC_STATUS_OK {
		return "", takeError(cerr)
	}
	return takeBuf(out), nil
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

// =============== Rule ===============

// Rule is a compiled JSONLogic rule.
//
// Rules are safe to share across goroutines — each Evaluate call uses
// its own short-lived arena. For tight loops, use a Session per
// goroutine instead.
//
// A Rule can only be evaluated through a Session opened on the same
// Engine that compiled it; mixing engines yields an InvalidArgument
// error.
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
// result as a JSON string. Safe to call from multiple goroutines.
func (r *Rule) Evaluate(dataJSON string) (string, error) {
	dp, dl := strBytes(dataJSON)
	var out C.datalogic_buf
	var cerr *C.datalogic_error
	rc := C.datalogic_rule_evaluate(r.ptr, dp, dl, &out, &cerr)
	runtime.KeepAlive(r)
	if rc != C.DATALOGIC_STATUS_OK {
		return "", takeError(cerr)
	}
	return takeBuf(out), nil
}

// =============== Session ===============

// Session is a hot-loop evaluation session bound to one Engine.
//
// Sessions reuse a single bumpalo arena across Evaluate calls; the
// arena resets at the start of every call so peak memory stays bounded.
// Sessions are NOT goroutine-safe — open one per goroutine.
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
//
// The rule must have been compiled by the same Engine this session was
// opened on.
func (s *Session) Evaluate(rule *Rule, dataJSON string) (string, error) {
	dp, dl := strBytes(dataJSON)
	var outPtr *C.uint8_t
	var outLen C.size_t
	var cerr *C.datalogic_error
	rc := C.datalogic_session_evaluate(s.ptr, rule.ptr, dp, dl, &outPtr, &outLen, &cerr)
	if rc != C.DATALOGIC_STATUS_OK {
		runtime.KeepAlive(s)
		runtime.KeepAlive(rule)
		return "", takeError(cerr)
	}
	// The result borrows the session's buffer (valid until the next
	// call touching this session) — copy before anything else runs.
	out := goStringN(outPtr, outLen)
	runtime.KeepAlive(s)
	runtime.KeepAlive(rule)
	return out, nil
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

// =============== TracedSession ===============

// TracedSession is a trace-enabled evaluation handle bound to one
// Engine. Each Evaluate call compiles the rule internally with the
// optimizer disabled, so every operator in the rule surfaces as an
// execution step, and returns a JSON envelope instead of a bare result:
//
//	{
//	  "result": <value, or null on error>,
//	  "expression_tree": <compiled expression node>,
//	  "steps": [<execution step>, ...],
//	  "error": "<message>",              // only present on engine errors
//	  "structured_error": {<Error>}      // only present on engine errors
//	}
//
// The envelope matches the WASM binding's wire format, so trace
// consumers (debuggers, visualizers) see one shape across every
// language. Engine errors (rule parse or evaluation failures) surface
// inside the envelope's "error"/"structured_error" fields with a null
// "result" — the Go error return is reserved for binding-level
// failures such as invalid handles.
//
// TracedSessions hold their own reference on the underlying engine and
// keep working even if the Engine is closed first. Unlike Session,
// TracedSession is safe to share across goroutines — every Evaluate
// uses a fresh internal arena.
type TracedSession struct {
	ptr *C.datalogic_traced_session
}

// TracedSession opens a trace-enabled session bound to this engine.
// Every Evaluate call returns the JSON trace envelope documented on
// TracedSession.
//
// Tracing pays for compile-per-call plus step recording — use it for
// debugging and tooling, not hot paths.
func (e *Engine) TracedSession() *TracedSession {
	ts := &TracedSession{ptr: C.datalogic_engine_traced_session(e.ptr)}
	runtime.KeepAlive(e)
	runtime.SetFinalizer(ts, (*TracedSession).Close)
	return ts
}

// Close releases the traced-session handle. Safe to call multiple times.
func (ts *TracedSession) Close() {
	if ts == nil || ts.ptr == nil {
		return
	}
	C.datalogic_traced_session_free(ts.ptr)
	ts.ptr = nil
	runtime.SetFinalizer(ts, nil)
}

// Evaluate compiles ruleJSON (optimizer disabled), evaluates it against
// dataJSON, and returns the trace envelope documented on TracedSession
// as a JSON string.
func (ts *TracedSession) Evaluate(ruleJSON, dataJSON string) (string, error) {
	rp, rl := strBytes(ruleJSON)
	dp, dl := strBytes(dataJSON)
	var out C.datalogic_buf
	var cerr *C.datalogic_error
	rc := C.datalogic_traced_session_evaluate(ts.ptr, rp, rl, dp, dl, &out, &cerr)
	runtime.KeepAlive(ts)
	if rc != C.DATALOGIC_STATUS_OK {
		return "", takeError(cerr)
	}
	return takeBuf(out), nil
}

// =============== package-level conveniences ===============

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
