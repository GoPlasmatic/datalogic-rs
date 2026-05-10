//! Builder for [`Engine`].
//!
//! Replaces the four ad-hoc 4.x constructors (`new`, `with_preserve_structure`,
//! `with_config`, `with_config_and_structure`) with a single fluent builder.
//! All four are still reachable through `crate::compat::LegacyApi` — see that
//! module for the deprecated shims.

use std::collections::HashMap;

use crate::CustomOperator;
use crate::config::EvaluationConfig;
use crate::engine::Engine;

/// Builder for [`Engine`]. Construct via [`Engine::builder`].
///
/// ```
/// use datalogic_rs::Engine;
///
/// let engine = Engine::builder().build();
/// # let _ = engine;
/// ```
///
/// # Defaults
///
/// `Engine::builder().build()` produces the same engine as
/// [`Engine::new`] / [`Engine::default`]:
///
/// - **`config`** — [`EvaluationConfig::default`]: JavaScript-flavoured
///   truthiness, NaN errors on bad arithmetic input, `±f64::MAX` on
///   division by zero, `loose_equality_errors = true`,
///   `max_recursion_depth = 256`, and the implicit `null`/`bool`/
///   `""` → 0 numeric coercions enabled. Override with [`Self::with_config`];
///   [`EvaluationConfig::safe_arithmetic`] / [`EvaluationConfig::strict`]
///   are alternative starting points.
/// - **`templating`** — `false` (templating mode off). Set with
///   [`Self::with_templating`]; only effective when the crate is
///   built with `feature = "templating"`.
/// - **`operators`** — empty. Add custom operators with
///   [`Self::add_operator`] before [`Self::build`] freezes the set.
#[must_use = "the builder is consumed by `.build()`"]
pub struct EngineBuilder {
    config: EvaluationConfig,
    templating: bool,
    operators: HashMap<String, Box<dyn CustomOperator>>,
}

impl Default for EngineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl EngineBuilder {
    /// Fresh builder with default config and no custom operators.
    #[inline]
    pub fn new() -> Self {
        Self {
            config: EvaluationConfig::default(),
            templating: false,
            operators: HashMap::new(),
        }
    }

    /// Set the evaluation config.
    #[inline]
    #[must_use = "builder methods return a new builder; chain into `.build()`"]
    pub fn with_config(mut self, config: EvaluationConfig) -> Self {
        self.config = config;
        self
    }

    /// Toggle templating mode (multi-key objects compile to output-shaping
    /// templates; unknown operator keys pass through verbatim). Only
    /// effective when the crate is built with `feature = "templating"`.
    #[inline]
    #[must_use = "builder methods return a new builder; chain into `.build()`"]
    pub fn with_templating(mut self, on: bool) -> Self {
        self.templating = on;
        self
    }

    /// Register a [`CustomOperator`] under `name`. Multiple calls with the
    /// same name overwrite the prior registration.
    ///
    /// Accepts both typed operators (`T: CustomOperator + 'static`) and
    /// pre-boxed trait objects (`Box<dyn CustomOperator>`) — the bare
    /// `Box<dyn CustomOperator>` itself implements `CustomOperator`
    /// (delegating to the inner), so a single entry point covers both
    /// shapes:
    ///
    /// ```ignore
    /// builder
    ///     .add_operator("typed", MyOp)                            // typed
    ///     .add_operator("dyn", boxed_op_from_registry as Box<_>)  // pre-boxed
    /// ```
    ///
    /// Operator registration is builder-only; once [`Self::build`] hands
    /// you an [`Engine`], its operator set is frozen.
    ///
    /// **Built-ins always win.** If `name` collides with a built-in
    /// JSONLogic operator (`+`, `if`, `var`, `map`, …), the built-in is
    /// dispatched and the registered custom op is never reached. To
    /// extend the operator set, choose a name that doesn't parse as a
    /// built-in.
    #[inline]
    #[must_use = "builder methods return a new builder; chain into `.build()`"]
    pub fn add_operator<T>(mut self, name: impl Into<String>, operator: T) -> Self
    where
        T: CustomOperator + 'static,
    {
        self.operators.insert(name.into(), Box::new(operator));
        self
    }

    /// Finalise the builder into an immutable [`Engine`] engine.
    pub fn build(self) -> Engine {
        Engine::from_builder_parts(self.config, self.templating, self.operators)
    }
}
