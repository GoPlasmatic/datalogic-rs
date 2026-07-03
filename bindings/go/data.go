package datalogic

// Parsed-data handles plus the evaluation tiers built on them: the
// data-handle hot path (parse once, evaluate many), typed scalar
// results, and the batch entry points. All new in C ABI v2.

/*
#cgo CFLAGS: -I${SRCDIR}/include

#include "datalogic.h"
*/
import "C"

import (
	"encoding/json"
	"runtime"
)

// =============== DataHandle ===============

// DataHandle is an immutable, pre-parsed JSON document.
//
// Parsing data once and evaluating it many times skips the per-call
// JSON parse that dominates string-based evaluation on larger payloads.
// A DataHandle is independent of any Engine — one handle can feed rules
// compiled by different engines — and is safe for concurrent use by
// multiple goroutines (evaluations only read it).
//
// Close it after the last evaluation that uses it (or rely on the GC
// finalizer, which is best-effort). Handles are not consumed by
// evaluation.
type DataHandle struct {
	ptr *C.datalogic_data
}

// ParseData parses a JSON document into a reusable DataHandle.
func ParseData(dataJSON string) (*DataHandle, error) {
	dp, dl := strBytes(dataJSON)
	var out *C.datalogic_data
	var cerr *C.datalogic_error
	rc := C.datalogic_data_parse(dp, dl, &out, &cerr)
	if rc != C.DATALOGIC_STATUS_OK {
		return nil, takeError(cerr)
	}
	d := &DataHandle{ptr: out}
	runtime.SetFinalizer(d, (*DataHandle).Close)
	return d, nil
}

// Close releases the data handle. Safe to call multiple times. Do not
// call while another goroutine is still evaluating against the handle.
func (d *DataHandle) Close() {
	if d == nil || d.ptr == nil {
		return
	}
	C.datalogic_data_free(d.ptr)
	d.ptr = nil
	runtime.SetFinalizer(d, nil)
}

// AllocatedBytes returns the bytes held by the handle's backing arena
// (input copy + parsed tree). Useful for sizing and diagnostics.
func (d *DataHandle) AllocatedBytes() uint64 {
	if d == nil || d.ptr == nil {
		return 0
	}
	n := uint64(C.datalogic_data_allocated_bytes(d.ptr))
	runtime.KeepAlive(d)
	return n
}

// cptr returns the underlying C handle, tolerating nil receivers (the
// C side reports a proper InvalidArgument error for NULL handles).
func (d *DataHandle) cptr() *C.datalogic_data {
	if d == nil {
		return nil
	}
	return d.ptr
}

// =============== data-handle evaluation ===============

// EvaluateData runs the compiled rule against a pre-parsed DataHandle
// and returns the result as a JSON string. Like Evaluate, it is safe to
// call from multiple goroutines.
func (r *Rule) EvaluateData(data *DataHandle) (string, error) {
	var out C.datalogic_buf
	var cerr *C.datalogic_error
	rc := C.datalogic_rule_evaluate_data(r.ptr, data.cptr(), &out, &cerr)
	runtime.KeepAlive(r)
	runtime.KeepAlive(data)
	if rc != C.DATALOGIC_STATUS_OK {
		return "", takeError(cerr)
	}
	return takeBuf(out), nil
}

// EvaluateData runs rule against a pre-parsed DataHandle using this
// session's arena — the hot path: zero parse work per call.
//
// The rule must have been compiled by the same Engine this session was
// opened on. Like every Session method, single-goroutine only.
func (s *Session) EvaluateData(rule *Rule, data *DataHandle) (string, error) {
	var outPtr *C.uint8_t
	var outLen C.size_t
	var cerr *C.datalogic_error
	rc := C.datalogic_session_evaluate_data(s.ptr, rule.ptr, data.cptr(), &outPtr, &outLen, &cerr)
	if rc != C.DATALOGIC_STATUS_OK {
		runtime.KeepAlive(s)
		runtime.KeepAlive(rule)
		runtime.KeepAlive(data)
		return "", takeError(cerr)
	}
	// Borrowed result — copy before anything else touches the session.
	out := goStringN(outPtr, outLen)
	runtime.KeepAlive(s)
	runtime.KeepAlive(rule)
	runtime.KeepAlive(data)
	return out, nil
}

// =============== typed scalar results ===============
//
// The typed evaluations take DataHandle input only (the predicate-heavy
// flows that want typed results are exactly the flows that parse data
// once) and skip JSON serialization entirely. On a result of the wrong
// type they return a *Error with Type "TypeMismatch".

// EvaluateBool evaluates rule and returns the result as a strict JSON
// boolean. Any other result type yields a TypeMismatch error; for
// JSONLogic truthiness coercion use EvaluateTruthy. Single-goroutine,
// like every Session method.
func (s *Session) EvaluateBool(rule *Rule, data *DataHandle) (bool, error) {
	var out C.int32_t
	var cerr *C.datalogic_error
	rc := C.datalogic_session_evaluate_bool(s.ptr, rule.ptr, data.cptr(), &out, &cerr)
	runtime.KeepAlive(s)
	runtime.KeepAlive(rule)
	runtime.KeepAlive(data)
	if rc != C.DATALOGIC_STATUS_OK {
		return false, takeError(cerr)
	}
	return out != 0, nil
}

// EvaluateInt64 evaluates rule and returns the result as an integer.
// A non-number or non-integer number yields a TypeMismatch error.
func (s *Session) EvaluateInt64(rule *Rule, data *DataHandle) (int64, error) {
	var out C.int64_t
	var cerr *C.datalogic_error
	rc := C.datalogic_session_evaluate_i64(s.ptr, rule.ptr, data.cptr(), &out, &cerr)
	runtime.KeepAlive(s)
	runtime.KeepAlive(rule)
	runtime.KeepAlive(data)
	if rc != C.DATALOGIC_STATUS_OK {
		return 0, takeError(cerr)
	}
	return int64(out), nil
}

// EvaluateFloat64 evaluates rule and returns the result as a float64.
// Accepts any JSON number; other types yield a TypeMismatch error.
func (s *Session) EvaluateFloat64(rule *Rule, data *DataHandle) (float64, error) {
	var out C.double
	var cerr *C.datalogic_error
	rc := C.datalogic_session_evaluate_f64(s.ptr, rule.ptr, data.cptr(), &out, &cerr)
	runtime.KeepAlive(s)
	runtime.KeepAlive(rule)
	runtime.KeepAlive(data)
	if rc != C.DATALOGIC_STATUS_OK {
		return 0, takeError(cerr)
	}
	return float64(out), nil
}

// EvaluateTruthy evaluates rule and collapses the result to a bool via
// the engine's configured truthiness rules (the same coercion `if`,
// `and`, and `or` apply). It never type-mismatches — any result
// truthy-converts.
func (s *Session) EvaluateTruthy(rule *Rule, data *DataHandle) (bool, error) {
	var out C.int32_t
	var cerr *C.datalogic_error
	rc := C.datalogic_session_evaluate_truthy(s.ptr, rule.ptr, data.cptr(), &out, &cerr)
	runtime.KeepAlive(s)
	runtime.KeepAlive(rule)
	runtime.KeepAlive(data)
	if rc != C.DATALOGIC_STATUS_OK {
		return false, takeError(cerr)
	}
	return out != 0, nil
}

// =============== batch evaluation ===============

// BatchResult is one item's outcome from Session.EvaluateBatch or
// Session.EvaluateMany. Exactly one side is meaningful: on success Err
// is nil and Value holds the result JSON; on failure Err is a *Error
// (carrying the engine's tag in Type) and Value is empty.
type BatchResult struct {
	// Value is the item's result as a JSON string; empty when Err is
	// non-nil.
	Value string
	// Err is the item's failure, if any. Item failures are independent
	// — one failing item never affects its neighbours.
	Err error
}

// EvaluateBatch evaluates one rule against many pre-parsed data handles
// in a single native call, returning one BatchResult per input in
// order. Per-item failures (including nil handles in the slice) land in
// the item's Err; the call-level error return covers argument problems
// only (nil session, rule from a different engine, …).
//
// Single-goroutine, like every Session method.
func (s *Session) EvaluateBatch(rule *Rule, datas []*DataHandle) ([]BatchResult, error) {
	n := len(datas)
	if n == 0 {
		return nil, nil
	}
	cDatas := make([]*C.datalogic_data, n)
	for i, d := range datas {
		cDatas[i] = d.cptr()
	}
	results := make([]C.datalogic_slice, n)
	statuses := make([]C.datalogic_status, n)
	var cerr *C.datalogic_error
	rc := C.datalogic_session_evaluate_batch(
		s.ptr, rule.ptr,
		&cDatas[0], C.size_t(n),
		&results[0], &statuses[0], &cerr,
	)
	if rc != C.DATALOGIC_STATUS_OK {
		runtime.KeepAlive(s)
		runtime.KeepAlive(rule)
		runtime.KeepAlive(datas)
		return nil, takeError(cerr)
	}
	out := collectBatch(results, statuses)
	runtime.KeepAlive(s)
	runtime.KeepAlive(rule)
	runtime.KeepAlive(datas)
	return out, nil
}

// EvaluateMany evaluates many rules against one pre-parsed data handle
// in a single native call — the rule-set / feature-flag shape —
// returning one BatchResult per rule in order. Per-item failures
// (including nil rules in the slice, or a rule compiled by a different
// engine) land in the item's Err; the call-level error return covers
// argument problems only.
//
// Single-goroutine, like every Session method.
func (s *Session) EvaluateMany(rules []*Rule, data *DataHandle) ([]BatchResult, error) {
	n := len(rules)
	if n == 0 {
		return nil, nil
	}
	cRules := make([]*C.datalogic_rule, n)
	for i, r := range rules {
		if r != nil {
			cRules[i] = r.ptr
		}
	}
	results := make([]C.datalogic_slice, n)
	statuses := make([]C.datalogic_status, n)
	var cerr *C.datalogic_error
	rc := C.datalogic_session_evaluate_many(
		s.ptr,
		&cRules[0], C.size_t(n),
		data.cptr(),
		&results[0], &statuses[0], &cerr,
	)
	if rc != C.DATALOGIC_STATUS_OK {
		runtime.KeepAlive(s)
		runtime.KeepAlive(rules)
		runtime.KeepAlive(data)
		return nil, takeError(cerr)
	}
	out := collectBatch(results, statuses)
	runtime.KeepAlive(s)
	runtime.KeepAlive(rules)
	runtime.KeepAlive(data)
	return out, nil
}

// collectBatch copies the borrowed per-item slices out of the session
// buffer into owned BatchResults. Must run before any other call
// touches the session (the borrow's validity window).
func collectBatch(results []C.datalogic_slice, statuses []C.datalogic_status) []BatchResult {
	out := make([]BatchResult, len(results))
	for i := range results {
		body := goStringN(results[i].ptr, results[i].len)
		if statuses[i] == C.DATALOGIC_STATUS_OK {
			out[i].Value = body
		} else {
			out[i].Err = decodeItemError(body)
		}
	}
	return out
}

// decodeItemError parses the per-item error JSON the batch entry points
// write into the result slot ({"tag": ..., "message": ...,
// "operator"?: ...}) into the binding's *Error type.
func decodeItemError(body string) *Error {
	var item struct {
		Tag      string `json:"tag"`
		Message  string `json:"message"`
		Operator string `json:"operator"`
	}
	if err := json.Unmarshal([]byte(body), &item); err != nil || (item.Tag == "" && item.Message == "") {
		// Defensive — the C side always writes the object shape above.
		return &Error{Message: body}
	}
	return &Error{Message: item.Message, Type: item.Tag, Operator: item.Operator}
}
