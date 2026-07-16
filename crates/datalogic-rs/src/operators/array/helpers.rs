//! Internal helpers shared by the array operators (filter / map / reduce /
//! quantifiers / sort / slice / merge / length).

use crate::arena::{ContextStack, DataValue, IterGuard};
use crate::node::{MetadataHint, ReduceHint};
use crate::opcode::OpCode;
use crate::{CompiledNode, Engine, Result};
use bumpalo::Bump;
use std::ops::ControlFlow;

/// Check if a compiled node is loop-invariant (doesn't depend on the current iteration context).
/// Used by filter/quantifier fast paths to detect values that can be evaluated once before the loop.
#[inline]
pub(super) fn is_filter_invariant(node: &CompiledNode) -> bool {
    match node {
        CompiledNode::Value { .. } => true,
        CompiledNode::Var { scope_level, .. } => *scope_level > 0,
        _ => false,
    }
}

/// Try to extract filter fast-path components from a comparison pair.
/// Returns (field_segments, invariant_node) if `a` is a simple scope_level=0 field var
/// and `b` is loop-invariant (literal value or parent scope var).
#[inline]
pub(super) fn try_extract_filter_field_cmp<'a>(
    a: &'a CompiledNode,
    b: &'a CompiledNode,
) -> Option<(&'a [crate::node::PathSegment], &'a CompiledNode)> {
    if let CompiledNode::Var {
        scope_level: 0,
        segments,
        reduce_hint: ReduceHint::None,
        metadata_hint: MetadataHint::None,
        default_value: None,
        ..
    } = a
    {
        if !segments.is_empty() && is_filter_invariant(b) {
            return Some((segments, b));
        }
    }
    None
}

/// Evaluate a loop-invariant predicate-side node once, before iteration.
/// Literal values are deep-converted into the arena; outer-scope
/// `CompiledVar`s resolve through arena dispatch with a synthesized null iter
/// frame so the var sees the outer context unaffected by the missing iter
/// frame this fast path skips.
#[inline]
pub(super) fn evaluate_invariant_no_push<'a>(
    invariant_node: &'a CompiledNode,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<&'a DataValue<'a>> {
    if let CompiledNode::Value { value, .. } = invariant_node {
        return Ok(arena.alloc(value.to_arena(arena)));
    }
    let null_av: &'a DataValue<'a> = crate::arena::singletons::singleton_null();
    ctx.push(null_av);
    let result = engine.dispatch_node(invariant_node, ctx, arena);
    ctx.pop();
    result
}

/// Represents a detected fast-path predicate pattern for quantifier/filter
/// operators. Avoids per-item context push/pop and dispatch overhead.
/// `var_path` is empty when the predicate compares the whole item directly;
/// otherwise it walks into a field inside the item.
///
/// Detection is hoisted to compile time and the result is cached on the
/// predicate's own [`CompiledNode::BuiltinOperator`] node — see the
/// `predicate_hint` field. Quantifier/filter operators read the cached hint
/// instead of pattern-matching the predicate tree on every iteration.
///
/// Beyond the scalar-comparison leaves, small `and` / `or` / `!` trees over
/// such leaves are folded into `AllOf` / `AnyOf` / `Not` nodes, and three
/// more leaf shapes are recognized (bare truthy var, `in` against a
/// string-literal array, loose equality against a string literal). Every
/// shape except [`FastPredicate::LooseStrEq`] evaluates totally; that one
/// reports "indeterminate" on non-string values (coercion territory), which
/// makes the whole per-item evaluation abort so the caller can re-run the
/// collection through the general dispatch path.
#[derive(Debug, Clone)]
pub(crate) enum FastPredicate {
    /// Strict equality (===) or inequality (!==) against a literal
    StrictEq {
        var_path: Box<[crate::node::PathSegment]>,
        literal: datavalue::OwnedDataValue,
        negate: bool,
    },
    /// Ordered numeric comparison (>, >=, <, <=) against a numeric literal
    NumericCmp {
        var_path: Box<[crate::node::PathSegment]>,
        literal_f: f64,
        opcode: OpCode,
        var_is_lhs: bool,
    },
    /// Loose numeric equality (==) or inequality (!=) against a numeric literal
    LooseNumericEq {
        var_path: Box<[crate::node::PathSegment]>,
        literal_f: f64,
        negate: bool,
    },
    /// Bare `{"var": path}` used for its truthiness (e.g. an `and` arm)
    Truthy {
        var_path: Box<[crate::node::PathSegment]>,
    },
    /// Loose equality (==) or inequality (!=) against a string literal.
    /// Only string values evaluate here; anything else is indeterminate
    /// (loose-equality coercion belongs to the general path).
    LooseStrEq {
        var_path: Box<[crate::node::PathSegment]>,
        literal: Box<str>,
        negate: bool,
    },
    /// `{"in": [{var}, ["a", "b", ...]]}` — membership of the var's value
    /// in an all-string literal array. `in` uses strict equality per
    /// element, so non-string values are simply `false` — total semantics.
    InStrLits {
        var_path: Box<[crate::node::PathSegment]>,
        items: Box<[Box<str>]>,
    },
    /// `{"and": [...]}` over detected sub-predicates; short-circuits on the
    /// first false arm, matching `evaluate_and`'s left-to-right order.
    AllOf(Box<[FastPredicate]>),
    /// `{"or": [...]}` over detected sub-predicates; short-circuits on true.
    AnyOf(Box<[FastPredicate]>),
    /// `{"!": pred}` — truthiness negation of a detected sub-predicate.
    Not(Box<FastPredicate>),
}

/// Recursion guard for compound-predicate detection. Real filter/quantifier
/// predicates are shallow (`and(not(in(...)), cmp)` is depth 3); the cap
/// only bounds pathological rule shapes from doing quadratic populate work.
const MAX_PREDICATE_DEPTH: u32 = 4;

impl FastPredicate {
    /// Try to detect a fast predicate pattern from a compiled predicate's
    /// `(opcode, args)` shape. Called at compile time during the post-compile
    /// populate pass so the result can be cached on the node and reused for
    /// every evaluation. Owns its `var_path` and `literal` so the cached
    /// hint has no lifetime tie to the args slice.
    pub(crate) fn try_detect_owned(opcode: OpCode, pred_args: &[CompiledNode]) -> Option<Self> {
        Self::detect_op(opcode, pred_args, 0)
    }

    /// Detection for one operator node, recursing through the compound
    /// combinators (`and` / `or` / `!` / `!!`). A single non-detectable arm
    /// anywhere makes the whole tree non-detectable — the general dispatch
    /// path keeps full semantics (including error propagation from impure
    /// sub-expressions, which detected trees can never contain).
    fn detect_op(opcode: OpCode, args: &[CompiledNode], depth: u32) -> Option<Self> {
        if depth >= MAX_PREDICATE_DEPTH {
            return None;
        }
        match opcode {
            OpCode::And | OpCode::Or if !args.is_empty() => {
                let preds: Option<Box<[FastPredicate]>> = args
                    .iter()
                    .map(|a| Self::detect_operand(a, depth + 1))
                    .collect();
                let preds = preds?;
                Some(if matches!(opcode, OpCode::And) {
                    FastPredicate::AllOf(preds)
                } else {
                    FastPredicate::AnyOf(preds)
                })
            }
            // `!` — truthiness negation. `!!` folds away entirely: the
            // consumer only reads the tree through truthiness.
            OpCode::Not if args.len() == 1 => Some(FastPredicate::Not(Box::new(
                Self::detect_operand(&args[0], depth + 1)?,
            ))),
            OpCode::BoolCast if args.len() == 1 => Self::detect_operand(&args[0], depth + 1),
            // `in` with an all-string literal array: `in` compares elements
            // with strict equality, so a non-string needle is always false —
            // total semantics, no coercion involved.
            OpCode::In if args.len() == 2 => {
                let CompiledNode::Var {
                    scope_level: 0,
                    segments,
                    reduce_hint: ReduceHint::None,
                    metadata_hint: MetadataHint::None,
                    default_value: None,
                    ..
                } = &args[0]
                else {
                    return None;
                };
                let CompiledNode::Value {
                    value: datavalue::OwnedDataValue::Array(items),
                    ..
                } = &args[1]
                else {
                    return None;
                };
                let strs: Option<Box<[Box<str>]>> = items
                    .iter()
                    .map(|it| match it {
                        datavalue::OwnedDataValue::String(s) => Some(s.as_str().into()),
                        _ => None,
                    })
                    .collect();
                Some(FastPredicate::InStrLits {
                    var_path: segments.clone(),
                    items: strs?,
                })
            }
            _ => Self::detect_cmp_leaf(opcode, args),
        }
    }

    /// Detection for a combinator operand: a bare scope-0 `var` is a
    /// truthiness test; a nested operator recurses through [`Self::detect_op`].
    fn detect_operand(node: &CompiledNode, depth: u32) -> Option<Self> {
        match node {
            CompiledNode::Var {
                scope_level: 0,
                segments,
                reduce_hint: ReduceHint::None,
                metadata_hint: MetadataHint::None,
                default_value: None,
                ..
            } => Some(FastPredicate::Truthy {
                var_path: segments.clone(),
            }),
            CompiledNode::BuiltinOperator { opcode, args, .. } => {
                Self::detect_op(*opcode, args, depth)
            }
            _ => None,
        }
    }

    /// The original two-arg `(var, literal)` comparison-leaf detection.
    fn detect_cmp_leaf(opcode: OpCode, pred_args: &[CompiledNode]) -> Option<Self> {
        if pred_args.len() != 2 {
            return None;
        }
        // Try both orderings: (var, literal) and (literal, var)
        for (var_idx, lit_idx, var_is_lhs) in [(0, 1, true), (1, 0, false)] {
            if let CompiledNode::Var {
                scope_level: 0,
                segments,
                reduce_hint: ReduceHint::None,
                metadata_hint: MetadataHint::None,
                default_value: None,
                ..
            } = &pred_args[var_idx]
            {
                if let CompiledNode::Value { value: literal, .. } = &pred_args[lit_idx] {
                    let var_path: Box<[crate::node::PathSegment]> = segments.clone();

                    match opcode {
                        OpCode::StrictEquals | OpCode::StrictNotEquals => {
                            let negate = matches!(opcode, OpCode::StrictNotEquals);
                            return Some(FastPredicate::StrictEq {
                                var_path,
                                literal: literal.clone(),
                                negate,
                            });
                        }
                        OpCode::Equals | OpCode::NotEquals => {
                            // For loose equality with numeric literals, we can use a fast
                            // numeric comparison (loose == is same as strict for numbers)
                            if let Some(lit_f) = literal.as_f64() {
                                let negate = matches!(opcode, OpCode::NotEquals);
                                return Some(FastPredicate::LooseNumericEq {
                                    var_path,
                                    literal_f: lit_f,
                                    negate,
                                });
                            }
                            // String literals: same-type loose equality is
                            // plain equality; other value types stay
                            // indeterminate at evaluation time.
                            if let datavalue::OwnedDataValue::String(s) = literal {
                                let negate = matches!(opcode, OpCode::NotEquals);
                                return Some(FastPredicate::LooseStrEq {
                                    var_path,
                                    literal: s.as_str().into(),
                                    negate,
                                });
                            }
                        }
                        OpCode::GreaterThan
                        | OpCode::GreaterThanEqual
                        | OpCode::LessThan
                        | OpCode::LessThanEqual => {
                            if let Some(lit_f) = literal.as_f64() {
                                return Some(FastPredicate::NumericCmp {
                                    var_path,
                                    literal_f: lit_f,
                                    opcode,
                                    var_is_lhs,
                                });
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        None
    }

    /// Look up the cached predicate hint on a compiled predicate node.
    /// Returns `None` when the predicate isn't a `BuiltinOperator` or the
    /// detection didn't match a fast pattern.
    #[inline]
    pub(super) fn from_node(predicate: &CompiledNode) -> Option<&FastPredicate> {
        if let CompiledNode::BuiltinOperator { predicate_hint, .. } = predicate {
            return predicate_hint.as_deref();
        }
        None
    }

    /// Resolve the value to compare: either the whole item or a field within it.
    #[inline(always)]
    fn resolve_value<'b>(
        segments: &[crate::node::PathSegment],
        item: &'b DataValue<'b>,
    ) -> Option<&'b DataValue<'b>> {
        if segments.is_empty() {
            Some(item)
        } else {
            crate::arena::value::traverse_segments(item, segments)
        }
    }

    /// Evaluate this predicate against a single item. `None` means
    /// "indeterminate" — the item's value shape needs coercion semantics
    /// only the general dispatch path implements, so the caller must
    /// abandon the fast path for the whole collection (fast evaluation is
    /// pure, so a re-run through the general path is exact).
    ///
    /// `#[inline]` (not `always`): the scalar leaves still collapse into
    /// the per-item loops, while the recursive combinator arms keep
    /// codegen from ballooning.
    #[inline]
    pub(super) fn evaluate_opt<'b>(
        &self,
        item: &'b DataValue<'b>,
        engine: &crate::Engine,
    ) -> Option<bool> {
        match self {
            FastPredicate::StrictEq {
                var_path,
                literal,
                negate,
            } => {
                // A missing field resolves to `var`'s implicit null default,
                // which strict-compares like any other value (`null === null`
                // is true) — total semantics, no coercion anywhere in `===`.
                let av = Self::resolve_value(var_path, item).unwrap_or(&DataValue::Null);
                Some(value_equals_serde(av, literal) != *negate)
            }
            FastPredicate::NumericCmp {
                var_path,
                literal_f,
                opcode,
                var_is_lhs,
            } => match Self::resolve_value(var_path, item) {
                // Only native numbers compare here. Anything else (string,
                // bool, null, missing) goes through the general path's
                // coercion table — `"9" >= 2` and `null >= 0` are true there.
                Some(DataValue::Number(n)) => {
                    let val_f = n.as_f64();
                    let (lhs, rhs) = if *var_is_lhs {
                        (val_f, *literal_f)
                    } else {
                        (*literal_f, val_f)
                    };
                    Some(inline_numeric_cmp(lhs, rhs, *opcode))
                }
                _ => None,
            },
            FastPredicate::LooseNumericEq {
                var_path,
                literal_f,
                negate,
            } => match Self::resolve_value(var_path, item) {
                // Same-type loose equality only; `"5" == 5` / `true == 1`
                // need the general path's coercion table.
                Some(DataValue::Number(n)) => Some((n.as_f64() == *literal_f) != *negate),
                _ => None,
            },
            FastPredicate::Truthy { var_path } => Some(match Self::resolve_value(var_path, item) {
                Some(av) => crate::arena::truthy_arena(av, engine),
                None => false,
            }),
            FastPredicate::LooseStrEq {
                var_path,
                literal,
                negate,
            } => match Self::resolve_value(var_path, item) {
                // Same-type loose equality is plain equality; any other
                // value shape (including a missing field's implicit null)
                // needs the general path's coercion table.
                Some(DataValue::String(s)) => Some((*s == &**literal) != *negate),
                _ => None,
            },
            FastPredicate::InStrLits { var_path, items } => {
                let found = match Self::resolve_value(var_path, item) {
                    Some(DataValue::String(s)) => items.iter().any(|lit| &**lit == *s),
                    // `in` is strict-equality membership: a non-string (or
                    // missing) needle never equals a string literal.
                    _ => false,
                };
                Some(found)
            }
            FastPredicate::AllOf(preds) => {
                for p in preds.iter() {
                    if !p.evaluate_opt(item, engine)? {
                        return Some(false);
                    }
                }
                Some(true)
            }
            FastPredicate::AnyOf(preds) => {
                for p in preds.iter() {
                    if p.evaluate_opt(item, engine)? {
                        return Some(true);
                    }
                }
                Some(false)
            }
            FastPredicate::Not(inner) => inner.evaluate_opt(item, engine).map(|b| !b),
        }
    }
}

/// Strict equality between a [`DataValue`] (arena) and an
/// [`OwnedDataValue`] literal — used by `FastPredicate::StrictEq` to
/// compare an arena-resident item against a compile-time literal without
/// allocating.
///
/// Scalar arms (Null/Bool/Number/String) live in the `#[inline(always)]`
/// entry point so the dominant `filter(arr, == [{var}, scalar])` shape
/// compiles down to a few branches at the call site. Compound arms
/// (Array/Object) trampoline to an outlined helper to keep the inlined
/// body small.
#[inline(always)]
fn value_equals_serde(av: &DataValue<'_>, v: &datavalue::OwnedDataValue) -> bool {
    use datavalue::OwnedDataValue;
    match (av, v) {
        (DataValue::Null, OwnedDataValue::Null) => true,
        (DataValue::Bool(a), OwnedDataValue::Bool(b)) => a == b,
        (DataValue::Number(a), OwnedDataValue::Number(b)) => a == b,
        (DataValue::String(s), OwnedDataValue::String(b)) => *s == b.as_str(),
        (DataValue::Array(_), OwnedDataValue::Array(_))
        | (DataValue::Object(_), OwnedDataValue::Object(_)) => value_equals_serde_compound(av, v),
        _ => false,
    }
}

/// Compound (Array/Object) cases of [`value_equals_serde`]. Outlined
/// so the recursive body never gets inlined into the per-item fast path.
#[inline(never)]
fn value_equals_serde_compound(av: &DataValue<'_>, v: &datavalue::OwnedDataValue) -> bool {
    use datavalue::OwnedDataValue;
    match (av, v) {
        (DataValue::Array(items), OwnedDataValue::Array(b)) => {
            items.len() == b.len()
                && items
                    .iter()
                    .zip(b.iter())
                    .all(|(x, y)| value_equals_serde(x, y))
        }
        (DataValue::Object(pairs), OwnedDataValue::Object(b)) => {
            if pairs.len() != b.len() {
                return false;
            }
            for (k, av) in *pairs {
                match b.iter().find(|(bk, _)| bk == *k) {
                    Some((_, bv)) => {
                        if !value_equals_serde(av, bv) {
                            return false;
                        }
                    }
                    None => return false,
                }
            }
            true
        }
        _ => false,
    }
}

/// Inline numeric comparison for fast predicate evaluation.
#[inline(always)]
fn inline_numeric_cmp(lhs: f64, rhs: f64, opcode: OpCode) -> bool {
    match opcode {
        OpCode::GreaterThan => lhs > rhs,
        OpCode::GreaterThanEqual => lhs >= rhs,
        OpCode::LessThan => lhs < rhs,
        OpCode::LessThanEqual => lhs <= rhs,
        _ => false,
    }
}

// =============================================================================
// Iterator input resolution
// =============================================================================

/// Unified view over an iterator op's input collection. Single shape:
/// arena slice of `DataValue`. Wrapper kept for API stability.
#[derive(Clone, Copy)]
pub(crate) struct IterSrc<'a>(pub(crate) &'a [DataValue<'a>]);

impl<'a> IterSrc<'a> {
    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }

    #[inline]
    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Get item by index.
    #[inline]
    pub(crate) fn get(&self, i: usize) -> &'a DataValue<'a> {
        &self.0[i]
    }
}

/// Outcome of resolving an iterator op's first arg in arena mode.
pub(crate) enum ResolvedInput<'a> {
    /// Iterable input — proceed with array iteration.
    Iterable(IterSrc<'a>),
    /// Empty/null input — caller returns its empty-collection result.
    Empty,
    /// A non-array, non-null resolved value (Object, scalar, datetime, ...).
    /// Carries the resolved arena value so callers can dispatch natively
    /// (object-iteration / error / ...) without re-evaluating the arg.
    ///
    /// **Invariant:** `value_as_iter` routes arrays to [`Self::Iterable`] and
    /// null to [`Self::Empty`], so a `Bridge` payload is never
    /// `DataValue::Array` or `DataValue::Null`. Consumers rely on this to omit
    /// those match arms.
    Bridge(&'a DataValue<'a>),
}

/// Compile-time classification of an iterator op's `args[0]` shape.
/// Stored on the parent `BuiltinOperator` (filter/map/all/some/none/reduce
/// /merge/min/max) and consulted by `resolve_iter_input` so the runtime
/// shape match collapses to a single byte compare.
///
/// `RootVarBorrow` covers the dominant pattern: `args[0]` is a plain
/// `{var: "..."}` against the root frame — we can read directly from
/// `ctx.root_input()` without dispatching into the arena evaluator. Any
/// other shape, including nested operators, falls through to `General`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IterArgKind {
    /// `args[0]` is a `CompiledVar { scope_level: 0, … }` with no
    /// metadata/reduce/default — borrow directly from the root frame.
    /// `path_segments_empty == true` short-circuits the per-call segment
    /// length check inside `resolve_iter_input`.
    RootVarBorrow { path_segments_empty: bool },
    /// `args[0]` is anything else — evaluate via the dispatcher.
    General,
}

impl IterArgKind {
    /// Classify `args[0]` at compile time. Called from
    /// [`crate::node::populate_lits`] whenever the parent
    /// `BuiltinOperator` is one of the iterator ops listed above.
    pub(crate) fn classify(arg: &CompiledNode) -> Self {
        if let CompiledNode::Var {
            scope_level: 0,
            segments,
            reduce_hint: ReduceHint::None,
            metadata_hint: MetadataHint::None,
            default_value: None,
            ..
        } = arg
        {
            return IterArgKind::RootVarBorrow {
                path_segments_empty: segments.is_empty(),
            };
        }
        IterArgKind::General
    }
}

/// Resolve `args[0]` for an iterator op given the compile-time kind cached on
/// the parent. Two paths only:
///   - **Root borrow**: traverse `ctx.root_input()` directly when we can —
///     the dominant pattern in real workloads, reached in one byte compare.
///   - **General**: dispatch through the arena evaluator (covers composition
///     with another arena op, expressions, primitives — the dispatcher itself
///     handles those branches).
///
/// `ctx.depth() != 0` falls through to General even when the kind is
/// `RootVarBorrow`, because a borrow at non-root depth would leak the caller's
/// iteration frame instead of reading the rule's input.
///
/// `#[inline(always)]` because every iterator-op call funnels through here —
/// outlining was paying a function call for every quantifier/filter/map/
/// reduce despite the body being short and largely constant-foldable from
/// the call site (the cached `IterArgKind` tag).
#[inline(always)]
pub(crate) fn resolve_iter_input<'a>(
    arg: &'a CompiledNode,
    kind: IterArgKind,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
) -> Result<ResolvedInput<'a>> {
    if let IterArgKind::RootVarBorrow {
        path_segments_empty,
    } = kind
    {
        if ctx.depth() == 0 {
            let root = ctx.root_input();
            let av = if path_segments_empty {
                Some(root)
            } else if let CompiledNode::Var { segments, .. } = arg {
                crate::arena::value::traverse_segments(root, segments)
            } else {
                // Compile-time invariant violated; fall through to General path.
                None
            };
            if let Some(av) = av {
                return Ok(value_as_iter(av));
            }
        }
    }

    let av = engine.dispatch_node(arg, ctx, arena)?;
    Ok(value_as_iter(av))
}

/// Convert a resolved arena value into an `IterSrc` view, or signal Empty/Bridge.
#[inline]
fn value_as_iter<'a>(av: &'a DataValue<'a>) -> ResolvedInput<'a> {
    match av {
        DataValue::Null => ResolvedInput::Empty,
        DataValue::Array(items) => ResolvedInput::Iterable(IterSrc(items)),
        _ => ResolvedInput::Bridge(av),
    }
}

/// True iff this arena value is `null`.
#[inline]
pub(super) fn item_is_null(av: &DataValue<'_>) -> bool {
    matches!(av, DataValue::Null)
}

// =============================================================================
// Per-iteration body machinery (filter / map / quantifiers)
// =============================================================================
//
// `for_each_iter_array` / `for_each_iter_object` factor out the common loop:
// `IterGuard::new` + `step_indexed`/`step_keyed` + `run_iter_body`. Each call
// site supplies a closure that consumes `(i, item, [key], body_result)` and
// returns `ControlFlow` to support short-circuit (quantifiers); filter/map
// always return `Continue`.
//
// Reduce is intentionally NOT factored through these — its `step_reduce`
// frame shape (item + accumulator) differs from the indexed/keyed shape.

/// Iterate `items` with an indexed iter frame pushed for each element.
/// `step_fn` receives `(i, item, body_result)` and may break the loop.
#[inline]
pub(super) fn for_each_iter_array<'a, F>(
    items: &'a [DataValue<'a>],
    body: &'a CompiledNode,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
    mut step_fn: F,
) -> Result<()>
where
    F: FnMut(usize, &'a DataValue<'a>, &'a DataValue<'a>) -> Result<ControlFlow<()>>,
{
    let total = items.len() as u32;
    let mut guard = IterGuard::new(ctx);
    for (i, item_av) in items.iter().enumerate() {
        guard.step_indexed(item_av, i);
        let av = engine.run_iter_body(body, guard.stack(), arena, i as u32, total)?;
        if step_fn(i, item_av, av)?.is_break() {
            break;
        }
    }
    drop(guard);
    Ok(())
}

/// Iterate object `pairs` with a keyed iter frame pushed for each (key, value).
/// `step_fn` receives `(i, item, key, body_result)` and may break the loop.
#[inline]
pub(super) fn for_each_iter_object<'a, F>(
    pairs: &'a [(&'a str, DataValue<'a>)],
    body: &'a CompiledNode,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
    mut step_fn: F,
) -> Result<()>
where
    F: FnMut(usize, &'a DataValue<'a>, &'a str, &'a DataValue<'a>) -> Result<ControlFlow<()>>,
{
    let total = pairs.len() as u32;
    let mut guard = IterGuard::new(ctx);
    for (i, (k, v)) in pairs.iter().enumerate() {
        guard.step_keyed(v, i, k);
        let av = engine.run_iter_body(body, guard.stack(), arena, i as u32, total)?;
        if step_fn(i, v, k, av)?.is_break() {
            break;
        }
    }
    drop(guard);
    Ok(())
}

/// Per-loop resolver for a scope-0 var path against successive row items.
/// Single object-key paths (the dominant row shape) carry a remembered pair
/// index across rows — see `object_lookup_field_hinted` — so homogeneous
/// rows resolve in one key compare after the first. Everything else
/// delegates to the general segment traversal. Shared by the map fast
/// paths and the reduce(map(...)) fusion loops.
pub(super) struct FieldCursor<'n> {
    segments: &'n [crate::node::PathSegment],
    /// Key of a single-`Field`/`FieldOrIndex` segment path, when applicable.
    single_key: Option<&'n str>,
    /// Last hit index for the hinted lookup.
    hint: usize,
}

impl<'n> FieldCursor<'n> {
    #[inline]
    pub(super) fn new(segments: &'n [crate::node::PathSegment]) -> Self {
        use crate::node::PathSegment;
        let single_key = match segments {
            [PathSegment::Field(k)] => Some(k.as_ref()),
            [PathSegment::FieldOrIndex(k, _)] => Some(k.as_ref()),
            _ => None,
        };
        Self {
            segments,
            single_key,
            hint: 0,
        }
    }

    #[inline(always)]
    pub(super) fn resolve<'a>(&mut self, item: &'a DataValue<'a>) -> Option<&'a DataValue<'a>> {
        if let (Some(key), DataValue::Object(pairs)) = (self.single_key, item) {
            return crate::arena::value::object_lookup_field_hinted(pairs, key, &mut self.hint);
        }
        if self.segments.is_empty() {
            Some(item)
        } else {
            crate::arena::value::traverse_segments(item, self.segments)
        }
    }
}

/// Classified fusible map body shape — the three per-item transforms the
/// map fast paths execute without per-item context pushes. Shared with the
/// reduce(map(...)) fusion in `reduce.rs`, which runs the same transforms
/// feeding a fold instead of materializing the intermediate array.
pub(super) enum FusedMapBody<'n> {
    /// `{"var": "path"}` — extract (identity when segments are empty).
    Extract {
        segments: &'n [crate::node::PathSegment],
    },
    /// `{op: [var, literal]}` or `{op: [literal, var]}` for + / - / *.
    ArithVarLit {
        op: OpCode,
        segments: &'n [crate::node::PathSegment],
        lit: &'n datavalue::OwnedDataValue,
        var_is_lhs: bool,
    },
    /// `{op: [var_a, var_b]}` for + / - / * — the line-total shape.
    ArithVarVar {
        op: OpCode,
        a_segments: &'n [crate::node::PathSegment],
        b_segments: &'n [crate::node::PathSegment],
    },
}

/// Match a plain scope-0 var with no reduce/metadata hints and no default —
/// the only var shape the fused loops can resolve with a `FieldCursor`.
#[inline]
fn plain_var_segments(node: &CompiledNode) -> Option<&[crate::node::PathSegment]> {
    if let CompiledNode::Var {
        scope_level: 0,
        segments,
        reduce_hint: ReduceHint::None,
        metadata_hint: MetadataHint::None,
        default_value: None,
        ..
    } = node
    {
        Some(segments.as_ref())
    } else {
        None
    }
}

impl FusedMapBody<'_> {
    /// Structural classification only — numeric checks (e.g. the literal
    /// being coercible) stay in the executing loops, which fall back to
    /// the general path when they fail.
    pub(super) fn detect(body: &CompiledNode) -> Option<FusedMapBody<'_>> {
        if let Some(segments) = plain_var_segments(body) {
            return Some(FusedMapBody::Extract { segments });
        }
        let CompiledNode::BuiltinOperator { opcode, args, .. } = body else {
            return None;
        };
        if args.len() != 2 || !matches!(opcode, OpCode::Add | OpCode::Subtract | OpCode::Multiply) {
            return None;
        }
        let op = *opcode;
        match (&args[0], &args[1]) {
            (var, CompiledNode::Value { value, .. }) => {
                let segments = plain_var_segments(var)?;
                Some(FusedMapBody::ArithVarLit {
                    op,
                    segments,
                    lit: value,
                    var_is_lhs: true,
                })
            }
            (CompiledNode::Value { value, .. }, var) => {
                let segments = plain_var_segments(var)?;
                Some(FusedMapBody::ArithVarLit {
                    op,
                    segments,
                    lit: value,
                    var_is_lhs: false,
                })
            }
            (a, b) => {
                let a_segments = plain_var_segments(a)?;
                let b_segments = plain_var_segments(b)?;
                Some(FusedMapBody::ArithVarVar {
                    op,
                    a_segments,
                    b_segments,
                })
            }
        }
    }
}
