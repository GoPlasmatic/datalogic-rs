//! Engine evaluation knobs: NaN/divbyzero handling, truthiness rules,
//! numeric coercion, and the recursion-depth cap.
//!
//! The default configuration matches the JSONLogic reference behaviour
//! (JavaScript-flavoured truthiness, NaN errors on bad arithmetic input,
//! `±f64::MAX` on division by zero). Use [`EvaluationConfig::default`]
//! and tweak from there, or pick [`EvaluationConfig::safe_arithmetic`] /
//! [`EvaluationConfig::strict`] as alternative starting points. Apply
//! via [`Engine::builder().with_config(...)`](crate::EngineBuilder::with_config).

use datavalue::OwnedDataValue;
use std::sync::Arc;

/// Knobs that change how an [`Engine`](crate::Engine) treats edge cases
/// during evaluation — non-numeric arguments to arithmetic, division by
/// zero, loose equality across types, truthiness rules, numeric coercion,
/// and a recursion-depth cap.
///
/// Construct via [`Self::default`] for JavaScript-flavoured semantics
/// (matches the JSONLogic reference behaviour), or use
/// [`Self::safe_arithmetic`] / [`Self::strict`] as starting points and
/// tweak from there. Pass to the engine via
/// [`Engine::builder().with_config(...)`](crate::EngineBuilder::with_config).
///
/// # Example
///
/// ```rust
/// use datalogic_rs::{Engine, EvaluationConfig, NanHandling};
///
/// // Chainable setters — only one import needed beyond the enum value.
/// let config = EvaluationConfig::default()
///     .with_arithmetic_nan_handling(NanHandling::IgnoreValue);
/// let engine = Engine::builder().with_config(config).build();
///
/// // "skipped" can't coerce to a number; with `IgnoreValue` the
/// // arithmetic continues with the remaining operands.
/// let result = engine.eval_str(r#"{"+": [1, "skipped", 2]}"#, "null").unwrap();
/// assert_eq!(result, "3");
/// ```
///
/// The struct is `#[non_exhaustive]` — fields can be added in 5.x
/// without breaking downstream. Use [`Self::default`] (or the presets)
/// followed by the `with_*` setters; struct-literal construction from
/// outside the crate is intentionally not supported.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct EvaluationConfig {
    /// What `+` / `-` / `*` / `/` / `%` (and the variadic `min` / `max`)
    /// do when an argument can't be coerced to a number. Default:
    /// [`NanHandling::ThrowError`] — return an `ErrorKind::Thrown`
    /// carrying `{"type": "NaN"}`. The other variants let arithmetic
    /// continue (skip the bad value, treat it as 0, or short-circuit
    /// to `null`).
    pub arithmetic_nan_handling: NanHandling,

    /// What `/` and `%` do when the divisor is zero, on the **float** path.
    /// Default: [`DivisionByZeroHandling::ReturnSaturated`] (the
    /// JavaScript-style `±f64::MAX` / `±f64::MIN` per dividend sign). Switch
    /// to [`DivisionByZeroHandling::ThrowError`] if you'd rather see a
    /// surface error than a sentinel value. Applies uniformly to the 2-arg,
    /// array-fold, and variadic forms.
    ///
    /// Carve-out: an **integer** dividend divided by an integer zero always
    /// errors with `{"type": "NaN"}`, regardless of this setting, since
    /// there is no in-range integer sentinel to return. Only genuinely
    /// fractional operands take the configurable float path.
    pub division_by_zero: DivisionByZeroHandling,

    /// Whether `==` / `!=` (loose equality) raise an error on values
    /// that can't be sensibly compared (e.g. an object compared to a
    /// number). Default: `true` (raise). Set to `false` for the
    /// JavaScript-classic behaviour where any cross-type compare
    /// returns `false` silently.
    pub loose_equality_errors: bool,

    /// How values are coerced to booleans by `if`, `and`, `or`, `!`,
    /// `!!`, and the predicate slot of array operators. Default:
    /// [`TruthyEvaluator::JavaScript`] (the reference JSONLogic rule:
    /// false for `null` / `false` / `0` / `NaN` / `""` / empty
    /// array / empty object, true for everything else). Pick
    /// [`TruthyEvaluator::Python`], [`TruthyEvaluator::StrictBoolean`],
    /// or supply a [`TruthyEvaluator::Custom`] closure for full control.
    pub truthy_evaluator: TruthyEvaluator,

    /// Knobs for the implicit string→number / null→number / bool→number
    /// coercions used by arithmetic and comparison. Default:
    /// [`NumericCoercionConfig::default`] (matches the JSONLogic
    /// reference behaviour). When more than one flag could fire on the
    /// same value, the precedence is:
    ///
    /// 1. `reject_non_numeric` — if `true`, no coercion at all happens;
    ///    the value either parses as a number or the engine reports a
    ///    type error. Overrides every other flag.
    /// 2. `null_to_zero` — only consulted on `null` values.
    /// 3. `bool_to_number` — only consulted on `true` / `false`.
    /// 4. `empty_string_to_zero` — consulted on empty strings.
    ///
    /// Each path is independent in practice (the type filters above
    /// don't overlap), so the precedence only matters when reasoning
    /// about `reject_non_numeric` vs the rest.
    pub numeric_coercion: NumericCoercionConfig,

    /// Maximum number of nested [`Engine::evaluate`](crate::Engine::evaluate)
    /// boundary calls before the engine bails with
    /// [`ErrorKind::ConfigurationError`](crate::ErrorKind::ConfigurationError).
    /// Tracked per-thread, so it
    /// catches `CustomOperator` impls that hold `Arc<Engine>` and
    /// re-enter via `engine.evaluate(...)` from inside their
    /// `evaluate(...)`.
    ///
    /// Default: `256` — generous for legitimate nested rules, tight
    /// enough to bail well before a stack overflow on typical
    /// platforms. The check is skipped entirely when the engine has no
    /// custom operators registered (built-ins can't recurse via
    /// boundary re-entry), so pure-built-in workloads pay nothing.
    pub max_recursion_depth: u32,
}

/// Defines how to handle NaN (Not a Number) scenarios in arithmetic operations
#[derive(Clone, Debug, PartialEq)]
pub enum NanHandling {
    /// Throw an error when encountering non-numeric values (default)
    ThrowError,
    /// Ignore non-numeric values and continue with remaining values
    IgnoreValue,
    /// Treat non-numeric values as zero
    CoerceToZero,
    /// Return null when encountering non-numeric values
    ReturnNull,
}

/// Defines how to handle division by zero
#[derive(Clone, Debug, PartialEq)]
pub enum DivisionByZeroHandling {
    /// Saturating division: clamp the result to the f64 extreme rather
    /// than throw or null. Returns `f64::MAX` for a positive dividend,
    /// `f64::MIN` for a negative dividend, and `0.0` for `0 / 0` (the
    /// indeterminate form saturates to neutral). Default.
    ReturnSaturated,
    /// Throw an error
    ThrowError,
    /// Return null
    ReturnNull,
    /// Return infinity (positive or negative based on dividend sign)
    ReturnInfinity,
}

/// Defines how to evaluate truthiness of values
#[derive(Clone)]
pub enum TruthyEvaluator {
    /// JavaScript-style truthiness (default)
    /// - false: null, false, 0, NaN, "", empty array, empty object
    /// - true: everything else
    JavaScript,

    /// Python-style truthiness
    /// - false: None/null, False, 0, 0.0, "", empty collections
    /// - true: everything else
    Python,

    /// Strict boolean truthiness
    /// - false: null, false
    /// - true: everything else
    StrictBoolean,

    /// Custom truthiness evaluator. Receives the value as an
    /// [`OwnedDataValue`] — the canonical v5 owned value type — so the
    /// callback works without enabling `serde_json` interop.
    ///
    /// Note: this variant cannot participate in `PartialEq` or in a
    /// derived [`Debug`] (the closure is opaque). The hand-rolled `Debug`
    /// impl prints `Custom(<fn>)` so the surrounding [`EvaluationConfig`]
    /// stays debug-printable.
    Custom(Arc<dyn Fn(&OwnedDataValue) -> bool + Send + Sync>),
}

impl std::fmt::Debug for TruthyEvaluator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::JavaScript => f.write_str("JavaScript"),
            Self::Python => f.write_str("Python"),
            Self::StrictBoolean => f.write_str("StrictBoolean"),
            Self::Custom(_) => f.write_str("Custom(<fn>)"),
        }
    }
}

impl TruthyEvaluator {
    /// Wrap a closure as a custom truthiness evaluator without typing
    /// `Arc::new(...)` at the call site.
    ///
    /// The `Arc` wrapping on [`TruthyEvaluator::Custom`] is structurally
    /// required (it keeps [`EvaluationConfig`] `Clone`), so this helper
    /// is purely ergonomic — the `Custom` variant remains public for
    /// callers that already hold an `Arc`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use datalogic_rs::{Engine, EvaluationConfig, TruthyEvaluator};
    /// use datalogic_rs::datavalue::OwnedDataValue;
    ///
    /// // Even integers are truthy.
    /// let config = EvaluationConfig::default().with_truthy_evaluator(
    ///     TruthyEvaluator::custom(|v: &OwnedDataValue| {
    ///         v.as_i64().map(|n| n % 2 == 0).unwrap_or(false)
    ///     }),
    /// );
    /// let engine = Engine::builder().with_config(config).build();
    /// let result = engine.eval_str(r#"{"if": [2, "even", "odd"]}"#, "null").unwrap();
    /// assert_eq!(result, "\"even\"");
    /// ```
    pub fn custom<F>(f: F) -> Self
    where
        F: Fn(&OwnedDataValue) -> bool + Send + Sync + 'static,
    {
        Self::Custom(Arc::new(f))
    }
}

/// Knobs for the implicit value→number coercions arithmetic and
/// comparison perform on non-numeric arguments.
///
/// See [`EvaluationConfig::numeric_coercion`] for how these flags
/// interact when more than one would fire on the same value (short
/// answer: `reject_non_numeric` overrides everything else; the rest are
/// type-disjoint so they don't conflict in practice).
#[derive(Clone, Debug)]
#[non_exhaustive]
pub struct NumericCoercionConfig {
    /// `""` → `0` in numeric context. Default: `true`. Disable to make
    /// `{"+": ["", 1]}` fail with a NaN error instead of returning `1`.
    pub empty_string_to_zero: bool,

    /// `null` → `0` in numeric context. Default: `true`.
    pub null_to_zero: bool,

    /// `true` → `1`, `false` → `0` in numeric context. Default: `true`.
    pub bool_to_number: bool,

    /// Reject non-numeric values: a non-numeric value is a type error.
    /// Default: `false`. When `true`, this flag overrides every other
    /// flag in this struct — empty strings, nulls, and booleans all
    /// raise rather than coerce. Acts as a kill switch for the rest of
    /// the coercion knobs in this struct.
    ///
    /// Note: earlier versions also declared a reserved `undefined_to_zero`
    /// flag here. It never had an effect (JSONLogic does not distinguish a
    /// missing key from an explicit `null` — the reference `missing`
    /// operator treats `{"a": null}` exactly like `{}`) and it has been
    /// removed. A missing var already coerces to `0` under the default
    /// [`Self::null_to_zero`]` = true`.
    pub reject_non_numeric: bool,
}

impl Default for EvaluationConfig {
    fn default() -> Self {
        Self {
            arithmetic_nan_handling: NanHandling::ThrowError,
            division_by_zero: DivisionByZeroHandling::ReturnSaturated,
            loose_equality_errors: true,
            truthy_evaluator: TruthyEvaluator::JavaScript,
            numeric_coercion: NumericCoercionConfig::default(),
            max_recursion_depth: 256,
        }
    }
}

impl Default for NumericCoercionConfig {
    fn default() -> Self {
        Self {
            empty_string_to_zero: true,
            null_to_zero: true,
            bool_to_number: true,
            reject_non_numeric: false,
        }
    }
}

impl NumericCoercionConfig {
    /// Set [`Self::empty_string_to_zero`].
    #[must_use]
    pub fn with_empty_string_to_zero(mut self, value: bool) -> Self {
        self.empty_string_to_zero = value;
        self
    }

    /// Set [`Self::null_to_zero`].
    #[must_use]
    pub fn with_null_to_zero(mut self, value: bool) -> Self {
        self.null_to_zero = value;
        self
    }

    /// Set [`Self::bool_to_number`].
    #[must_use]
    pub fn with_bool_to_number(mut self, value: bool) -> Self {
        self.bool_to_number = value;
        self
    }

    /// Set [`Self::reject_non_numeric`]. When `true`, this flag
    /// overrides every other flag — empty strings, nulls, and booleans
    /// all raise rather than coerce.
    #[must_use]
    pub fn with_reject_non_numeric(mut self, value: bool) -> Self {
        self.reject_non_numeric = value;
        self
    }
}

impl EvaluationConfig {
    /// Set [`Self::arithmetic_nan_handling`].
    #[must_use]
    pub fn with_arithmetic_nan_handling(mut self, value: NanHandling) -> Self {
        self.arithmetic_nan_handling = value;
        self
    }

    /// Set [`Self::division_by_zero`].
    #[must_use]
    pub fn with_division_by_zero(mut self, value: DivisionByZeroHandling) -> Self {
        self.division_by_zero = value;
        self
    }

    /// Set [`Self::loose_equality_errors`].
    #[must_use]
    pub fn with_loose_equality_errors(mut self, value: bool) -> Self {
        self.loose_equality_errors = value;
        self
    }

    /// Set [`Self::truthy_evaluator`].
    #[must_use]
    pub fn with_truthy_evaluator(mut self, value: TruthyEvaluator) -> Self {
        self.truthy_evaluator = value;
        self
    }

    /// Set [`Self::numeric_coercion`].
    #[must_use]
    pub fn with_numeric_coercion(mut self, value: NumericCoercionConfig) -> Self {
        self.numeric_coercion = value;
        self
    }

    /// Set [`Self::max_recursion_depth`].
    #[must_use]
    pub fn with_max_recursion_depth(mut self, value: u32) -> Self {
        self.max_recursion_depth = value;
        self
    }

    /// Create a configuration with safe arithmetic (ignores non-numeric values)
    pub fn safe_arithmetic() -> Self {
        Self {
            arithmetic_nan_handling: NanHandling::IgnoreValue,
            division_by_zero: DivisionByZeroHandling::ReturnNull,
            loose_equality_errors: false,
            ..Default::default()
        }
    }

    /// Create a configuration with strict behavior (more errors)
    pub fn strict() -> Self {
        Self {
            arithmetic_nan_handling: NanHandling::ThrowError,
            division_by_zero: DivisionByZeroHandling::ThrowError,
            loose_equality_errors: true,
            numeric_coercion: NumericCoercionConfig {
                empty_string_to_zero: false,
                null_to_zero: false,
                bool_to_number: false,
                reject_non_numeric: true,
            },
            ..Default::default()
        }
    }
}

#[cfg(feature = "serde_json")]
impl EvaluationConfig {
    /// Build a configuration from a JSON object (string form).
    ///
    /// This is the wire format the language bindings use to pass engine
    /// configuration across FFI boundaries through one shared parser;
    /// Rust callers normally use the typed `with_*` setters instead.
    ///
    /// All keys are optional. An optional `"preset"` key (`"default"`,
    /// `"safe_arithmetic"`, or `"strict"`) selects the starting point;
    /// the remaining keys override individual fields on top of it.
    /// Unknown keys, unknown enum strings, and type mismatches are
    /// rejected with
    /// [`ErrorKind::ConfigurationError`](crate::ErrorKind::ConfigurationError)
    /// so typos fail loudly instead of being silently ignored.
    /// [`TruthyEvaluator::Custom`] cannot be expressed in JSON — custom
    /// truthiness is only available through the Rust API.
    ///
    /// Accepted keys and values (all enum strings are snake_case):
    ///
    /// | Key | Value |
    /// |-----|-------|
    /// | `preset` | `"default"` \| `"safe_arithmetic"` \| `"strict"` |
    /// | `arithmetic_nan_handling` | `"throw_error"` \| `"ignore_value"` \| `"coerce_to_zero"` \| `"return_null"` |
    /// | `division_by_zero` | `"return_saturated"` \| `"throw_error"` \| `"return_null"` \| `"return_infinity"` |
    /// | `loose_equality_errors` | bool |
    /// | `truthy_evaluator` | `"javascript"` \| `"python"` \| `"strict_boolean"` |
    /// | `numeric_coercion` | object with bool keys `empty_string_to_zero`, `null_to_zero`, `bool_to_number`, `reject_non_numeric` |
    /// | `max_recursion_depth` | integer ≥ 1 |
    ///
    /// # Example
    ///
    /// ```rust
    /// use datalogic_rs::{Engine, EvaluationConfig};
    ///
    /// let config = EvaluationConfig::from_json_str(r#"{
    ///     "preset": "strict",
    ///     "division_by_zero": "return_null",
    ///     "numeric_coercion": {"null_to_zero": true},
    ///     "max_recursion_depth": 64
    /// }"#).unwrap();
    /// let engine = Engine::builder().with_config(config).build();
    /// // 1.5 keeps this on the configurable float path (an integer
    /// // dividend over an integer zero always errors).
    /// let result = engine.eval_str(r#"{"/": [1.5, 0]}"#, "null").unwrap();
    /// assert_eq!(result, "null");
    /// ```
    ///
    /// # Errors
    ///
    /// [`ErrorKind::ConfigurationError`](crate::ErrorKind::ConfigurationError)
    /// if the string is not a JSON object or any key or value is
    /// unrecognized.
    pub fn from_json_str(json: &str) -> crate::Result<Self> {
        use serde_json::Value;

        fn cfg_err(msg: String) -> crate::Error {
            crate::Error::configuration_error(msg)
        }
        fn expect_str<'v>(key: &str, value: &'v Value) -> crate::Result<&'v str> {
            value
                .as_str()
                .ok_or_else(|| cfg_err(format!("config key {key:?} must be a string")))
        }
        fn expect_bool(key: &str, value: &Value) -> crate::Result<bool> {
            value
                .as_bool()
                .ok_or_else(|| cfg_err(format!("config key {key:?} must be a boolean")))
        }

        let root: Value = serde_json::from_str(json)
            .map_err(|e| cfg_err(format!("config is not valid JSON: {e}")))?;
        let Value::Object(map) = root else {
            return Err(cfg_err("config must be a JSON object".to_string()));
        };

        let mut config = match map.get("preset") {
            None => Self::default(),
            Some(preset) => match expect_str("preset", preset)? {
                "default" => Self::default(),
                "safe_arithmetic" => Self::safe_arithmetic(),
                "strict" => Self::strict(),
                other => {
                    return Err(cfg_err(format!(
                        "unknown preset {other:?} (expected \"default\", \"safe_arithmetic\", or \"strict\")"
                    )));
                }
            },
        };

        for (key, value) in &map {
            match key.as_str() {
                "preset" => {} // applied above, before the overrides
                "arithmetic_nan_handling" => {
                    config.arithmetic_nan_handling = match expect_str(key, value)? {
                        "throw_error" => NanHandling::ThrowError,
                        "ignore_value" => NanHandling::IgnoreValue,
                        "coerce_to_zero" => NanHandling::CoerceToZero,
                        "return_null" => NanHandling::ReturnNull,
                        other => {
                            return Err(cfg_err(format!(
                                "unknown arithmetic_nan_handling {other:?} (expected \"throw_error\", \"ignore_value\", \"coerce_to_zero\", or \"return_null\")"
                            )));
                        }
                    };
                }
                "division_by_zero" => {
                    config.division_by_zero = match expect_str(key, value)? {
                        "return_saturated" => DivisionByZeroHandling::ReturnSaturated,
                        "throw_error" => DivisionByZeroHandling::ThrowError,
                        "return_null" => DivisionByZeroHandling::ReturnNull,
                        "return_infinity" => DivisionByZeroHandling::ReturnInfinity,
                        other => {
                            return Err(cfg_err(format!(
                                "unknown division_by_zero {other:?} (expected \"return_saturated\", \"throw_error\", \"return_null\", or \"return_infinity\")"
                            )));
                        }
                    };
                }
                "loose_equality_errors" => {
                    config.loose_equality_errors = expect_bool(key, value)?;
                }
                "truthy_evaluator" => {
                    config.truthy_evaluator = match expect_str(key, value)? {
                        "javascript" => TruthyEvaluator::JavaScript,
                        "python" => TruthyEvaluator::Python,
                        "strict_boolean" => TruthyEvaluator::StrictBoolean,
                        other => {
                            return Err(cfg_err(format!(
                                "unknown truthy_evaluator {other:?} (expected \"javascript\", \"python\", or \"strict_boolean\"; custom evaluators are Rust-only)"
                            )));
                        }
                    };
                }
                "numeric_coercion" => {
                    let Value::Object(coercion) = value else {
                        return Err(cfg_err(
                            "config key \"numeric_coercion\" must be an object".to_string(),
                        ));
                    };
                    for (ck, cv) in coercion {
                        match ck.as_str() {
                            "empty_string_to_zero" => {
                                config.numeric_coercion.empty_string_to_zero = expect_bool(ck, cv)?;
                            }
                            "null_to_zero" => {
                                config.numeric_coercion.null_to_zero = expect_bool(ck, cv)?;
                            }
                            "bool_to_number" => {
                                config.numeric_coercion.bool_to_number = expect_bool(ck, cv)?;
                            }
                            "reject_non_numeric" => {
                                config.numeric_coercion.reject_non_numeric = expect_bool(ck, cv)?;
                            }
                            other => {
                                return Err(cfg_err(format!(
                                    "unknown numeric_coercion key {other:?}"
                                )));
                            }
                        }
                    }
                }
                "max_recursion_depth" => {
                    let depth = value
                        .as_u64()
                        .filter(|n| (1..=u64::from(u32::MAX)).contains(n))
                        .ok_or_else(|| {
                            cfg_err(format!(
                                "config key \"max_recursion_depth\" must be an integer between 1 and {}",
                                u32::MAX
                            ))
                        })?;
                    config.max_recursion_depth = depth as u32;
                }
                other => {
                    return Err(cfg_err(format!("unknown config key {other:?}")));
                }
            }
        }

        Ok(config)
    }
}
