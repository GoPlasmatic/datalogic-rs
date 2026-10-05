//! Internal helpers shared by the array operators (filter / map / reduce /
//! quantifiers / sort / slice / merge / length).

use crate::OpCode;
use crate::arena::{ContextStack, DataValue, IterGuard};
use crate::node::{MetadataHint, ReduceHint};
use crate::operators::meta::{Algebra, ArithOp, EqOp, Logic, OrdOp, Truth};
use crate::{CompiledNode, Engine, Result};
use bumpalo::Bump;
use datavalue::NumberValue;
use std::ops::ControlFlow;

/// Check if a compiled node is loop-invariant — i.e. whether hoisting it out
/// of the iteration and evaluating it once yields what the per-item path
/// would have produced. Used by the filter/quantifier fast paths.
///
/// The subtlety is what "invariant" has to mean here. The fast path skips the
/// per-item frame entirely and dispatches the hoisted operand one frame
/// shallower than its static depth, so an operand may be hoisted only when it
/// resolves to the *same physical frame* at depth `D` and at `D - 1`. Since a
/// level climbs `data_climb(level)` frames and that count does not depend on
/// the depth, the only binding with that property is the root:
///
/// - A literal takes the dispatcher's literal fast path.
/// - [`ScopeBinding::Root`] reads the rule input straight from
///   `ctx.root_input()`; a level whose climb clamps to the root at `D` still
///   clamps at `D - 1`, so the debug oracle agrees with the hoisted
///   evaluation.
/// - [`ScopeBinding::Current`] *is* the per-item frame. Not invariant.
/// - [`ScopeBinding::Ancestor`] names a frame relative to one that is not
///   pushed on this path, so at `D - 1` the same level lands one frame
///   further out. Left to the general path rather than resolved with a second
///   copy of the level arithmetic.
/// - [`ScopeBinding::Unresolved`] means the scope pass never reached this
///   node, so nothing is proven. Treated as not invariant.
///
/// A bare `scope_level > 0` test is **not** a substitute for the binding:
/// `{"val": [[1], …]}` names the *enclosing* element once the filter itself
/// sits one or more frames deep, which is the shape that made this predicate
/// wrong before. Metadata and reduce hints read a frame the level picks out
/// rather than the root, so they are never invariant; and a `default_value`
/// is an arbitrary subtree that could.
#[inline]
pub(super) fn is_filter_invariant(node: &CompiledNode) -> bool {
    match node {
        CompiledNode::Value { .. } => true,
        CompiledNode::Var {
            binding,
            reduce_hint,
            metadata_hint,
            default_value,
            ..
        } => {
            *binding == crate::node::ScopeBinding::Root
                && *reduce_hint == ReduceHint::None
                && *metadata_hint == MetadataHint::None
                && default_value.is_none()
        }
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
        && !segments.is_empty()
        && is_filter_invariant(b)
    {
        return Some((segments, b));
    }
    None
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
///
/// Measured negative result (2026-07-16, M2 Pro): replacing `var_path`
/// with a hinted-lookup path (compile-assigned cursor slots resolved
/// through a per-collection `&mut [usize]`, mirroring `FieldCursor`) made
/// macro/checkout-40 ~7% *slower* — slice-indexed cursor state defeats
/// register allocation across the recursive `evaluate_opt` walk, while a
/// linear scan over realistic (≤ 8-key) row objects with early exit is
/// already near-optimal. `FieldCursor` wins only where its single cursor
/// lives in a register-promoted local driving one loop (map fast paths,
/// the strict-eq filter path, the reduce fold). Re-attempt only with a
/// design that keeps per-leaf hint state in registers, and gate it on the
/// macro rows.
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
        op: OrdOp,
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
        match (opcode, opcode.algebra()) {
            (_, Some(Algebra::Logic(logic))) if !args.is_empty() => {
                let preds: Option<Box<[FastPredicate]>> = args
                    .iter()
                    .map(|a| Self::detect_operand(a, depth + 1))
                    .collect();
                let preds = preds?;
                Some(match logic {
                    Logic::And => FastPredicate::AllOf(preds),
                    Logic::Or => FastPredicate::AnyOf(preds),
                })
            }
            // `!` — truthiness negation. `!!` folds away entirely: the
            // consumer only reads the tree through truthiness.
            (_, Some(Algebra::Truth(Truth::Not))) if args.len() == 1 => Some(FastPredicate::Not(
                Box::new(Self::detect_operand(&args[0], depth + 1)?),
            )),
            (_, Some(Algebra::Truth(Truth::Bool))) if args.len() == 1 => {
                Self::detect_operand(&args[0], depth + 1)
            }
            // `in` with an all-string literal array: `in` compares elements
            // with strict equality, so a non-string needle is always false —
            // total semantics, no coercion involved.
            (OpCode::In, _) if args.len() == 2 => {
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
                && let CompiledNode::Value { value: literal, .. } = &pred_args[lit_idx]
            {
                let var_path: Box<[crate::node::PathSegment]> = segments.clone();

                match opcode.algebra() {
                    Some(Algebra::Eq(EqOp {
                        strict: true,
                        negate,
                    })) => {
                        return Some(FastPredicate::StrictEq {
                            var_path,
                            literal: literal.clone(),
                            negate,
                        });
                    }
                    Some(Algebra::Eq(EqOp {
                        strict: false,
                        negate,
                    })) => {
                        // For loose equality with numeric literals, we can use a fast
                        // numeric comparison (loose == is same as strict for numbers)
                        if let Some(lit_f) = literal.as_f64() {
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
                            return Some(FastPredicate::LooseStrEq {
                                var_path,
                                literal: s.as_str().into(),
                                negate,
                            });
                        }
                    }
                    Some(Algebra::Ord(op)) => {
                        if let Some(lit_f) = literal.as_f64() {
                            return Some(FastPredicate::NumericCmp {
                                var_path,
                                literal_f: lit_f,
                                op,
                                var_is_lhs,
                            });
                        }
                    }
                    _ => {}
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
                op,
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
                    Some(op.cmp_f64(lhs, rhs))
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

impl FastPredicate {
    /// Evaluate the predicate against every item of `src`, in order, handing
    /// each verdict to `on_item`, which returns [`ControlFlow::Break`] to
    /// stop early. `None` when an item is indeterminate (see
    /// [`Self::evaluate_opt`]): the caller re-runs the collection on the
    /// general path, which is exact because fast evaluation is pure.
    ///
    /// The predicate kind is matched once, outside the loop: each scalar
    /// leaf runs its own loop with its test inlined, and a numeric
    /// comparison also fixes its operator up front, so the per-item work has
    /// no call and no indirect branch. Calling [`Self::evaluate_opt`] per
    /// item put both in the loop, and that loop ran at 3.1 or 6.0 µs per 1k
    /// items depending only on where the linker placed the (byte-identical)
    /// code. Combinator trees (`and` / `or` / `!`) still walk
    /// `evaluate_opt` per item.
    #[inline]
    pub(super) fn scan<'b, F>(&self, src: &IterSrc<'b>, engine: &Engine, on_item: F) -> Option<()>
    where
        F: FnMut(&'b DataValue<'b>, bool) -> ControlFlow<()>,
    {
        match self {
            FastPredicate::NumericCmp {
                var_path,
                literal_f,
                op,
                var_is_lhs,
            } => {
                let lit = *literal_f;
                // `lit op x` is `x op.flip() lit`, so every loop below has
                // the item on the left.
                let op = if *var_is_lhs { *op } else { op.flip() };
                match op {
                    OrdOp::Gt => scan_leaf(src, var_path, on_item, |v| number(v).map(|x| x > lit)),
                    OrdOp::Ge => scan_leaf(src, var_path, on_item, |v| number(v).map(|x| x >= lit)),
                    OrdOp::Lt => scan_leaf(src, var_path, on_item, |v| number(v).map(|x| x < lit)),
                    OrdOp::Le => scan_leaf(src, var_path, on_item, |v| number(v).map(|x| x <= lit)),
                }
            }
            FastPredicate::LooseNumericEq {
                var_path,
                literal_f,
                negate,
            } => {
                let (lit, negate) = (*literal_f, *negate);
                scan_leaf(src, var_path, on_item, |v| {
                    number(v).map(|x| (x == lit) != negate)
                })
            }
            FastPredicate::StrictEq {
                var_path,
                literal,
                negate,
            } => scan_leaf(src, var_path, on_item, |v| {
                // A missing field is `var`'s implicit null.
                let av = v.unwrap_or(&DataValue::Null);
                Some(value_equals_serde(av, literal) != *negate)
            }),
            FastPredicate::LooseStrEq {
                var_path,
                literal,
                negate,
            } => scan_leaf(src, var_path, on_item, |v| match v {
                Some(DataValue::String(s)) => Some((*s == &**literal) != *negate),
                // Any other shape needs the general path's coercion table.
                _ => None,
            }),
            FastPredicate::InStrLits { var_path, items } => {
                scan_leaf(src, var_path, on_item, |v| {
                    Some(match v {
                        Some(DataValue::String(s)) => items.iter().any(|lit| &**lit == *s),
                        // Strict-equality membership: a non-string never matches.
                        _ => false,
                    })
                })
            }
            // `Truthy` only occurs inside a combinator.
            FastPredicate::Truthy { .. }
            | FastPredicate::AllOf(_)
            | FastPredicate::AnyOf(_)
            | FastPredicate::Not(_) => {
                let mut on_item = on_item;
                for item in src.0 {
                    if on_item(item, self.evaluate_opt(item, engine)?).is_break() {
                        break;
                    }
                }
                Some(())
            }
        }
    }
}

/// One scalar leaf's loop: resolve each item's value (the item itself for
/// an empty path), test it, hand the verdict on. Monomorphised per leaf
/// test, so the test is inlined into its own loop.
#[inline(always)]
fn scan_leaf<'b>(
    src: &IterSrc<'b>,
    var_path: &[crate::node::PathSegment],
    mut on_item: impl FnMut(&'b DataValue<'b>, bool) -> ControlFlow<()>,
    test: impl Fn(Option<&'b DataValue<'b>>) -> Option<bool>,
) -> Option<()> {
    // Two loops, so the whole-item case (`{"var": ""}`) never walks a path.
    if var_path.is_empty() {
        for item in src.0 {
            if on_item(item, test(Some(item))?).is_break() {
                break;
            }
        }
    } else {
        for item in src.0 {
            let value = crate::arena::value::traverse_segments(item, var_path);
            if on_item(item, test(value)?).is_break() {
                break;
            }
        }
    }
    Some(())
}

/// A native number as `f64`; anything else is indeterminate (the general
/// path's coercion table decides `"9" > 2` and `null >= 0`).
#[inline(always)]
fn number(value: Option<&DataValue<'_>>) -> Option<f64> {
    match value {
        Some(DataValue::Number(n)) => Some(n.as_f64()),
        _ => None,
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

/// An `each` row's source, resolved by the generated adapter: a null,
/// missing or empty-array source never reaches the body (the row's
/// `on_empty_source` answers it).
pub(crate) enum Items<'a> {
    /// A non-empty array.
    Array(IterSrc<'a>),
    /// An object, possibly empty: iterated as `(key, value)` pairs or
    /// rejected, per operator.
    Object(&'a [(&'a str, DataValue<'a>)]),
    /// Any other value (number, string, bool, datetime, ...).
    Scalar(&'a DataValue<'a>),
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
        && ctx.depth() == 0
    {
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
            return charged(value_as_iter(av), ctx);
        }
    }

    let av = engine.dispatch_node(arg, ctx, arena)?;
    charged(value_as_iter(av), ctx)
}

/// Charge one operation per item the caller is about to examine.
///
/// Every iterator operator funnels through [`resolve_iter_input`], so this
/// is the one place the per-item cost has to be taken — and taking it here,
/// before the caller chooses between its compile-time fast paths and the
/// general path, is what makes the cost independent of that choice. A fast
/// path that evaluates a predicate inline never dispatches the body and
/// would otherwise cost 0 per item, which would make whether a rule fits
/// its budget depend on which shape the populate pass recognised.
///
/// An object source arrives as `Bridge` and is charged one per pair, the
/// same as an array's one per item: a literal body costs nothing to
/// dispatch, so the body charge alone would price a `filter` over a large
/// object at a constant. Any other `Bridge` (a scalar) is charged 1.
#[inline(always)]
fn charged<'a>(input: ResolvedInput<'a>, ctx: &mut ContextStack<'a>) -> Result<ResolvedInput<'a>> {
    let items = match &input {
        ResolvedInput::Iterable(src) => src.len() as u64,
        ResolvedInput::Bridge(DataValue::Object(pairs)) => pairs.len() as u64,
        _ => 1,
    };
    ctx.charge(items)?;
    Ok(input)
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

// =============================================================================
// Exact arithmetic for the map / reduce fast paths
// =============================================================================

/// `a op b` with the binary arithmetic operators' representation rules,
/// for one pair: integer math when both operands are exactly `i64` (whole
/// floats included), promoting this result alone to `from_f64` on
/// overflow; `from_f64` otherwise. Generic over the two combines so each
/// operation's loop gets them inlined (see [`with_arith`]).
///
/// Every map / reduce arithmetic fast path computes element by element
/// through this, never in a separate integer pass that restarts the whole
/// collection in `f64` on the first overflow: that restart rounded exact
/// neighbours past 2^53 (`i64::MIN + 3` came back as `i64::MIN`), the bug
/// class of issue #61. Deliberately not `NumberValue::add` / `sub` / `mul`,
/// which leave an overflowed whole result as `Float` where the operators
/// collapse it back to `Integer`.
#[inline(always)]
pub(super) fn combine(
    a: NumberValue,
    b: NumberValue,
    int_op: impl Fn(i64, i64) -> Option<i64>,
    float_op: impl Fn(f64, f64) -> f64,
) -> NumberValue {
    match (a.as_i64(), b.as_i64()) {
        (Some(x), Some(y)) => match int_op(x, y) {
            Some(r) => NumberValue::from_i64(r),
            None => NumberValue::from_f64(float_op(x as f64, y as f64)),
        },
        _ => NumberValue::from_f64(float_op(a.as_f64(), b.as_f64())),
    }
}

/// [`combine`] for a runtime operation: a `match` per call. For a loop,
/// use [`with_arith`], which matches once outside it.
#[inline(always)]
pub(super) fn arith_number(op: ArithOp, a: NumberValue, b: NumberValue) -> NumberValue {
    with_arith!(op, |f| f(a, b))
}

/// Evaluate `$body` with `$int` bound to `op`'s checked integer operation
/// (`fn(i64, i64) -> Option<i64>`) and `$float` to its `f64` operation,
/// once per operation, so a loop inside `$body` does no dispatch on the
/// operation per item. For loops that classify an operand up front (a
/// literal known to be integral); everything else uses [`with_arith`].
macro_rules! with_ops {
    ($op:expr, |$int:ident, $float:ident| $body:expr) => {
        match $op {
            ArithOp::Add => {
                let ($int, $float) = (i64::checked_add, |x: f64, y: f64| x + y);
                $body
            }
            ArithOp::Sub => {
                let ($int, $float) = (i64::checked_sub, |x: f64, y: f64| x - y);
                $body
            }
            ArithOp::Mul => {
                let ($int, $float) = (i64::checked_mul, |x: f64, y: f64| x * y);
                $body
            }
        }
    };
}
pub(super) use with_ops;

/// Evaluate `$body` with `$f` bound to `op`'s exact combine
/// (`Fn(NumberValue, NumberValue) -> NumberValue`), once per operation, so
/// a loop inside `$body` does no dispatch on the operation per item.
macro_rules! with_arith {
    ($op:expr, |$f:ident| $body:expr) => {
        $crate::operators::array::helpers::with_ops!($op, |int_op, float_op| {
            let $f = |a: NumberValue, b: NumberValue| {
                $crate::operators::array::helpers::combine(a, b, int_op, float_op)
            };
            $body
        })
    };
}
pub(super) use with_arith;

/// `x op y` for an integer pair, promoting this result to `from_f64` on
/// overflow: [`combine`]'s integer arm, for loops that already know both
/// operands are integers.
#[inline(always)]
pub(super) fn combine_ints(
    x: i64,
    y: i64,
    int_op: impl Fn(i64, i64) -> Option<i64>,
    float_op: impl Fn(f64, f64) -> f64,
) -> NumberValue {
    match int_op(x, y) {
        Some(r) => NumberValue::from_i64(r),
        None => NumberValue::from_f64(float_op(x as f64, y as f64)),
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
        op: ArithOp,
        segments: &'n [crate::node::PathSegment],
        lit: &'n datavalue::OwnedDataValue,
        var_is_lhs: bool,
    },
    /// `{op: [var_a, var_b]}` for + / - / * — the line-total shape.
    ArithVarVar {
        op: ArithOp,
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
        let op = opcode.arith_op()?;
        if args.len() != 2 {
            return None;
        }
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

#[cfg(all(test, feature = "serde_json"))]
mod invariant_tests {
    use super::is_filter_invariant;
    use crate::node::CompiledNode;
    use crate::{Engine, Logic};

    /// Pull the right-hand operand out of `{"filter": [src, {"===": [a, b]}]}`.
    fn filter_rhs(logic: &Logic) -> &CompiledNode {
        let CompiledNode::BuiltinOperator { args, .. } = &logic.root else {
            panic!("expected filter at the root");
        };
        let CompiledNode::BuiltinOperator { args: pred, .. } = &args[1] else {
            panic!("expected a comparison predicate");
        };
        &pred[1]
    }

    /// Nested form: reach into `{"map": [src, {"filter": …}]}`.
    fn nested_filter_rhs(logic: &Logic) -> &CompiledNode {
        let CompiledNode::BuiltinOperator { args, .. } = &logic.root else {
            panic!("expected map at the root");
        };
        let CompiledNode::BuiltinOperator { args: inner, .. } = &args[1] else {
            panic!("expected a nested filter");
        };
        let CompiledNode::BuiltinOperator { args: pred, .. } = &inner[1] else {
            panic!("expected a comparison predicate");
        };
        &pred[1]
    }

    /// The strict-eq filter fast path evaluates a "loop-invariant" operand
    /// once, before the loop, with no per-item frame pushed. Anything that
    /// reads the frame stack must therefore be rejected, or the fast path
    /// and the general path disagree.
    ///
    /// Pins both directions: the shapes that must stay hoistable (or the fast
    /// path is silently lost) and the shapes that must not be (or results are
    /// wrong). The rejected cases below each produced a different answer from
    /// the general path before the binding check replaced a bare
    /// `scope_level > 0` test.
    #[test]
    fn only_frame_independent_operands_are_hoistable() {
        let engine = Engine::new();

        // Hoistable: a literal, and levels that resolve to the root.
        for rule in [
            r#"{"filter": [{"val": "xs"}, {"===": [{"var": "a"}, 1]}]}"#,
            r#"{"filter": [{"val": "xs"}, {"===": [{"var": "a"}, {"val": [[2], "d"]}]}]}"#,
            r#"{"filter": [{"val": "xs"}, {"===": [{"var": "a"}, {"val": [[1], "d"]}]}]}"#,
        ] {
            let logic = engine.compile(rule).unwrap();
            assert!(
                is_filter_invariant(filter_rhs(&logic)),
                "should be hoistable, losing this loses the fast path: {rule}"
            );
        }

        // Not hoistable: metadata hints read `ctx.current()` whatever the
        // level, so hoisted they would read the enclosing frame, not the item.
        for rule in [
            r#"{"filter": [{"val": "xs"}, {"===": [{"var": "a"}, {"val": [[1], "index"]}]}]}"#,
            r#"{"filter": [{"val": "xs"}, {"===": [{"var": "a"}, {"val": [[1], "key"]}]}]}"#,
        ] {
            let logic = engine.compile(rule).unwrap();
            assert!(
                !is_filter_invariant(filter_rhs(&logic)),
                "metadata hint must not be hoisted: {rule}"
            );
        }

        // Not hoistable: one frame deeper, `[[1]]` and `[[2]]` both name the
        // enclosing `map` element, which is indexed relative to the per-item
        // frame this path never pushes.
        for rule in [
            r#"{"map": [{"val": "g"}, {"filter": [{"val": "i"}, {"===": [{"var": "a"}, {"val": [[1], "a"]}]}]}]}"#,
            r#"{"map": [{"val": "g"}, {"filter": [{"val": "i"}, {"===": [{"var": "a"}, {"val": [[2], "a"]}]}]}]}"#,
        ] {
            let logic = engine.compile(rule).unwrap();
            assert!(
                !is_filter_invariant(nested_filter_rhs(&logic)),
                "an enclosing-frame reference takes the general path: {rule}"
            );
        }

        // Hoistable again at the same nesting once the level clamps to root:
        // a climb of 2 passes both frames, and still passes both when the
        // hoisted operand is dispatched one frame shallower.
        let nested_root = r#"{"map": [{"val": "g"}, {"filter": [{"val": "i"}, {"===": [{"var": "a"}, {"val": [[4], "a"]}]}]}]}"#;
        let logic = engine.compile(nested_root).unwrap();
        assert!(
            is_filter_invariant(nested_filter_rhs(&logic)),
            "a level that clamps to root stays hoistable"
        );

        // Not hoistable: a genuine ancestor frame is indexed relative to the
        // per-item frame this path never pushes. Two `map`s deep, `[[2]]`
        // names the outer map's item; the general path resolves it.
        let ancestor = r#"{"map": [{"val": "g"}, {"map": [{"val": "h"}, {"filter": [{"val": "i"}, {"===": [{"var": "a"}, {"val": [[2], "a"]}]}]}]}]}"#;
        let logic = engine.compile(ancestor).unwrap();
        let CompiledNode::BuiltinOperator { args, .. } = &logic.root else {
            panic!("expected map at the root");
        };
        let inner = nested_filter_rhs_of(&args[1]);
        assert!(
            !is_filter_invariant(inner),
            "an ancestor reference takes the general path"
        );
    }

    /// `nested_filter_rhs` for a subtree rather than a `Logic`.
    fn nested_filter_rhs_of(map: &CompiledNode) -> &CompiledNode {
        let CompiledNode::BuiltinOperator { args, .. } = map else {
            panic!("expected map");
        };
        let CompiledNode::BuiltinOperator { args: inner, .. } = &args[1] else {
            panic!("expected a nested filter");
        };
        let CompiledNode::BuiltinOperator { args: pred, .. } = &inner[1] else {
            panic!("expected a comparison predicate");
        };
        &pred[1]
    }
}
