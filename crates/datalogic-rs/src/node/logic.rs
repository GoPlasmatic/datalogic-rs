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

/// Static operator-name lookup for the root node. Returns `Cow::Borrowed`
/// for built-ins and the named compiled-node forms (`var`, `missing`,
/// etc.) — these never allocate at compile time. `CustomOperator`
/// returns `Cow::Owned` (one allocation per compile, then re-cloneable as
/// many times as the rule errors). `Value` literals have no operator and
/// return `None`.
#[inline]
fn root_op_name(node: &CompiledNode) -> Option<std::borrow::Cow<'static, str>> {
    use std::borrow::Cow;
    match node {
        CompiledNode::BuiltinOperator { opcode, .. } => Some(Cow::Borrowed(opcode.as_str())),
        CompiledNode::Var { .. } => Some(Cow::Borrowed("var")),
        CompiledNode::Missing(_) => Some(Cow::Borrowed("missing")),
        CompiledNode::MissingSome(_) => Some(Cow::Borrowed("missing_some")),
        #[cfg(feature = "ext-control")]
        CompiledNode::Exists(_) => Some(Cow::Borrowed("exists")),
        #[cfg(feature = "error-handling")]
        CompiledNode::Throw(_) => Some(Cow::Borrowed("throw")),
        CompiledNode::CustomOperator(data) => Some(Cow::Owned(data.name.clone())),
        CompiledNode::InvalidArgs { op_name, .. } => Some(Cow::Borrowed(op_name)),
        _ => None,
    }
}

impl Logic {
    /// Creates a new compiled logic from a root node.
    ///
    /// Caches per-operator analysis results onto every `BuiltinOperator`
    /// node. Trivial literals (Null/Bool/Number/empty) are pre-built by
    /// [`super::populate::precompute_lit`] at construction; non-trivial literals
    /// (non-empty Strings/Arrays/Objects) fall through to `literal_fallback`
    /// at dispatch time.
    ///
    /// # Arguments
    ///
    /// * `root` - The root node of the compiled logic tree
    pub(crate) fn new(mut root: CompiledNode) -> Self {
        populate_lits(&mut root);
        let root_op_name = root_op_name(&root);
        Self { root, root_op_name }
    }

    /// Check if this compiled logic is static (can be evaluated without context)
    pub fn is_static(&self) -> bool {
        node_is_static(&self.root)
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
