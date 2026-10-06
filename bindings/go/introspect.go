package datalogic

/*
#include "datalogic.h"
*/
import "C"

import (
	"runtime"
	"unsafe"
)

// Mode chooses how Engine.CompileMode and Engine.Check read a rule.
type Mode uint32

const (
	// ModeEngine reads the rule in the engine's own mode, as Compile does.
	ModeEngine Mode = C.DATALOGIC_MODE_ENGINE
	// ModeStrict reads it outside templating mode: a multi-key object or
	// an unknown operator is an error.
	ModeStrict Mode = C.DATALOGIC_MODE_STRICT
	// ModeTemplate reads it in templating mode: a multi-key object is an
	// output template and an unknown key an output field.
	ModeTemplate Mode = C.DATALOGIC_MODE_TEMPLATE
)

// newRule wraps a compiled rule handle, freed when the Rule is collected.
// reg is the compiling engine's custom-operator registry: the Rule keeps
// it alive, since the native rule calls into it after the Engine is gone.
func newRule(ptr *C.datalogic_rule, reg *opRegistry) *Rule {
	r := &Rule{ptr: unsafe.Pointer(ptr), reg: reg}
	runtime.SetFinalizer(r, (*Rule).Close)
	return r
}

// CompileMode compiles ruleJSON in mode, whatever mode the engine was
// built with.
func (e *Engine) CompileMode(ruleJSON string, mode Mode) (*Rule, error) {
	rp, rl := strBytes(ruleJSON)
	var rulePtr *C.datalogic_rule
	var cerr *C.datalogic_error
	rc := C.datalogic_engine_compile_mode(e.cptr(), rp, rl, C.uint32_t(mode), &rulePtr, &cerr)
	runtime.KeepAlive(e)
	if rc != C.DATALOGIC_STATUS_OK {
		return nil, takeError(cerr)
	}
	return newRule(rulePtr, e.registry()), nil
}

// CompileTemplate compiles ruleJSON in templating mode.
func (e *Engine) CompileTemplate(ruleJSON string) (*Rule, error) {
	return e.CompileMode(ruleJSON, ModeTemplate)
}

// CompileStrict compiles ruleJSON outside templating mode.
func (e *Engine) CompileStrict(ruleJSON string) (*Rule, error) {
	return e.CompileMode(ruleJSON, ModeStrict)
}

// CompileChecked compiles ruleJSON, refusing it if Check finds any error.
// The refusal is an *Error with Type "CompileError" whose DiagnosticsJSON
// lists every problem.
func (e *Engine) CompileChecked(ruleJSON string) (*Rule, error) {
	rp, rl := strBytes(ruleJSON)
	var rulePtr *C.datalogic_rule
	var cerr *C.datalogic_error
	rc := C.datalogic_engine_compile_checked(e.cptr(), rp, rl, &rulePtr, &cerr)
	runtime.KeepAlive(e)
	if rc != C.DATALOGIC_STATUS_OK {
		return nil, takeError(cerr)
	}
	return newRule(rulePtr, e.registry()), nil
}

// Check returns every problem the engine can see in ruleJSON before it
// runs, as a JSON array of {code, severity, message, pointer, operator}.
// Finding problems is not an error: they are in the array.
func (e *Engine) Check(ruleJSON string, mode Mode) (string, error) {
	rp, rl := strBytes(ruleJSON)
	var out C.datalogic_buf
	var cerr *C.datalogic_error
	rc := C.datalogic_engine_check(e.cptr(), rp, rl, C.uint32_t(mode), &out, &cerr)
	runtime.KeepAlive(e)
	if rc != C.DATALOGIC_STATUS_OK {
		return "", takeError(cerr)
	}
	return takeBuf(out), nil
}

// Operators returns every built-in operator the engine evaluates, as a
// JSON array in the schema of the docs' operators.json.
func (e *Engine) Operators() (string, error) {
	var out C.datalogic_buf
	var cerr *C.datalogic_error
	rc := C.datalogic_engine_operators(e.cptr(), &out, &cerr)
	runtime.KeepAlive(e)
	if rc != C.DATALOGIC_STATUS_OK {
		return "", takeError(cerr)
	}
	return takeBuf(out), nil
}

// Truthy reports whether the JSON value is truthy under the engine's
// configured truthiness. Under the default rules an empty object is
// falsy, like an empty array.
func (e *Engine) Truthy(valueJSON string) (bool, error) {
	vp, vl := strBytes(valueJSON)
	var out C.int32_t
	var cerr *C.datalogic_error
	rc := C.datalogic_engine_truthy(e.cptr(), vp, vl, &out, &cerr)
	runtime.KeepAlive(e)
	if rc != C.DATALOGIC_STATUS_OK {
		return false, takeError(cerr)
	}
	return out != 0, nil
}

// Facts reports what the compiled rule reads and calls, as JSON:
// {reads, computed_reads, reads_complete, reads_data, operators,
// custom_operators, deterministic}, each read path as its segments.
func (r *Rule) Facts() (string, error) {
	var out C.datalogic_buf
	var cerr *C.datalogic_error
	rc := C.datalogic_rule_facts(r.cptr(), &out, &cerr)
	runtime.KeepAlive(r)
	if rc != C.DATALOGIC_STATUS_OK {
		return "", takeError(cerr)
	}
	return takeBuf(out), nil
}

// EvaluateMetered evaluates rule against dataJSON under an operation
// budget and reports what it cost. budget 0 means the engine's
// configured ops_budget, or unbounded when it has none. Crossing the
// budget is an *Error with Type "BudgetExceeded".
func (s *Session) EvaluateMetered(rule *Rule, dataJSON string, budget uint64) (string, uint64, error) {
	dp, dl := strBytes(dataJSON)
	var outPtr *C.uint8_t
	var outLen C.size_t
	var ops C.uint64_t
	var cerr *C.datalogic_error
	rc := C.datalogic_session_evaluate_metered(
		s.cptr(), rule.cptr(), dp, dl, C.uint64_t(budget), &outPtr, &outLen, &ops, &cerr)
	if rc != C.DATALOGIC_STATUS_OK {
		runtime.KeepAlive(s)
		runtime.KeepAlive(rule)
		return "", 0, takeError(cerr)
	}
	// The result borrows the session's buffer — copy it before the
	// session can be collected.
	out := goStringN(outPtr, outLen)
	runtime.KeepAlive(s)
	runtime.KeepAlive(rule)
	return out, uint64(ops), nil
}
