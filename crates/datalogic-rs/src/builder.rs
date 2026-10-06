//! Builder for [`Engine`].
//!
//! The single entry point for non-default engine construction
//! (config, custom operators, templating). Replaces the four ad-hoc
//! 4.x constructors (`new`, `with_preserve_structure`, `with_config`,
//! `with_config_and_structure`).

use std::collections::HashMap;
use std::sync::Arc;

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
#[derive(Clone)]
#[must_use = "the builder is consumed by `.build()`"]
pub struct EngineBuilder {
    config: EvaluationConfig,
    templating: bool,
    template_key_escape: Option<char>,
    constant_folding: bool,
    operators: HashMap<String, Arc<dyn CustomOperator>>,
    /// The built-in families the engine has, as [`crate::Family`] bits.
    families: u32,
    /// The names registered through [`Self::try_add_operator`], which
    /// [`Self::try_build`] checks again against the final settings.
    checked_names: Vec<String>,
}

/// Every family bit: the default family set.
pub(crate) const ALL_FAMILIES: u32 = u32::MAX;

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
            families: ALL_FAMILIES,
            checked_names: Vec::new(),
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
    /// built-in, or register through [`Self::try_add_operator`], which
    /// refuses such a name instead.
    #[inline]
    #[must_use = "builder methods return a new builder; chain into `.build()`"]
    pub fn add_operator<T>(mut self, name: impl Into<String>, operator: T) -> Self
    where
        T: CustomOperator + 'static,
    {
        self.operators.insert(name.into(), Arc::new(operator));
        self
    }

    /// [`Self::add_operator`], refusing a name a built-in operator of this
    /// build answers to (a canonical name or an alias such as `var` or
    /// `?:`), where the custom operator would never run.
    ///
    /// Only operators compiled into this build count: without the
    /// `datetime` feature, `now` is a free name.
    ///
    /// # Errors
    ///
    /// [`crate::ErrorKind::ConfigurationError`] naming the built-in that
    /// would win.
    ///
    /// # Example
    ///
    /// ```rust
    /// use datalogic_rs::{
    ///     CustomOperator, DataValue, Engine, ErrorCode, Result, operator::EvalContext,
    /// };
    ///
    /// struct Answer;
    /// impl CustomOperator for Answer {
    ///     fn evaluate<'a>(
    ///         &self,
    ///         _args: &[&'a DataValue<'a>],
    ///         _ctx: &mut EvalContext<'_, 'a>,
    ///         arena: &'a bumpalo::Bump,
    ///     ) -> Result<&'a DataValue<'a>> {
    ///         Ok(arena.alloc(DataValue::from_f64(42.0)))
    ///     }
    /// }
    ///
    /// assert!(Engine::builder().try_add_operator("answer", Answer).is_ok());
    /// let err = Engine::builder().try_add_operator("if", Answer).err().unwrap();
    /// assert_eq!(err.code(), ErrorCode::ConfigurationError);
    /// ```
    pub fn try_add_operator<T>(self, name: impl Into<String>, operator: T) -> crate::Result<Self>
    where
        T: CustomOperator + 'static,
    {
        let name = name.into();
        self.check_operator_name(&name)?;
        let mut builder = self.add_operator(name.clone(), operator);
        builder.checked_names.push(name);
        Ok(builder)
    }

    /// The refusal [`Self::try_add_operator`] gives a custom operator named
    /// `name`, without registering anything: a `ConfigurationError` when a
    /// built-in of this builder's families answers to the name, or when
    /// the name begins with the [template key
    /// escape](Self::with_template_key_escape), which makes such a key an
    /// output field. For a host that must keep the builder whether or not
    /// the name is taken.
    ///
    /// The check reads the builder's settings when it is called; set
    /// [`Self::with_families`] and the escape first, or build with
    /// [`Self::try_build`], which checks every such name again.
    ///
    /// ```rust
    /// use datalogic_rs::{Engine, Family};
    ///
    /// assert!(Engine::builder().check_operator_name("if").is_err());
    /// // `upper` is free once its family is left out.
    /// let core = Engine::builder().with_families([Family::ExtArray]);
    /// assert!(core.check_operator_name("upper").is_ok());
    /// ```
    pub fn check_operator_name(&self, name: &str) -> crate::Result<()> {
        if let Some(builtin) = crate::engine::builtin_in(self.families, name) {
            return Err(crate::Error::configuration_error(format!(
                "custom operator `{name}` would never run: the built-in operator `{}` answers to that name",
                builtin.as_str()
            )));
        }
        if let Some(escape) = self.template_key_escape
            && name.starts_with(escape)
        {
            return Err(crate::Error::configuration_error(format!(
                "custom operator `{name}` would never run in a template: it begins with the template key escape `{escape}`"
            )));
        }
        Ok(())
    }

    /// [`Self::check_operator_name`] for every name registered through
    /// [`Self::try_add_operator`], against the builder's settings now: a
    /// name checked before [`Self::with_families`] or
    /// [`Self::with_template_key_escape`] changed them may be taken since.
    ///
    /// # Errors
    ///
    /// The first name's `ConfigurationError`, in registration order.
    pub fn check_operator_names(&self) -> crate::Result<()> {
        self.checked_names
            .iter()
            .try_for_each(|name| self.check_operator_name(name))
    }

    /// Keep the engine to the JSONLogic core and the extension families
    /// named here; by default it has every family this build compiled in.
    /// A family left out (or not compiled in) is not there for this engine:
    /// its operator names compile as unknown operators, as output fields in
    /// templating mode, or as a custom operator registered under that name.
    /// [`Engine::operators`] and [`Engine::builtin_operator_names`] list
    /// only what the engine has, and [`Self::try_add_operator`] refuses only
    /// those names. [`Family::Core`](crate::Family::Core) is always there,
    /// named or not.
    ///
    /// The set applies when a rule is compiled. A rule compiled on another
    /// engine keeps its operators wherever it is evaluated.
    ///
    /// ```rust
    /// use datalogic_rs::{Engine, Family};
    ///
    /// // The JSONLogic core and the string extensions, nothing else.
    /// let engine = Engine::builder().with_families([Family::ExtString]).build();
    /// assert_eq!(engine.eval_str(r#"{"upper": "a"}"#, "null").unwrap(), r#""A""#);
    /// // `sort` is an unknown operator here, as `length` would be on an
    /// // engine without `ExtString`.
    /// assert!(engine.eval_str(r#"{"sort": [[2, 1]]}"#, "null").is_err());
    /// assert!(engine.compile_checked(r#"{"sort": [[2, 1]]}"#).is_err());
    /// ```
    #[must_use = "builder methods return a new builder; chain into `.build()`"]
    pub fn with_families(mut self, families: impl IntoIterator<Item = crate::Family>) -> Self {
        self.families = families
            .into_iter()
            .fold(crate::Family::Core.bit(), |set, f| set | f.bit());
        self
    }

    /// A builder holding `engine`'s operators and settings: the
    /// [`Engine::to_builder`] seam.
    pub(crate) fn from_engine_parts(
        config: EvaluationConfig,
        templating: bool,
        template_key_escape: Option<char>,
        constant_folding: bool,
        operators: HashMap<String, Arc<dyn CustomOperator>>,
        families: u32,
    ) -> Self {
        Self {
            config,
            templating,
            template_key_escape,
            constant_folding,
            operators,
            families,
            checked_names: Vec::new(),
        }
    }

    /// [`Self::build`], first checking every name registered through
    /// [`Self::try_add_operator`] again ([`Self::check_operator_names`]),
    /// so the refusal holds whatever order the settings were given in.
    ///
    /// ```rust
    /// use datalogic_rs::{CustomOperator, DataValue, Engine, Family, Result, operator::EvalContext};
    ///
    /// struct Up;
    /// impl CustomOperator for Up {
    ///     fn evaluate<'a>(
    ///         &self,
    ///         args: &[&'a DataValue<'a>],
    ///         _ctx: &mut EvalContext<'_, 'a>,
    ///         _arena: &'a datalogic_rs::bumpalo::Bump,
    ///     ) -> Result<&'a DataValue<'a>> {
    ///         Ok(args[0])
    ///     }
    /// }
    ///
    /// // `upper` is free without the string family, then taken once it is
    /// // added back.
    /// let builder = Engine::builder()
    ///     .with_families([Family::ExtArray])
    ///     .try_add_operator("upper", Up)
    ///     .unwrap()
    ///     .with_families([Family::ExtString]);
    /// assert!(builder.try_build().is_err());
    /// ```
    ///
    /// # Errors
    ///
    /// The first name's `ConfigurationError`, in registration order.
    pub fn try_build(self) -> crate::Result<Engine> {
        self.check_operator_names()?;
        Ok(self.build())
    }

    /// Finalise the builder into an immutable [`Engine`] engine.
    pub fn build(self) -> Engine {
        Engine::from_builder_parts(
            self.config,
            self.templating,
            self.template_key_escape,
            self.constant_folding,
            self.operators,
            self.families,
        )
    }
}
