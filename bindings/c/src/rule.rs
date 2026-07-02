//! Compiled-rule handle and its evaluate entry point.

use std::ffi::c_char;
use std::sync::Arc;

use datalogic_rs::bumpalo::Bump;
use datalogic_rs::{Engine as RsEngine, Logic};

use crate::cstr_to_str;
use crate::error::{clear_error_state, set_error, set_error_message};
use crate::string_to_cstring;

/// Compiled JSONLogic rule. Send + Sync — share one across threads and
/// evaluate in parallel; each `datalogic_rule_evaluate` call creates its
/// own short-lived arena.
///
/// Holds an `Arc<Engine>` so the engine outlives every rule compiled
/// from it — C consumers can free the engine before the rule and the
/// rule still works (the underlying engine keeps a refcount).
pub struct Rule {
    pub(crate) engine: Arc<RsEngine>,
    pub(crate) logic: Arc<Logic>,
}

/// Release a rule handle. Safe to call with `NULL`.
///
/// # Safety
///
/// `rule` must either be `NULL` or a pointer previously returned by
/// [`crate::datalogic_engine_compile`] that has not been freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_rule_free(rule: *mut Rule) {
    if rule.is_null() {
        return;
    }
    // SAFETY: caller contract.
    drop(unsafe { Box::from_raw(rule) });
}

/// Evaluate a compiled rule against `data_json` and return the result
/// as a freshly-allocated JSON string the caller must release via
/// [`crate::datalogic_string_free`]. Returns `NULL` on failure — query
/// the last-error state for details.
///
/// Uses a short-lived per-call arena. For tight loops, prefer
/// [`crate::datalogic_session_evaluate`] which reuses one arena.
///
/// # Safety
///
/// `rule` must be a valid pointer returned by
/// [`crate::datalogic_engine_compile`]; `data_json` must be a valid
/// NUL-terminated UTF-8 string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn datalogic_rule_evaluate(
    rule: *mut Rule,
    data_json: *const c_char,
) -> *mut c_char {
    crate::ffi_guard(std::ptr::null_mut(), || {
        clear_error_state();
        let rule = match unsafe { rule.as_ref() } {
            Some(r) => r,
            None => {
                set_error_message("rule pointer is null", "ParseError");
                return std::ptr::null_mut();
            }
        };
        let data_json = match cstr_to_str(data_json) {
            Some(s) => s,
            None => {
                set_error_message("data_json is null or not valid UTF-8", "ParseError");
                return std::ptr::null_mut();
            }
        };
        let arena = Bump::new();
        match rule.engine.evaluate(&rule.logic, data_json, &arena) {
            Ok(av) => string_to_cstring(av.to_string()),
            Err(e) => {
                set_error(&e, Some(&rule.logic));
                std::ptr::null_mut()
            }
        }
    })
}
