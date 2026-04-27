//! Builder for [`DataLogic`].
//!
//! Replaces the four ad-hoc 4.x constructors (`new`, `with_preserve_structure`,
//! `with_config`, `with_config_and_structure`) with a single fluent builder.
//! All four are still reachable through `crate::compat::DataLogicLegacyExt` —
//! see that module for the deprecated shims.

use std::collections::HashMap;

use crate::DataOperator;
use crate::config::EvaluationConfig;
use crate::engine::DataLogic;

/// Builder for [`DataLogic`]. Construct via [`DataLogic::builder`].
///
/// ```
/// use datalogic_rs::DataLogic;
///
/// let engine = DataLogic::builder().build();
/// # let _ = engine;
/// ```
#[must_use = "the builder is consumed by `.build()`"]
pub struct DataLogicBuilder {
    config: EvaluationConfig,
    preserve_structure: bool,
    operators: HashMap<String, Box<dyn DataOperator>>,
}

impl Default for DataLogicBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl DataLogicBuilder {
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

    /// Register a custom [`DataOperator`] under `name`. Multiple calls with
    /// the same name overwrite the prior registration.
    #[inline]
    pub fn add_operator(
        mut self,
        name: impl Into<String>,
        operator: Box<dyn DataOperator>,
    ) -> Self {
        self.operators.insert(name.into(), operator);
        self
    }

    /// Finalise the builder into an immutable [`DataLogic`] engine.
    pub fn build(self) -> DataLogic {
        DataLogic::from_builder_parts(self.config, self.preserve_structure, self.operators)
    }
}
