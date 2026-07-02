//! `Logic` — the compiled, thread-safe rule snapshot returned by
//! `Engine::compile`. Includes the static-evaluation predicates the compiler
//! consults to decide whether a sub-expression can be folded.

use super::{CompiledNode, populate_lits};
use crate::opcode::OpCode;

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
    pub(crate) fn new(mut root: CompiledNode) -> Self {
        populate_lits(&mut root);
        let root_op_name = root.operator_name();
        Self { root, root_op_name }
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
        CompiledNode::BuiltinOperator { opcode, args, .. } => opcode_is_static(opcode, args),
        CompiledNode::CustomOperator(_) => false,
        CompiledNode::Var { .. } => false,
        #[cfg(feature = "ext-control")]
        CompiledNode::Exists(_) => false,
        #[cfg(feature = "error-handling")]
        CompiledNode::Throw(_) => false,
        #[cfg(feature = "templating")]
        CompiledNode::StructuredObject(data) => {
            data.fields.iter().all(|(_, node)| node_is_static(node))
        }
        CompiledNode::Missing(_) | CompiledNode::MissingSome(_) => false,
        // InvalidArgs is dynamic — it raises an error at runtime.
        CompiledNode::InvalidArgs { .. } => false,
    }
}

/// Check if an operator can be statically evaluated at compile time.
///
/// Static operators can be pre-computed during compilation when their arguments
/// are also static, eliminating runtime evaluation overhead.
///
/// # Classification Criteria
///
/// An operator is **non-static** (dynamic) if it:
/// 1. Reads from the data context (`var`, `val`, `missing`, `exists`)
/// 2. Uses iterative callbacks with changing context (`map`, `filter`, `reduce`)
/// 3. Has side effects or error handling (`try`, `throw`)
/// 4. Depends on runtime state (`now` for current time)
/// 5. Needs runtime disambiguation (`merge`, `min`, `max`)
///
/// All other operators are **static** when their arguments are static.
fn opcode_is_static(opcode: &OpCode, args: &[CompiledNode]) -> bool {
    use OpCode::*;

    // Check if all arguments are static first (common pattern)
    let args_static = || args.iter().all(node_is_static);

    match opcode {
        // Context-dependent: These operators read from the data context, which is
        // not available at compile time. They must remain dynamic.
        Val | Missing | MissingSome => false,
        #[cfg(feature = "ext-control")]
        Exists => false,

        // Iteration operators: These push new contexts for each iteration and use
        // callbacks that may reference the iteration variable. Even with static
        // arrays, the callback logic depends on the per-element context.
        Map | Filter | Reduce | All | Some | None => false,

        // Error handling: These have control flow effects (early exit, error propagation)
        // that should be preserved for runtime execution.
        #[cfg(feature = "error-handling")]
        Try | Throw => false,

        // Time-dependent: Returns current UTC time, inherently non-static.
        #[cfg(feature = "datetime")]
        Now => false,

        // Context-dependent in implicit form: when the bucketing
        // expression is omitted, `fractional` reads `$flagd.flagKey` and
        // `targetingKey` from the root data, so it cannot be folded even
        // if every literal arg is static. Even the explicit form depends
        // on the user expecting it to be evaluated per-call (the same
        // input always produces the same output, but folding bakes in
        // *one* bucketing key for the lifetime of the compiled rule —
        // which is correct, but surprises users who rebuild the rule
        // with a different bucketing strategy). Keep dynamic.
        #[cfg(feature = "flagd")]
        Fractional => false,
        // `sem_ver` is pure given static args — `Version::parse` +
        // comparison has no context dependency. Fold when every arg
        // (version1, op, version2) is a literal. The common case is
        // `sem_ver(var("app_version"), ">=", "1.2.0")` which has a
        // dynamic var and stays dynamic naturally.
        #[cfg(feature = "flagd")]
        SemVer => args_static(),

        // Runtime disambiguation needed: Merge/Min/Max have to distinguish
        // a [1,2,3] literal from operator arguments at runtime to handle
        // nested arrays correctly.
        Merge | Min | Max => false,

        // Pure operators: Static when all arguments are static. These perform
        // deterministic transformations without side effects or context access.
        _ => args_static(),
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
