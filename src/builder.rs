//! Builder for [`Engine`].
//!
//! Replaces the four ad-hoc 4.x constructors (`new`, `with_preserve_structure`,
//! `with_config`, `with_config_and_structure`) with a single fluent builder.
//! All four are still reachable through `crate::compat::LegacyApi` — see that
//! module for the deprecated shims.

use std::collections::HashMap;

use crate::CustomOperator;
use crate::IntoOperatorBox;
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
#[must_use = "the builder is consumed by `.build()`"]
pub struct EngineBuilder {
    config: EvaluationConfig,
    preserve_structure: bool,
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
            preserve_structure: false,
            operators: HashMap::new(),
        }
    }

    /// Set the evaluation config.
    #[inline]
    pub fn config(mut self, config: EvaluationConfig) -> Self {
        self.config = config;
        self
    }

    /// Toggle structure-preservation mode (templating). Only effective when
    /// the crate is built with `feature = "preserve"`.
    #[inline]
    pub fn preserve_structure(mut self, on: bool) -> Self {
        self.preserve_structure = on;
        self
    }

    /// Register a [`CustomOperator`] under `name`. Accepts either a bare
    /// `T: CustomOperator` or a pre-boxed `Box<dyn CustomOperator>` via
    /// [`IntoOperatorBox`]. Multiple calls with the same name overwrite the
    /// prior registration.
    ///
    /// Operator registration is builder-only; once [`Self::build`] hands you
    /// an [`Engine`], its operator set is frozen.
    #[inline]
    pub fn add_operator(mut self, name: impl Into<String>, operator: impl IntoOperatorBox) -> Self {
        self.operators
            .insert(name.into(), operator.into_operator_box());
        self
    }

    /// Unregister a previously-added operator. Silently no-ops if `name`
    /// wasn't registered. Useful when composing builders from helper
    /// functions that pre-register more than the caller needs.
    #[inline]
    pub fn remove_operator(mut self, name: &str) -> Self {
        self.operators.remove(name);
        self
    }

    /// Finalise the builder into an immutable [`Engine`] engine.
    pub fn build(self) -> Engine {
        Engine::from_builder_parts(self.config, self.preserve_structure, self.operators)
    }
}
