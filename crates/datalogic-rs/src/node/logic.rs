//! `Logic` — the compiled, thread-safe rule snapshot returned by
//! `Engine::compile`. Includes the static-evaluation predicates the compiler
//! consults to decide whether a sub-expression can be folded.

use super::{CompiledNode, populate_lits};

/// Compiled logic that can be evaluated multiple times across different data.
///
/// `Logic` represents a pre-processed JSONLogic expression that has been
/// optimized for repeated evaluation. It's thread-safe and can be shared across
/// threads using `Arc`.
///
/// # Performance Benefits
///
/// - **Parse once, evaluate many**: Avoid repeated JSON parsing
/// - **Static evaluation**: Constant expressions are pre-computed
/// - **OpCode dispatch**: Built-in operators use fast enum dispatch
/// - **Thread-safe sharing**: Use `Arc` to share across threads
///
/// # Example
///
/// ```rust
/// use std::sync::Arc;
/// use datalogic_rs::Engine;
///
/// let engine = Engine::new();
/// let compiled = Arc::new(engine.compile(r#"{">": [{"var": "score"}, 90]}"#).unwrap());
///
/// // Compiled logic can be cloned cheaply (atomic refcount) and sent across threads.
/// let compiled_clone = Arc::clone(&compiled);
/// std::thread::spawn(move || {
///     let engine = Engine::new();
///     let _result = engine
///         .session()
///         .eval_str(&compiled_clone, r#"{"score": 95}"#)
///         .unwrap();
/// });
/// ```
///
/// `Logic` is `Clone` (deep-clones the compiled tree). Cloning is the right
/// choice when a caller needs an independently mutable copy or wants to
/// store the rule by value; for sharing the *same* compiled rule across
/// threads or evaluations, prefer `Arc<Logic>` — the `Arc::clone` is a
/// single atomic refcount bump rather than a tree walk.
#[derive(Clone)]
pub struct Logic {
    /// The root node of the compiled logic tree.
    pub(crate) root: CompiledNode,
    /// Pre-resolved operator name for the root node, attached to every
    /// `Error` returned from the public `evaluate*` API. Cached at compile
    /// time so the error-unwind path does no tree walk. `Cow::Borrowed`
    /// for built-ins (zero alloc on attach), `Cow::Owned` for
    /// `CustomOperator` (one alloc per compile, amortised over many
    /// evaluations), `None` for `Value` literals.
    pub(crate) root_op_name: Option<std::borrow::Cow<'static, str>>,
    /// Number of CSE memo slots assigned by the compile-time CSE pass
    /// (`crate::compile::optimize::cse`); the upper bound of the lazily
    /// grown per-evaluation slot table on `ContextStack`. `0` for rules
    /// with no shared pure aggregates (and for every no-fold / traced
    /// compile). Plain `Copy` data — `Logic` stays `Send + Sync` with no
    /// interior mutability.
    pub(crate) cse_slot_count: u16,
    /// Whether evaluation must maintain the ancestor-frame list. `false` for
    /// the overwhelming majority of rules — reaching one takes both two
    /// levels of iterator nesting and a level marker inside the inner one —
    /// which lets
    /// `ContextStack` skip the list entirely. See
    /// [`crate::compile::scope::resolve`].
    pub(crate) needs_ancestor_frames: bool,
    /// The id of the engine that compiled this rule; see
    /// [`Self::compiled_on`].
    pub(crate) engine_id: u64,
    /// `(node id, JSON Pointer)` in id order, for a rule compiled for
    /// tracing; see [`Self::pointer`].
    pub(crate) pointers: Option<super::compile_ctx::NodePointers>,
    /// What of an input this rule reads, worked out on first use; see
    /// [`crate::projection`].
    pub(crate) projection: std::sync::OnceLock<Option<Box<crate::projection::Projection>>>,
    /// For a rule whose compile folded something under the engine's
    /// settings: its source, to compile it again for an engine that
    /// would fold it differently; see [`Self::for_engine`]. `None` for
    /// every other rule.
    pub(crate) refold: Option<Box<Refold>>,
}

/// How many differently-configured engines keep their own compile of one
/// rule (see [`Logic::for_engine`]). Past that, an engine evaluates the
/// rule as first compiled.
const REFOLD_SLOTS: usize = 4;

/// The source of a rule whose constants were folded under the compiling
/// engine's settings, and that rule compiled again on engines with other
/// settings; see [`Logic::for_engine`].
#[derive(Clone)]
pub(crate) struct Refold {
    source: datavalue::OwnedDataValue,
    templating: bool,
    /// The compiling engine's `Engine::fold_fingerprint`.
    fingerprint: u64,
    /// `(fingerprint, rule compiled on an engine with it)`, filled on
    /// first use.
    others: [std::sync::OnceLock<(u64, Box<Logic>)>; REFOLD_SLOTS],
}

impl Refold {
    pub(crate) fn new(
        source: datavalue::OwnedDataValue,
        templating: bool,
        fingerprint: u64,
    ) -> Self {
        Self {
            source,
            templating,
            fingerprint,
            others: Default::default(),
        }
    }

    /// The rule compiled on `engine`, from a slot holding `engine`'s
    /// fingerprint or one filled now. `None` when the compile fails (the
    /// engine lacks an operator the rule uses, say) or every slot holds
    /// another fingerprint.
    #[cold]
    fn on(&self, engine: &crate::Engine) -> Option<&Logic> {
        let fingerprint = engine.fold_fingerprint();
        for slot in &self.others {
            if slot.get().is_none() {
                let mut logic =
                    Logic::compile_in_mode(&self.source, engine, self.templating).ok()?;
                // Only ever reached through this rule, so it needs no
                // source of its own.
                logic.refold = None;
                // Another thread may fill the slot first; then its entry
                // is checked like any other.
                let _ = slot.set((fingerprint, Box::new(logic)));
            }
            if let Some((held, logic)) = slot.get()
                && *held == fingerprint
            {
                return Some(logic);
            }
        }
        None
    }
}

impl std::fmt::Debug for Logic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Logic")
            .field("root", &self.root)
            .field("root_op_name", &self.root_op_name)
            .finish_non_exhaustive()
    }
}

impl Logic {
    /// Creates a new compiled logic from a root node.
    ///
    /// Caches per-operator analysis results onto every `BuiltinOperator`
    /// node and pre-builds every literal onto its `Value` node. Trivial
    /// literals (Null/Bool/Number/empty) are pre-built by
    /// [`super::populate::precompute_lit`] at construction; non-trivial
    /// literals (non-empty Strings/Arrays/Objects) by the
    /// [`super::populate::populate_lits`] pass here, so dispatch returns a
    /// borrow instead of re-converting the literal per evaluation.
    ///
    /// # Arguments
    ///
    /// * `root` - The root node of the compiled logic tree
    /// * `cse_slot_count` - Memo slots assigned by the CSE pass (0 when
    ///   the pass didn't run or found nothing to share)
    pub(crate) fn new(
        mut root: CompiledNode,
        cse_slot_count: u16,
        needs_ancestor_frames: bool,
    ) -> Self {
        populate_lits(&mut root);
        let root_op_name = root.operator_name();
        Self {
            root,
            root_op_name,
            cse_slot_count,
            needs_ancestor_frames,
            engine_id: 0,
            pointers: None,
            projection: std::sync::OnceLock::new(),
            refold: None,
        }
    }

    /// This rule as `engine` runs it. Compiling folds constant
    /// subexpressions under the compiling engine's settings (number
    /// coercion, NaN and division handling, loose equality, truthiness).
    /// On an engine whose settings differ, a rule that folded something
    /// is compiled again on that engine, once per distinct setting, so a
    /// folded constant and the same expression computed at runtime agree.
    /// Any other rule, and every rule on the engine that compiled it, is
    /// itself.
    #[inline]
    pub(crate) fn for_engine(&self, engine: &crate::Engine) -> &Logic {
        match &self.refold {
            Some(refold)
                if self.engine_id != engine.id()
                    && refold.fingerprint != engine.fold_fingerprint() =>
            {
                refold.on(engine).unwrap_or(self)
            }
            _ => self,
        }
    }

    /// Whether `engine` compiled this rule: the same engine instance, not
    /// one built alike or rebuilt from it with
    /// [`Engine::to_builder`](crate::Engine::to_builder).
    ///
    /// Any engine can evaluate a rule in 5.x; on another engine its custom
    /// operators are looked up by name, and a rule whose constants were
    /// folded under different evaluation settings is compiled again on
    /// that engine (once per distinct setting) so its folded constants
    /// follow the evaluating engine's settings. In 6.0 a rule evaluates
    /// only on the engine that compiled it, so a host can use this to
    /// find the places that evaluate on another one.
    ///
    /// ```rust
    /// use datalogic_rs::Engine;
    ///
    /// let boot = Engine::new();
    /// let serving = Engine::new();
    /// let logic = boot.compile(r#"{"var": "x"}"#).unwrap();
    /// assert!(logic.compiled_on(&boot));
    /// assert!(!logic.compiled_on(&serving));
    /// ```
    pub fn compiled_on(&self, engine: &crate::Engine) -> bool {
        self.engine_id == engine.id()
    }

    /// The [RFC 6901](https://www.rfc-editor.org/rfc/rfc6901) JSON Pointer
    /// of the source value that node `id` was compiled from: `""` for the
    /// whole rule, `"/if/1"` for the second argument of a top-level `if`,
    /// `"/!"` for the lone argument of `{"!": x}`. A node the compiler adds
    /// for a call (a computed `var` path, say) points at that call.
    ///
    /// Recorded only for a rule compiled with
    /// `TracedSession::compile`; `None`
    /// for any other rule and for an id it does not have. A trace's
    /// `ExpressionNode::id` and
    /// `ExecutionStep::node_id` are such ids, so a
    /// debugger can place every step in the rule it shows.
    pub fn pointer(&self, id: u32) -> Option<&str> {
        let pointers = self.pointers.as_deref()?;
        pointers
            .binary_search_by_key(&id, |(node, _)| *node)
            .ok()
            .map(|at| &*pointers[at].1)
    }

    /// The read projection to evaluate this rule with on `engine`, or
    /// `None` to view the whole input. Only on the engine that compiled the
    /// rule: the facts it rests on trust each custom operator's declaration
    /// as compiled, and another engine may run another operator of that
    /// name.
    #[inline]
    pub(crate) fn projection_for(
        &self,
        engine: &crate::Engine,
    ) -> Option<&crate::projection::Projection> {
        if !self.compiled_on(engine) {
            return None;
        }
        self.projection
            .get_or_init(|| crate::projection::Projection::of(&self.facts()).map(Box::new))
            .as_deref()
    }

    /// Every `(node id, pointer)` [`Self::pointer`] knows, in id order;
    /// empty unless the rule was compiled with
    /// `TracedSession::compile`.
    pub fn pointers(&self) -> impl Iterator<Item = (u32, &str)> {
        self.pointers
            .as_deref()
            .unwrap_or_default()
            .iter()
            .map(|(id, pointer)| (*id, &**pointer))
    }

    /// Check if this compiled logic is static (can be evaluated without context)
    pub fn is_static(&self) -> bool {
        node_is_static(&self.root)
    }

    /// Check if compilation reduced this rule to a compile-time constant.
    ///
    /// The compiler constant-folds every static sub-expression it can
    /// prove, so a rule with no data dependency usually compiles down to a
    /// single literal node. `is_constant` reports whether that happened
    /// for the *whole* rule: evaluating a constant rule returns the
    /// pre-computed value without executing any operator, so its cost is
    /// literal-return overhead, not engine work.
    ///
    /// Contrast with [`Self::is_static`]: `is_static` asks whether the
    /// tree *could* be evaluated without a data context, while
    /// `is_constant` reports whether the compiler actually *did* collapse
    /// the root to a literal. The two can differ; for example
    /// `{"/": [1, 0]}` is static, but folding it fails (division by zero
    /// errors under the default configuration), so the operator node is
    /// kept and the error surfaces at evaluation time. Benchmarks and
    /// rule-analysis tooling use this accessor to separate folded rules
    /// from genuinely data-dependent ones.
    ///
    /// # Example
    ///
    /// ```rust
    /// use datalogic_rs::Engine;
    ///
    /// let engine = Engine::new();
    ///
    /// // No data dependency: the compiler folds `1 + 2` to the literal `3`.
    /// let folded = engine.compile(r#"{"+": [1, 2]}"#).unwrap();
    /// assert!(folded.is_constant());
    ///
    /// // Reads the data context, so it stays an operator node.
    /// let dynamic = engine.compile(r#"{"var": "x"}"#).unwrap();
    /// assert!(!dynamic.is_constant());
    /// ```
    pub fn is_constant(&self) -> bool {
        matches!(self.root, CompiledNode::Value { .. })
    }

    /// What this rule reads, which operators it uses, and whether its
    /// result is a function of its data.
    ///
    /// Computed from the compiled tree on each call (one walk, no
    /// evaluation), so it describes the rule after the optimizer: a branch
    /// constant folding removed is not read. See [`crate::Facts`] for what
    /// each answer covers.
    ///
    /// # Example
    ///
    /// ```rust
    /// use datalogic_rs::Engine;
    ///
    /// let engine = Engine::new();
    /// let rule = engine
    ///     .compile(r#"{"if": [{"var": "user.vip"}, {"map": [{"var": "cart"}, {"var": "price"}]}, []]}"#)
    ///     .unwrap();
    /// let facts = rule.facts();
    ///
    /// // `price` is read from each cart item, which `cart` already covers.
    /// let reads: Vec<String> = facts.reads().iter().map(|p| p.to_string()).collect();
    /// assert_eq!(reads, ["cart", "user.vip"]);
    /// assert!(facts.reads_complete());
    ///
    /// assert_eq!(facts.operators(), ["if", "map", "val"]);
    /// assert!(facts.is_deterministic());
    /// ```
    pub fn facts(&self) -> crate::Facts {
        crate::facts::collect(&self.root)
    }

    /// Number of shared-subexpression memo slots the compiler assigned to
    /// this rule.
    ///
    /// The compile-time CSE pass detects structurally identical pure
    /// subtrees (typically repeated aggregates — JSONLogic has no `let`
    /// bindings, so rule authors paste them) and arranges for each
    /// equivalence class to be computed once per evaluation. `0` means no
    /// shared subexpressions were found — or the pass didn't run (engines
    /// built with [`crate::EngineBuilder::with_constant_folding`]`(false)`
    /// or configured with a [`crate::TruthyEvaluator::Custom`] evaluator
    /// compile without CSE). Useful for rule-analysis tooling and for
    /// verifying that a hot rule benefits from sharing.
    ///
    /// # Example
    ///
    /// ```rust
    /// use datalogic_rs::Engine;
    ///
    /// let engine = Engine::new();
    /// let agg = r#"{"reduce": [{"var": "xs"}, {"+": [{"var": "accumulator"}, {"var": "current"}]}, 0]}"#;
    /// let rule = format!(r#"{{"+": [{agg}, {agg}]}}"#);
    /// let compiled = engine.compile(rule.as_str()).unwrap();
    /// assert_eq!(compiled.cse_slot_count(), 1);
    /// ```
    pub fn cse_slot_count(&self) -> u16 {
        self.cse_slot_count
    }

    /// Reconstruct a JSONLogic string from this compiled tree.
    ///
    /// Reflects the *compiled* shape — constant-folded sub-expressions
    /// appear as literals, since the original operator is gone by then.
    /// Re-parsing the output through [`crate::Engine::compile`] yields a
    /// `Logic` that evaluates identically. Useful for caching keys, identity
    /// checks across compiled rules, debug logging, and tooling.
    ///
    /// `Var` nodes serialise to `{"var": "..."}` for `scope_level == 0`
    /// and to `{"val": [[<level>], ...]}` for `scope_level > 0` — that's
    /// the shape the compiler accepts on round-trip.
    ///
    /// # Example
    ///
    /// ```rust
    /// use datalogic_rs::Engine;
    ///
    /// let engine = Engine::new();
    /// let compiled = engine.compile(r#"{">": [{"var": "score"}, 90]}"#).unwrap();
    /// let json = compiled.to_json();
    /// assert!(json.contains(r#""var": "score""#));
    ///
    /// // Round-trip: re-compiling the output produces an equivalent rule.
    /// let recompiled = engine.compile(&json).unwrap();
    /// assert_eq!(
    ///     engine.eval_str(&json, r#"{"score": 95}"#).unwrap(),
    ///     "true",
    /// );
    /// # let _ = (compiled, recompiled);
    /// ```
    pub fn to_json(&self) -> String {
        crate::node_serialize::node_to_json_string(&self.root)
    }
}

impl std::fmt::Display for Logic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_json())
    }
}

/// Check if a compiled node is static (can be evaluated without runtime context).
pub(crate) fn node_is_static(node: &CompiledNode) -> bool {
    match node {
        CompiledNode::Value { .. } => true,
        CompiledNode::Array { nodes, .. } => nodes.iter().all(node_is_static),
        CompiledNode::BuiltinOperator { opcode, args, .. } => opcode.meta().can_fold(args),
        // Static only when the operator declared that its result depends on
        // its arguments alone.
        CompiledNode::CustomOperator(data) => {
            data.info.deterministic
                && !data.info.reads_context
                && data.args.iter().all(node_is_static)
        }
        CompiledNode::Cse(data) => node_is_static(&data.inner),
        CompiledNode::Var { .. } => false,
        #[cfg(feature = "ext-control")]
        CompiledNode::Exists(_) => false,
        #[cfg(feature = "error-handling")]
        CompiledNode::Throw(_) => false,
        // An escaped template is deliberately *not* static. Folding it
        // would evaluate the strip and bake the result in as an object
        // literal, so `to_json` would then emit the stripped key — and a
        // bare `type` re-parses as the `type` operator, breaking the
        // round-trip this module's callers rely on. Output templates
        // almost always contain a `var` anyway, so little folding is lost.
        #[cfg(feature = "templating")]
        CompiledNode::StructuredObject(data) => {
            data.escape.is_none() && data.fields.iter().all(|(_, node)| node_is_static(node))
        }
        CompiledNode::Missing(_) | CompiledNode::MissingSome(_) => false,
        // InvalidArgs is dynamic — it raises an error at runtime.
        CompiledNode::InvalidArgs { .. } => false,
    }
}

#[cfg(test)]
mod tests {
    use crate::Engine;

    #[test]
    fn is_constant_tracks_folding() {
        let engine = Engine::new();

        // Static expressions fold all the way down to a literal.
        let folded = engine.compile(r#"{"+": [1, {"*": [2, 3]}]}"#).unwrap();
        assert!(folded.is_constant());
        assert!(folded.is_static());

        // Bare literals (including composite ones) compile to Value nodes.
        assert!(engine.compile("42").unwrap().is_constant());
        assert!(engine.compile("[1, 2, 3]").unwrap().is_constant());

        // Data-dependent rules stay operator nodes.
        let dynamic = engine.compile(r#"{"var": "x"}"#).unwrap();
        assert!(!dynamic.is_constant());
        assert!(!dynamic.is_static());

        // `merge` needs runtime disambiguation, so it is classified
        // non-static and never folded even with literal args.
        assert!(
            !engine
                .compile(r#"{"merge": [[1], [2]]}"#)
                .unwrap()
                .is_constant()
        );

        // Static but not constant: folding `1 / 0` fails (NaN error under
        // the default config), so the operator node is kept and the error
        // is deferred to evaluation time.
        let div = engine.compile(r#"{"/": [1, 0]}"#).unwrap();
        assert!(div.is_static());
        assert!(!div.is_constant());
    }

    /// Folding classification of the collection operators: `group_by`
    /// always stays dynamic (its key expression runs under per-element
    /// frames, even when literal), `distinct` folds only in its unkeyed
    /// pure form.
    #[cfg(feature = "ext-array")]
    #[test]
    fn group_by_distinct_folding_classification() {
        let engine = Engine::new();

        // All-literal group_by is still classified dynamic.
        let grouped = engine.compile(r#"{"group_by": [[1, 2, 1], 7]}"#).unwrap();
        assert!(!grouped.is_constant());
        assert_eq!(
            engine
                .eval_str(r#"{"group_by": [[1, 2, 1], 7]}"#, "null")
                .unwrap(),
            r#"[{"key":7,"items":[1,2,1]}]"#
        );

        // Unkeyed distinct over literals folds to its result.
        let unkeyed = engine.compile(r#"{"distinct": [[1, 1, 2]]}"#).unwrap();
        assert!(unkeyed.is_constant());
        assert_eq!(
            engine
                .eval_str(r#"{"distinct": [[1, 1, 2]]}"#, "null")
                .unwrap(),
            "[1,2]"
        );

        // Keyed distinct and dynamic input stay dynamic.
        assert!(
            !engine
                .compile(r#"{"distinct": [[1, 1, 2], {"var": ""}]}"#)
                .unwrap()
                .is_constant()
        );
        assert!(
            !engine
                .compile(r#"{"distinct": [{"var": "xs"}]}"#)
                .unwrap()
                .is_constant()
        );
    }

    /// `keys` / `values` / `entries` are pure and fold when their argument
    /// is static. Null is the only literal-expressible input in strict
    /// mode (object literals in args read as operator invocations), and
    /// folds to the empty array.
    #[cfg(feature = "ext-object")]
    #[test]
    fn object_ops_fold_when_static() {
        let engine = Engine::new();
        let keys = engine.compile(r#"{"keys": [null]}"#).unwrap();
        assert!(keys.is_constant());
        assert_eq!(
            engine.eval_str(r#"{"keys": [null]}"#, "null").unwrap(),
            "[]"
        );
        assert!(
            !engine
                .compile(r#"{"keys": [{"var": "o"}]}"#)
                .unwrap()
                .is_constant()
        );
    }

    /// Composite literals are pre-built (`PreLit`) at compile time; a
    /// deep `Logic::clone` rebuilds the cells rather than sharing them,
    /// and both copies must evaluate identically even after the original
    /// is dropped.
    #[test]
    fn cloned_logic_keeps_prebuilt_composite_literals() {
        let engine = Engine::new();
        let rule = r#"{"in": [{"var": "x"}, ["a", "b", "c"]]}"#;
        let original = engine.compile(rule).unwrap();
        let cloned = original.clone();
        drop(original);
        assert_eq!(
            engine.eval_str(rule, r#"{"x": "b"}"#).unwrap(),
            "true",
            "sanity: rule matches via one-shot path"
        );
        let mut session = engine.session();
        assert_eq!(session.eval_str(&cloned, r#"{"x": "b"}"#).unwrap(), "true");
        session.reset();
        assert_eq!(session.eval_str(&cloned, r#"{"x": "z"}"#).unwrap(), "false");
    }

    /// A `switch` whose case table folds to a composite literal must still
    /// match its cases. The folded table's `PreLit` powers
    /// `evaluate_switch`'s `Value { lit: Some(..) }` arms — both for a
    /// dynamic discriminant (table folded, switch kept) and for a fully
    /// static switch (table folded, then the whole switch constant-folded
    /// at compile time, which requires the table's prebuilt view to exist
    /// *during* the fold — see `CompiledNode::compile_time_value`).
    #[cfg(feature = "ext-control")]
    #[test]
    fn folded_switch_case_tables_match() {
        let engine = Engine::new();

        // Dynamic discriminant, static (folded) case table.
        let rule = r#"{"switch": [{"var": "x"}, [[1, "one"], [2, "two"]], "dflt"]}"#;
        assert_eq!(engine.eval_str(rule, r#"{"x": 1}"#).unwrap(), "\"one\"");
        assert_eq!(engine.eval_str(rule, r#"{"x": 2}"#).unwrap(), "\"two\"");
        assert_eq!(engine.eval_str(rule, r#"{"x": 3}"#).unwrap(), "\"dflt\"");

        // Fully static switch: constant-folded at compile time.
        let folded = engine
            .compile(r#"{"switch": ["b", [["a", 1], ["b", 2]], 0]}"#)
            .unwrap();
        assert!(folded.is_constant());
        assert_eq!(
            engine.eval_str(folded.to_json().as_str(), "null").unwrap(),
            "2"
        );

        // Mixed table: one static (folded) pair among dynamic ones.
        let mixed = r#"{"switch": [{"var": "x"}, [["s", "static-hit"], [{"var": "k"}, "dyn-hit"]], "none"]}"#;
        assert_eq!(
            engine.eval_str(mixed, r#"{"x": "s", "k": "?"}"#).unwrap(),
            "\"static-hit\""
        );
        assert_eq!(
            engine.eval_str(mixed, r#"{"x": "d", "k": "d"}"#).unwrap(),
            "\"dyn-hit\""
        );
    }
}
