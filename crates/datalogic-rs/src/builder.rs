//! Builder for [`Engine`].
//!
//! The single entry point for non-default engine construction
//! (config, custom operators, templating). Replaces the four ad-hoc
//! 4.x constructors (`new`, `with_preserve_structure`, `with_config`,
//! `with_config_and_structure`).

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
/// - **`template_key_escape`** — `None` (no escape prefix; a single-key
///   object whose key names an operator is always an operator
///   invocation). Set with [`Self::with_template_key_escape`].
/// - **`operators`** — empty. Add custom operators with
///   [`Self::add_operator`] before [`Self::build`] freezes the set.
/// - **`constant_folding`** — `true`. The compile pipeline pre-computes
///   constant sub-expressions during [`Engine::compile`]. Disable with
///   [`Self::with_constant_folding`] when you need every operator to
///   survive in the compiled tree (e.g. for tooling that walks the
///   structure or applies its own rewrites).
#[must_use = "the builder is consumed by `.build()`"]
pub struct EngineBuilder {
    config: EvaluationConfig,
    templating: bool,
    template_key_escape: Option<char>,
    constant_folding: bool,
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
            template_key_escape: None,
            constant_folding: true,
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

    /// Set the escape prefix that marks a template object key as a literal
    /// output field instead of an operator invocation. Unset by default.
    ///
    /// Without it, a single-key object is *always* an operator call, so the
    /// ~60 built-in names (`type`, `map`, `if`, `keys`, `length`, `+`, …)
    /// and every registered custom operator are unreachable as output keys.
    /// With it, exactly one leading `prefix` is stripped from every template
    /// key, and an escaped key is never resolved as an operator:
    ///
    /// | Template key | Output key |
    /// |--------------|------------|
    /// | `$type`      | `type`     |
    /// | `$$type`     | `$type`    |
    /// | `$$$type`    | `$$type`   |
    /// | `$foo`       | `foo`      |
    /// | `type`       | not a key: still the `type` operator |
    ///
    /// Stripping is uniform across arities, so a key's source text always
    /// maps to the same output name whether or not it has siblings. Two
    /// consequences worth knowing: `{"$a": 1, "a": 2}` emits the key `a`
    /// twice (the engine keeps duplicate pairs, as `keys`/`values`/`entries`
    /// already do), and a bare `{"$": 1}` emits the empty key.
    ///
    /// `prefix` is a `char` rather than a fixed `$` because `$` already
    /// begins real keys in MongoDB documents and JSON Schema output; those
    /// callers can pick `~` or `#` and leave their `$` keys untouched.
    ///
    /// Only effective in templating mode ([`Self::with_templating`]) and
    /// when the crate is built with `feature = "templating"`. Without
    /// templating every single-key object is an operator invocation, so
    /// there is nothing to escape *into* and this setting is inert.
    ///
    /// ```
    /// use datalogic_rs::Engine;
    ///
    /// let engine = Engine::builder()
    ///     .with_templating(true)
    ///     .with_template_key_escape('$')
    ///     .build();
    /// # let _ = engine;
    /// ```
    #[inline]
    #[must_use = "builder methods return a new builder; chain into `.build()`"]
    pub fn with_template_key_escape(mut self, prefix: char) -> Self {
        self.template_key_escape = Some(prefix);
        self
    }

    /// Toggle the compile-time constant-folding pass. Default: `true`
    /// (folding enabled). Pass `false` when every operator must survive
    /// in the compiled tree — debuggers, alternate evaluators, or any
    /// caller that walks the compiled structure and would be surprised
    /// to see a `{"+": [1, 2]}` collapsed to a `3` literal.
    // The trace-feature addendum links to `Engine::trace`, which only
    // exists with `feature = "trace"`. Gate the whole paragraph behind
    // the same feature so `cargo doc` without `--all-features` doesn't
    // break on the intra-doc link.
    #[cfg_attr(feature = "trace", doc = "")]
    #[cfg_attr(
        feature = "trace",
        doc = "The trace surface ([`crate::Engine::trace`]) always disables folding"
    )]
    #[cfg_attr(
        feature = "trace",
        doc = "internally regardless of this setting, since traces would otherwise"
    )]
    #[cfg_attr(feature = "trace", doc = "lose the folded operators as steps.")]
    #[inline]
    #[must_use = "builder methods return a new builder; chain into `.build()`"]
    pub fn with_constant_folding(mut self, on: bool) -> Self {
        self.constant_folding = on;
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
        Engine::from_builder_parts(
            self.config,
            self.templating,
            self.template_key_escape,
            self.constant_folding,
            self.operators,
        )
    }
}
