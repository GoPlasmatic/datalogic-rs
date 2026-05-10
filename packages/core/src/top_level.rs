//! Module-level convenience functions backed by a default [`Engine`].
//!
//! These are the **zero-config** path — call sites that don't need
//! custom operators, custom configuration, or templating mode reach
//! for [`eval`] / [`eval_str`] / [`eval_into`] / [`compile`] directly,
//! without constructing an [`Engine`] first. The shared engine is a
//! `OnceLock<Engine>` of [`Engine::default`]; subsequent calls are
//! free of construction cost.
//!
//! Escalate to [`Engine`] when you need any of:
//! - a non-default [`crate::EvaluationConfig`],
//! - registered [`crate::CustomOperator`]s,
//! - templating mode,
//! - a long-lived [`crate::Session`] for hot loops,
//! - the raw [`crate::Engine::evaluate`] path with caller-owned `&Bump`.

use std::sync::OnceLock;

use crate::{Engine, IntoLogic, Logic, Result};

/// Shared engine for the module-level `eval*` / `compile` helpers.
///
/// Constructed lazily on first use via [`OnceLock`]; later calls reuse
/// the same instance. `Engine` is `Send + Sync`, so the lock itself is
/// only paid on the cold first call.
fn default_engine() -> &'static Engine {
    static ENGINE: OnceLock<Engine> = OnceLock::new();
    ENGINE.get_or_init(Engine::default)
}

/// Compile a rule using the default engine. Equivalent to
/// `Engine::default().compile(rule)`.
///
/// # Example
///
/// ```rust
/// let logic = datalogic_rs::compile(r#"{"+": [1, 2]}"#).unwrap();
/// assert!(logic.is_static());
/// ```
pub fn compile<R: IntoLogic>(rule: R) -> Result<Logic> {
    default_engine().compile(rule)
}

/// One-shot evaluation returning [`datavalue::OwnedDataValue`].
///
/// # Example
///
/// ```rust
/// let result = datalogic_rs::eval(
///     r#"{"+": [{"var": "x"}, 1]}"#,
///     r#"{"x": 41}"#,
/// ).unwrap();
/// assert_eq!(result.as_i64(), Some(42));
/// ```
pub fn eval<R, D>(rule: R, data: D) -> Result<datavalue::OwnedDataValue>
where
    R: IntoLogic,
    D: crate::OwnedInput,
{
    default_engine().eval(rule, data)
}

/// One-shot evaluation returning a JSON [`String`].
///
/// # Example
///
/// ```rust
/// let result = datalogic_rs::eval_str(
///     r#"{"==": [{"var": "x"}, 5]}"#,
///     r#"{"x": 5}"#,
/// ).unwrap();
/// assert_eq!(result, "true");
/// ```
pub fn eval_str<R, D>(rule: R, data: D) -> Result<String>
where
    R: IntoLogic,
    D: crate::OwnedInput,
{
    default_engine().eval_str(rule, data)
}

/// One-shot evaluation deserialised into a typed `T: DeserializeOwned`.
///
/// Use `T = serde_json::Value` for the JSON-value boundary; use a
/// typed struct for direct mapping.
///
/// # Example
///
/// ```rust
/// # #[cfg(feature = "serde_json")] {
/// use serde_json::Value;
///
/// let result: Value = datalogic_rs::eval_into(
///     r#"{"+": [{"var": "x"}, 1]}"#,
///     r#"{"x": 41}"#,
/// ).unwrap();
/// assert_eq!(result, Value::from(42));
/// # }
/// ```
#[cfg(feature = "serde_json")]
#[cfg_attr(docsrs, doc(cfg(feature = "serde_json")))]
pub fn eval_into<T, R, D>(rule: R, data: D) -> Result<T>
where
    T: serde::de::DeserializeOwned,
    R: IntoLogic,
    D: crate::OwnedInput,
{
    default_engine().eval_into(rule, data)
}
