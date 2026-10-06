//! The filter / quantifier fast predicates: compile-time detection of a
//! predicate shape ([`FastPredicate`]), its inline per-item evaluation, and
//! the loop-invariance test the strict-equality filter path relies on.

use crate::OpCode;
use crate::arena::DataValue;
use crate::node::{MetadataHint, ReduceHint};
use crate::operators::meta::{Algebra, EqOp, Logic, OrdOp, Truth};
use crate::{CompiledNode, Engine};
use std::ops::ControlFlow;

use super::input::{IterSrc, plain_var_segments};

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
    let segments = plain_var_segments(a)?;
    (!segments.is_empty() && is_filter_invariant(b)).then_some((segments, b))
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
    /// `{"!!": pred}` — the truthiness of a detected sub-predicate. It
    /// evaluates as `pred` does; it is kept as its own node so the metered
    /// count includes the `!!` node the general path dispatches.
    Bool(Box<FastPredicate>),
}

/// A numeric literal as the `f64` the numeric leaves compare items with,
/// when that comparison gives `NumberValue`'s answer for every item: a
/// float literal (an integer item meets it as `f64` there too), or an
/// integer literal below 2^53 in magnitude, against which an integer item
/// rounded to `f64` keeps its order and its (in)equality. A larger integer
/// literal is left to the general path, which compares two integers
/// exactly.
fn f64_exact_literal(literal: &datavalue::OwnedDataValue) -> Option<f64> {
    const EXACT: i64 = 1 << 53;
    match literal {
        datavalue::OwnedDataValue::Number(datavalue::NumberValue::Integer(i))
            if !(-EXACT < *i && *i < EXACT) =>
        {
            None
        }
        _ => literal.as_f64(),
    }
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
            // `!` — truthiness negation. `!!` evaluates as its operand (the
            // consumer only reads the tree through truthiness) but stays a
            // node, for the metered count.
            (_, Some(Algebra::Truth(Truth::Not))) if args.len() == 1 => Some(FastPredicate::Not(
                Box::new(Self::detect_operand(&args[0], depth + 1)?),
            )),
            (_, Some(Algebra::Truth(Truth::Bool))) if args.len() == 1 => Some(FastPredicate::Bool(
                Box::new(Self::detect_operand(&args[0], depth + 1)?),
            )),
            // `in` with an all-string literal array: `in` compares elements
            // with strict equality, so a non-string needle is always false —
            // total semantics, no coercion involved.
            (OpCode::In, _) if args.len() == 2 => {
                let segments = plain_var_segments(&args[0])?;
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
                    var_path: segments.into(),
                    items: strs?,
                })
            }
            _ => Self::detect_cmp_leaf(opcode, args),
        }
    }

    /// Detection for a combinator operand: a bare scope-0 `var` is a
    /// truthiness test; a nested operator recurses through [`Self::detect_op`].
    fn detect_operand(node: &CompiledNode, depth: u32) -> Option<Self> {
        if let Some(segments) = plain_var_segments(node) {
            return Some(FastPredicate::Truthy {
                var_path: segments.into(),
            });
        }
        match node {
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
            if let Some(segments) = plain_var_segments(&pred_args[var_idx])
                && let CompiledNode::Value { value: literal, .. } = &pred_args[lit_idx]
            {
                let var_path: Box<[crate::node::PathSegment]> = segments.into();

                match opcode.algebra() {
                    // Not against an array or object literal: `===` charges
                    // a structural walk of two containers, which this leaf
                    // does not price.
                    Some(Algebra::Eq(EqOp {
                        strict: true,
                        negate,
                    })) if !matches!(
                        literal,
                        datavalue::OwnedDataValue::Array(_) | datavalue::OwnedDataValue::Object(_)
                    ) =>
                    {
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
                        if let Some(lit_f) = f64_exact_literal(literal) {
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
                        if let Some(lit_f) = f64_exact_literal(literal) {
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

    /// Operations the general path charges for one evaluation of a scalar
    /// leaf: the comparison node and its `var` (the literal operand is
    /// free), plus the haystack length `in` charges. Combinators are priced
    /// per evaluation in [`Self::evaluate_opt`], since they short-circuit.
    #[inline(always)]
    fn leaf_cost(&self) -> u64 {
        match self {
            FastPredicate::Truthy { .. } => 1,
            FastPredicate::InStrLits { items, .. } => 2 + items.len() as u64,
            _ => 2,
        }
    }

    /// Evaluate this predicate against a single item, adding to `cost` the
    /// operations the general path would have charged for it (the nodes it
    /// dispatches, short-circuiting included). `None` means
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
        cost: &mut u64,
    ) -> Option<bool> {
        match self {
            FastPredicate::AllOf(preds) => {
                *cost += 1;
                for p in preds.iter() {
                    if !p.evaluate_opt(item, engine, cost)? {
                        return Some(false);
                    }
                }
                Some(true)
            }
            FastPredicate::AnyOf(preds) => {
                *cost += 1;
                for p in preds.iter() {
                    if p.evaluate_opt(item, engine, cost)? {
                        return Some(true);
                    }
                }
                Some(false)
            }
            FastPredicate::Not(inner) => {
                *cost += 1;
                inner.evaluate_opt(item, engine, cost).map(|b| !b)
            }
            FastPredicate::Bool(inner) => {
                *cost += 1;
                inner.evaluate_opt(item, engine, cost)
            }
            leaf => {
                *cost += leaf.leaf_cost();
                leaf.evaluate_leaf(item, engine)
            }
        }
    }

    /// [`Self::evaluate_opt`] for a scalar leaf, uncounted.
    #[inline]
    fn evaluate_leaf<'b>(&self, item: &'b DataValue<'b>, engine: &crate::Engine) -> Option<bool> {
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
                strict_eq_literal(av, literal).map(|eq| eq != *negate)
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
                // Same-type loose equality is plain equality, short of the
                // datetime probe; any other value shape (including a
                // missing field's implicit null) needs the general path's
                // coercion table.
                Some(DataValue::String(s)) => str_eq(s, literal).map(|eq| eq != *negate),
                _ => None,
            },
            FastPredicate::InStrLits { var_path, items } => {
                match Self::resolve_value(var_path, item) {
                    Some(DataValue::String(s)) => str_in(s, items),
                    // `in` is strict-equality membership: a non-string (or
                    // missing) needle never equals a string literal.
                    _ => Some(false),
                }
            }
            // Combinators evaluate in `evaluate_opt`; never reached, and
            // "indeterminate" would hand the item to the exact general path.
            FastPredicate::AllOf(_)
            | FastPredicate::AnyOf(_)
            | FastPredicate::Not(_)
            | FastPredicate::Bool(_) => None,
        }
    }
}

impl FastPredicate {
    /// Evaluate the predicate against every item of `src`, in order, handing
    /// each verdict to `on_item`, which returns [`ControlFlow::Break`] to
    /// stop early. Returns the operations the general path would have
    /// charged for the predicate over the items examined, for the caller to
    /// charge: the count is the same whichever path runs. `None` when an
    /// item is indeterminate (see [`Self::evaluate_opt`]): the caller
    /// re-runs the collection on the general path, which is exact because
    /// fast evaluation is pure, and charges nothing here.
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
    pub(super) fn scan<'b, F>(&self, src: &IterSrc<'b>, engine: &Engine, on_item: F) -> Option<u64>
    where
        F: FnMut(&'b DataValue<'b>, bool) -> ControlFlow<()>,
    {
        let examined = match self {
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
                strict_eq_literal(av, literal).map(|eq| eq != *negate)
            }),
            FastPredicate::LooseStrEq {
                var_path,
                literal,
                negate,
            } => scan_leaf(src, var_path, on_item, |v| match v {
                Some(DataValue::String(s)) => str_eq(s, literal).map(|eq| eq != *negate),
                // Any other shape needs the general path's coercion table.
                _ => None,
            }),
            FastPredicate::InStrLits { var_path, items } => {
                scan_leaf(src, var_path, on_item, |v| match v {
                    Some(DataValue::String(s)) => str_in(s, items),
                    // Strict-equality membership: a non-string never matches.
                    _ => Some(false),
                })
            }
            // `Truthy` only occurs inside a combinator.
            FastPredicate::Truthy { .. }
            | FastPredicate::AllOf(_)
            | FastPredicate::AnyOf(_)
            | FastPredicate::Not(_)
            | FastPredicate::Bool(_) => {
                let mut on_item = on_item;
                let mut cost = 0u64;
                for item in src.0 {
                    if on_item(item, self.evaluate_opt(item, engine, &mut cost)?).is_break() {
                        break;
                    }
                }
                return Some(cost);
            }
        }?;
        Some(examined as u64 * self.leaf_cost())
    }
}

/// One scalar leaf's loop: resolve each item's value (the item itself for
/// an empty path), test it, hand the verdict on. Monomorphised per leaf
/// test, so the test is inlined into its own loop. Returns how many items
/// were examined (all of them, unless `on_item` broke off).
#[inline(always)]
fn scan_leaf<'b>(
    src: &IterSrc<'b>,
    var_path: &[crate::node::PathSegment],
    mut on_item: impl FnMut(&'b DataValue<'b>, bool) -> ControlFlow<()>,
    test: impl Fn(Option<&'b DataValue<'b>>) -> Option<bool>,
) -> Option<usize> {
    // Two loops, so the whole-item case (`{"var": ""}`) never walks a path.
    // The count comes from the loop index only on a break.
    if var_path.is_empty() {
        for (i, item) in src.0.iter().enumerate() {
            if on_item(item, test(Some(item))?).is_break() {
                return Some(i + 1);
            }
        }
    } else {
        for (i, item) in src.0.iter().enumerate() {
            let value = crate::arena::value::traverse_segments(item, var_path);
            if on_item(item, test(value)?).is_break() {
                return Some(i + 1);
            }
        }
    }
    Some(src.len())
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

/// `av === literal` as [`compare_equals`] answers it, or `None` (the
/// general path decides) when that tries the pair as datetimes first.
/// Numbers compare with `NumberValue`'s equality, as strict equality
/// compares them.
///
/// [`compare_equals`]: crate::operators::comparison::compare_equals
#[inline(always)]
fn strict_eq_literal(av: &DataValue<'_>, literal: &datavalue::OwnedDataValue) -> Option<bool> {
    use crate::operators::comparison::{ProbeSide, datetime_probe};
    if datetime_probe(ProbeSide::of(av), ProbeSide::of_owned(literal)) {
        return None;
    }
    Some(value_equals_serde(av, literal))
}

/// Two strings' equality, strict or loose (the same for two strings), or
/// `None` when the general path would compare them as datetimes or
/// durations first.
#[inline(always)]
fn str_eq(s: &str, literal: &str) -> Option<bool> {
    use crate::operators::comparison::{ProbeSide, datetime_probe};
    if datetime_probe(ProbeSide::Str(s), ProbeSide::Str(literal)) {
        return None;
    }
    Some(s == literal)
}

/// Whether `s` strictly equals one of `items`, or `None` when a pair needs
/// the datetime probe.
#[inline]
fn str_in(s: &str, items: &[Box<str>]) -> Option<bool> {
    for lit in items {
        if str_eq(s, lit)? {
            return Some(true);
        }
    }
    Some(false)
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
