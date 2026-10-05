//! Tests generated from the operator table: each iterates
//! [`OpCode::ALL`] or [`CATALOGUE`], so a new row is covered without
//! editing anything here.

use std::str::FromStr;

use datavalue::OwnedDataValue;

use super::eager::Arity;
use super::meta::{Extra, Miss};
use super::table::{CATALOGUE, builtin_operator_names};
use crate::node::{
    CompiledNode, MetadataHint, PathSegment, ReduceHint, SYNTHETIC_ID, ScopeBinding,
};
use crate::{Engine, ErrorKind, OpCode};

/// Every name parses to its opcode, and the opcode's canonical name parses
/// back to the same opcode.
#[test]
fn names_round_trip() {
    for &op in OpCode::ALL {
        for name in op.catalogue_entry().names {
            assert_eq!(OpCode::from_str(name), Ok(op), "{name:?}");
        }
        // An internal opcode has no names; it renders as the operator it
        // was compiled from (`VarDefault` as `var`).
        if !op.catalogue_entry().names.is_empty() {
            assert_eq!(OpCode::from_str(op.as_str()), Ok(op), "{op:?}");
        }
    }
}

/// `builtin_operator_names` is a public contract: no duplicates, and each
/// operator's canonical name comes before its aliases.
#[test]
fn names_are_unique_and_canonical_first() {
    let names: Vec<&str> = builtin_operator_names().collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted.len(), names.len(), "duplicate operator name");
    let mut seen = Vec::new();
    for name in &names {
        let op = OpCode::from_str(name).unwrap();
        if !seen.contains(&op) {
            seen.push(op);
            assert_eq!(op.as_str(), *name, "first name for {op:?} is not canonical");
        }
    }
}

/// The eager adapters find a row's metadata by indexing [`OpCode::ALL`]
/// with the opcode's discriminant, so the two orders must agree.
#[test]
fn all_is_indexed_by_discriminant() {
    for (i, &op) in OpCode::ALL.iter().enumerate() {
        assert_eq!(op as usize, i, "{op:?}");
    }
}

/// The catalogue lists exactly the compiled-in opcodes as enabled, and a
/// gated-off operator's names do not parse.
#[test]
fn catalogue_agrees_with_the_build() {
    let enabled: Vec<&str> = CATALOGUE
        .iter()
        .filter(|e| e.enabled)
        .map(|e| e.variant)
        .collect();
    let compiled: Vec<&str> = OpCode::ALL
        .iter()
        .map(|op| op.catalogue_entry().variant)
        .collect();
    assert_eq!(enabled, compiled);
    for entry in CATALOGUE.iter().filter(|e| !e.enabled) {
        for name in entry.names {
            assert!(OpCode::from_str(name).is_err(), "gated-off {name:?} parses");
        }
    }
    // An internal opcode has no name but must still render.
    for &op in OpCode::ALL {
        assert!(!op.as_str().is_empty(), "{op:?}");
    }
}

/// Only `iter` rows get an iteration-source classification, and every row
/// that runs a per-element body is one.
#[test]
fn iterators_are_iter_rows() {
    for &op in OpCode::ALL {
        if op.meta().is_iterator() {
            assert!(
                op.iterates_arg0(),
                "{op:?} runs a body but is not an `iter` row"
            );
        }
    }
}

/// Each row's operation, pinned. The fast paths, constant folding and the
/// `@ Kind(payload)` bodies all key on it, so a row that loses or changes
/// its algebra is a deliberate edit here too. Every row not listed has none.
#[test]
fn algebra_is_pinned() {
    use super::meta::{Algebra as A, ArithOp, DivOp, EqOp, Extremum, Logic, OrdOp, Quant, Truth};
    let expected = |op: OpCode| -> Option<A> {
        Some(match op {
            OpCode::Equals => A::Eq(EqOp::LOOSE),
            OpCode::StrictEquals => A::Eq(EqOp::STRICT),
            OpCode::NotEquals => A::Eq(EqOp::LOOSE_NE),
            OpCode::StrictNotEquals => A::Eq(EqOp::STRICT_NE),
            OpCode::GreaterThan => A::Ord(OrdOp::Gt),
            OpCode::GreaterThanEqual => A::Ord(OrdOp::Ge),
            OpCode::LessThan => A::Ord(OrdOp::Lt),
            OpCode::LessThanEqual => A::Ord(OrdOp::Le),
            OpCode::Not => A::Truth(Truth::Not),
            OpCode::BoolCast => A::Truth(Truth::Bool),
            OpCode::Concat => A::Concat,
            OpCode::And => A::Logic(Logic::And),
            OpCode::Or => A::Logic(Logic::Or),
            OpCode::Add => A::Arith(ArithOp::Add),
            OpCode::Subtract => A::Arith(ArithOp::Sub),
            OpCode::Multiply => A::Arith(ArithOp::Mul),
            OpCode::Divide => A::Div(DivOp::Divide),
            OpCode::Modulo => A::Div(DivOp::Modulo),
            OpCode::Max => A::Extremum(Extremum::Max),
            OpCode::Min => A::Extremum(Extremum::Min),
            OpCode::All => A::Quant(Quant::All),
            OpCode::Some => A::Quant(Quant::Some),
            OpCode::None => A::Quant(Quant::None),
            _ => return None,
        })
    };
    for &op in OpCode::ALL {
        assert_eq!(op.meta().algebra, expected(op), "{op:?}");
        // The by-value accessors the per-evaluation fast paths use agree
        // with the row.
        assert_eq!(op.algebra(), op.meta().algebra, "{op:?}");
        let arith = match op.meta().algebra {
            Some(A::Arith(a)) => Some(a),
            _ => None,
        };
        assert_eq!(op.arith_op(), arith, "{op:?}");
    }
}

/// The rows whose source the generated `each` adapter resolves, pinned.
/// `sort` (null and `[]` give different results), `reduce` (evaluates its
/// initial value first) and `min` / `max` (variadic form) resolve their own.
#[test]
fn each_rows_are_pinned() {
    let mut each: Vec<&str> = OpCode::ALL
        .iter()
        .filter(|op| op.catalogue_entry().shape == "each")
        .map(|op| op.as_str())
        .collect();
    each.sort_unstable();
    let mut want = vec!["all", "filter", "map", "none", "some"];
    if cfg!(feature = "ext-array") {
        want.extend(["distinct", "group_by"]);
    }
    want.sort_unstable();
    assert_eq!(each, want);
}

/// Every `each` row answers a null, missing or empty-array source with its
/// declared `on_empty_source`, and never runs its body to get there (the
/// body here throws).
#[test]
fn each_rows_short_circuit_empty_sources() {
    let engine = Engine::builder().with_constant_folding(false).build();
    let mut checked = 0;
    for &op in OpCode::ALL {
        if op.catalogue_entry().shape != "each" {
            continue;
        }
        let empty = op
            .meta()
            .on_empty_source
            .expect("an `each` row declares `on_empty_source`");
        let want = render(Ok(empty().to_owned()));
        for source in ["null", "[]", r#"{"var": "nope"}"#, r#"{"val": "nope"}"#] {
            let rule = format!(r#"{{"{}": [{source}, {{"throw": "ran"}}]}}"#, op.as_str());
            assert_eq!(outcome(&engine, &rule), want, "{rule}");
        }
        checked += 1;
    }
    assert!(checked >= 5, "only {checked} rows checked");
}

/// Every boolean-combinator row (`and`, `or`, `!`, `!!`, found by their
/// algebra) over fast-predicate leaves is itself cached as a fast predicate.
#[test]
fn combinator_predicates_take_the_fast_path() {
    use super::meta::Algebra;
    let engine = Engine::new();
    let leaf = r#"{">": [{"var": "a"}, 1]}"#;
    let mut checked = 0;
    for &op in OpCode::ALL {
        let args = match op.meta().algebra {
            Some(Algebra::Logic(_)) => format!("[{leaf}, {leaf}]"),
            Some(Algebra::Truth(_)) => format!("[{leaf}]"),
            _ => continue,
        };
        let rule = format!(
            r#"{{"filter": [{{"var": "xs"}}, {{"{}": {args}}}]}}"#,
            op.as_str()
        );
        let logic = engine.compile(rule.as_str()).unwrap();
        let CompiledNode::BuiltinOperator { args, .. } = &logic.root else {
            panic!("{rule} did not compile to an operator node");
        };
        let CompiledNode::BuiltinOperator { predicate_hint, .. } = &args[1] else {
            panic!("{rule}: predicate is not an operator node");
        };
        assert!(predicate_hint.is_some(), "{rule}: no fast predicate");
        checked += 1;
    }
    assert_eq!(checked, 4);
}

/// Every comparison row's `{op: [var, literal]}` predicate is cached as a
/// fast predicate, so `filter` / `all` / `some` / `none` over it never
/// dispatch per element. Keyed on the row's algebra, so a comparison row
/// that loses it (or a detection that stops matching) fails here rather
/// than as a benchmark regression.
#[test]
fn comparison_predicates_take_the_fast_path() {
    use super::meta::Algebra;
    let engine = Engine::new();
    let mut checked = 0;
    for &op in OpCode::ALL {
        if !matches!(op.meta().algebra, Some(Algebra::Ord(_) | Algebra::Eq(_))) {
            continue;
        }
        for literal in ["500", "\"a\""] {
            // Loose and ordered comparisons only specialise numeric (and
            // loose-equality string) literals.
            if literal == "\"a\"" && matches!(op.meta().algebra, Some(Algebra::Ord(_))) {
                continue;
            }
            let rule = format!(
                r#"{{"filter": [{{"var": "xs"}}, {{"{}": [{{"var": ""}}, {literal}]}}]}}"#,
                op.as_str()
            );
            let logic = engine.compile(rule.as_str()).unwrap();
            let CompiledNode::BuiltinOperator { args, .. } = &logic.root else {
                panic!("{rule} did not compile to an operator node");
            };
            let CompiledNode::BuiltinOperator { predicate_hint, .. } = &args[1] else {
                panic!("{rule}: predicate is not an operator node");
            };
            assert!(predicate_hint.is_some(), "{rule}: no fast predicate");
            checked += 1;
        }
    }
    assert_eq!(checked, 12);
}

fn lit() -> CompiledNode {
    CompiledNode::synthetic_value(OwnedDataValue::from(1i64))
}

fn dynamic() -> CompiledNode {
    CompiledNode::Var {
        id: SYNTHETIC_ID,
        scope_level: 0,
        segments: vec![PathSegment::Field("x".into())].into_boxed_slice(),
        reduce_hint: ReduceHint::None,
        metadata_hint: MetadataHint::None,
        default_value: None,
        binding: ScopeBinding::Unresolved,
    }
}

fn statics(n: usize) -> Vec<CompiledNode> {
    (0..n).map(|_| lit()).collect()
}

/// The classification decisions recorded when the six hand-written
/// classifiers were replaced by derivation (proposal §4.3). Pinned so a
/// change to a row's facts that flips one of them is deliberate.
#[test]
fn derived_classification_decisions() {
    // Every pure operator folds with static arguments and not with a
    // dynamic one.
    let add = OpCode::Add.meta();
    assert!(add.can_fold(&statics(2)));
    assert!(!add.can_fold(&[lit(), dynamic()]));

    // Context readers, effects and explicit opt-outs never fold.
    for op in [
        OpCode::Val,
        OpCode::VarDefault,
        OpCode::Missing,
        OpCode::MissingSome,
    ] {
        assert!(!op.meta().can_fold(&statics(1)), "{op:?}");
    }
    for op in [OpCode::Merge, OpCode::Min, OpCode::Max] {
        assert!(!op.meta().can_fold(&statics(2)), "{op:?}");
    }

    // An iterator with a body never folds; without one there is nothing
    // under a frame, so the call is a pure function of its arguments.
    for op in [OpCode::Map, OpCode::Filter, OpCode::Reduce, OpCode::All] {
        assert!(!op.meta().can_fold(&statics(2)), "{op:?}");
        assert!(op.meta().can_fold(&statics(1)), "{op:?}");
    }

    // Every iterator memoises as a whole; `min` / `max` are not iterators
    // (they push no frame) although they consume `args[0]` as one.
    assert!(OpCode::Map.meta().is_iterator());
    assert!(!OpCode::Max.meta().is_iterator());
    assert!(OpCode::Max.iterates_arg0());

    #[cfg(feature = "ext-array")]
    {
        // `sort` with a key expression runs it per element, so it no
        // longer folds; without one it does.
        assert!(!OpCode::Sort.meta().can_fold(&statics(3)));
        assert!(OpCode::Sort.meta().can_fold(&statics(2)));
        // `distinct` folds without a key expression.
        assert!(OpCode::Distinct.meta().can_fold(&statics(1)));
        assert!(!OpCode::Distinct.meta().can_fold(&statics(2)));
    }
    #[cfg(feature = "error-handling")]
    {
        // `try`'s catch frame is not an iterator; neither `try` nor
        // `throw` folds or memoises.
        assert!(!OpCode::Try.meta().is_iterator());
        assert_eq!(OpCode::Try.meta().frames_for(1, 2), 1);
        assert_eq!(OpCode::Try.meta().frames_for(0, 1), 0);
        for op in [OpCode::Try, OpCode::Throw] {
            assert!(!op.meta().can_fold(&statics(1)), "{op:?}");
            assert!(!op.meta().cse_pure(), "{op:?}");
        }
    }
    #[cfg(feature = "datetime")]
    {
        assert!(!OpCode::Now.meta().can_fold(&[]));
        assert!(!OpCode::Now.meta().cse_pure());
    }
    #[cfg(feature = "flagd")]
    {
        // `sem_ver` folds a literal call but is never memoised;
        // `fractional` does neither.
        assert!(OpCode::SemVer.meta().can_fold(&statics(3)));
        assert!(!OpCode::SemVer.meta().cse_pure());
        assert!(!OpCode::Fractional.meta().can_fold(&statics(2)));
        assert!(!OpCode::Fractional.meta().cse_pure());
    }
    #[cfg(feature = "tensor")]
    for &op in OpCode::ALL
        .iter()
        .filter(|op| op.catalogue_entry().family == "Tensor")
    {
        assert!(!op.meta().can_fold(&statics(2)), "{op:?}");
        assert!(!op.meta().cse_pure(), "{op:?}");
    }
}

/// Arity is derived from the `eager` signature.
#[test]
fn arity_is_derived_from_the_signature() {
    assert_eq!(OpCode::In.arity(), Arity::exactly(2));
    assert_eq!(OpCode::Substr.arity(), Arity::between(1, 3));
    #[cfg(feature = "datetime")]
    {
        assert_eq!(OpCode::Now.arity(), Arity::exactly(0));
        assert_eq!(OpCode::FormatDate.arity(), Arity::between(2, 3));
    }
    assert_eq!(OpCode::Not.arity(), Arity::between(0, 1));
    // A `Rest` tail leaves the maximum open.
    #[cfg(feature = "ext-math")]
    assert_eq!(OpCode::Abs.arity(), Arity::at_least(1));
    #[cfg(feature = "tensor")]
    assert_eq!(OpCode::TensorPad.arity(), Arity::between(3, 4));
}

/// The arity of every row that is not a plain fixed-arity `eager` row,
/// pinned, whatever shape implements it now: a row may move between shapes
/// (`raw[2..]` to `eager(Any, Any, Rest<Any>)`) without changing the counts
/// it publishes. `ANY` means the body takes any count (it may still branch
/// on it). Published in `Engine::operators()` and `operators.json`.
#[test]
fn declared_arity_is_pinned() {
    let expected = |op: OpCode| -> Option<Arity> {
        Some(match op {
            OpCode::Val | OpCode::VarDefault | OpCode::Add | OpCode::Multiply => Arity::ANY,
            OpCode::Concat | OpCode::Merge | OpCode::Missing => Arity::ANY,
            OpCode::Equals | OpCode::StrictEquals => Arity::at_least(2),
            OpCode::NotEquals | OpCode::StrictNotEquals => Arity::exactly(2),
            OpCode::GreaterThan
            | OpCode::GreaterThanEqual
            | OpCode::LessThan
            | OpCode::LessThanEqual => Arity::at_least(2),
            OpCode::And | OpCode::Or | OpCode::If => Arity::at_least(1),
            OpCode::Subtract | OpCode::Divide | OpCode::Modulo => Arity::at_least(1),
            OpCode::Max | OpCode::Min => Arity::at_least(1),
            OpCode::Filter | OpCode::Map | OpCode::All | OpCode::Some | OpCode::None => {
                Arity::exactly(2)
            }
            OpCode::Reduce => Arity::between(2, 3),
            OpCode::MissingSome => Arity::exactly(2),
            #[cfg(feature = "ext-array")]
            OpCode::Sort => Arity::between(1, 3),
            #[cfg(feature = "ext-array")]
            OpCode::Slice => Arity::between(1, 4),
            #[cfg(feature = "ext-array")]
            OpCode::GroupBy => Arity::exactly(2),
            #[cfg(feature = "ext-array")]
            OpCode::Distinct => Arity::between(1, 2),
            #[cfg(feature = "ext-control")]
            OpCode::Exists => Arity::at_least(1),
            #[cfg(feature = "ext-control")]
            OpCode::Coalesce => Arity::ANY,
            #[cfg(feature = "ext-control")]
            OpCode::Switch => Arity::between(2, 3),
            #[cfg(feature = "error-handling")]
            OpCode::Try => Arity::at_least(1),
            #[cfg(feature = "error-handling")]
            OpCode::Throw => Arity::ANY,
            #[cfg(feature = "flagd")]
            OpCode::Fractional => Arity::at_least(1),
            _ => return None,
        })
    };
    for &op in OpCode::ALL {
        match expected(op) {
            Some(arity) => assert_eq!(op.arity(), arity, "{op:?}"),
            // Every other row is a fixed-arity `eager` row, whose arity the
            // signature test covers.
            None => assert!(
                op.catalogue_entry().shape.starts_with("eager"),
                "{op:?} needs a pinned arity"
            ),
        }
    }
}

/// The rows that stay `raw`, pinned. Most inspect their argument nodes
/// before (or instead of) evaluating them, so no extractor signature fits:
/// `val` reads its path segments, `if` / `switch` / `try` evaluate only the
/// arms they reach, `throw` builds its payload from a literal, the
/// arithmetic operators reject a literal array argument, and `slice`
/// returns for a null collection before evaluating its other arguments.
/// The comparisons, `and` / `or` and `fractional` would fit an `eager`
/// signature but measured slower as one (phase 2, P6); moving them needs a
/// new benchmark result, not just a passing suite. Everything else is
/// `eager`, `each` or `iter`.
#[test]
fn raw_rows_are_pinned() {
    let mut raw: Vec<&str> = OpCode::ALL
        .iter()
        .filter(|op| op.catalogue_entry().shape == "raw")
        .map(|op| op.catalogue_entry().variant)
        .collect();
    raw.sort_unstable();
    let mut want = vec![
        "Add",
        "And",
        "Divide",
        "Equals",
        "GreaterThan",
        "GreaterThanEqual",
        "If",
        "LessThan",
        "LessThanEqual",
        "Modulo",
        "Multiply",
        "NotEquals",
        "Or",
        "StrictEquals",
        "StrictNotEquals",
        "Subtract",
        "Val",
        "VarDefault",
    ];
    if cfg!(feature = "ext-array") {
        want.push("Slice");
    }
    if cfg!(feature = "ext-control") {
        want.push("Switch");
    }
    if cfg!(feature = "error-handling") {
        want.extend(["Throw", "Try"]);
    }
    if cfg!(feature = "flagd") {
        want.push("Fractional");
    }
    want.sort_unstable();
    assert_eq!(raw, want);
}

/// `{name: [null, null, ...]}` with `n` arguments.
fn call_with(name: &str, n: usize) -> String {
    let args = vec!["null"; n].join(", ");
    format!("{{\"{name}\": [{args}]}}")
}

/// The evaluation outcome, rendered for comparison (`ErrorKind` has no
/// `PartialEq`).
fn outcome(engine: &Engine, rule: &str) -> String {
    render(engine.eval(rule, "{}").map_err(|e| e.kind))
}

fn render(r: Result<OwnedDataValue, ErrorKind>) -> String {
    format!("{r:?}")
}

/// Every row with a declared arity honours its policy: one argument short
/// of the minimum produces exactly its `on_missing` result, one past the
/// maximum exactly its `on_extra` result. Pins the messages the suites do
/// not, since an `InvalidArguments` message is the serialised error `type`.
#[test]
fn rows_honour_their_arity_policy() {
    let engine = Engine::builder().with_constant_folding(false).build();
    let mut checked = 0;
    for &op in OpCode::ALL {
        let arity = op.arity();
        // Nothing to fall short of or exceed.
        if arity == Arity::ANY {
            continue;
        }
        let meta = op.meta();
        let name = op.as_str();

        if arity.min > 0 {
            let got = outcome(&engine, &call_with(name, arity.min as usize - 1));
            let want = render(match meta.on_missing {
                Miss::InvalidArgs => Err(ErrorKind::InvalidArguments("Invalid Arguments".into())),
                Miss::Err(msg) => Err(ErrorKind::InvalidArguments(msg.into())),
                Miss::Return(value) => Ok(value().to_owned()),
            });
            assert_eq!(got, want, "{name} with {} argument(s)", arity.min - 1);
        }

        // A variadic tail (`Rest`) has no maximum to exceed.
        let Some(max) = arity.max else {
            checked += 1;
            continue;
        };
        let extra = outcome(&engine, &call_with(name, max as usize + 1));
        match meta.on_extra {
            // Extras are never evaluated: the call behaves as if they were
            // not there.
            Extra::Ignore => {
                let exact = outcome(&engine, &call_with(name, max as usize));
                assert_eq!(extra, exact, "{name} ignores extra arguments");
            }
            Extra::InvalidArgs => assert_eq!(
                extra,
                render(Err(ErrorKind::InvalidArguments("Invalid Arguments".into()))),
                "{name}"
            ),
            Extra::Err(msg) => assert_eq!(
                extra,
                render(Err(ErrorKind::InvalidArguments(msg.into()))),
                "{name}"
            ),
            Extra::Return(value) => assert_eq!(extra, render(Ok(value().to_owned())), "{name}"),
        }
        checked += 1;
    }
    // The core rows alone declare more than this.
    assert!(checked >= 25, "only {checked} rows checked");
}

/// Every row that declares a scoped argument is exercised with context
/// reads at that position: the current frame (`{"var": ""}`) and one level
/// up (`{"val": [[1]]}`). In debug builds the scope oracle
/// (`operators::variable::debug_check_binding`) asserts on every read that
/// the compile-time frame count matches the frames the operator actually
/// pushed, so a wrong `frames` declaration fails here.
///
/// The call sits inside an outer `map` so every level has a distinct frame:
/// at top level a miscounted frame clamps to the root and is unobservable.
#[test]
fn scoped_arguments_resolve_against_the_pushed_frame() {
    use super::meta::Frames;
    let body = r#"{"cat": [{"var": ""}, {"val": [[1]]}]}"#;
    for engine in [
        Engine::new(),
        Engine::builder().with_constant_folding(false).build(),
    ] {
        for &op in OpCode::ALL {
            let args = match op.meta().frames {
                Frames::None => continue,
                Frames::At(i) => {
                    let mut args = vec!["[1, 2]"];
                    args.extend(std::iter::repeat_n("null", i as usize - 1));
                    args.push(body);
                    args
                }
                Frames::LastIfMulti => vec![r#"{"throw": "boom"}"#, body],
            };
            let call = format!("{{\"{}\": [{}]}}", op.as_str(), args.join(", "));
            let rule = format!("{{\"map\": [{{\"var\": \"rows\"}}, {call}]}}");
            let result = engine.eval(rule.as_str(), r#"{"rows": [{"outer": 1}]}"#);
            assert!(result.is_ok(), "{rule}: {result:?}");
        }
    }
}

/// Every row the fold rule lets fold is folded by at least one suite case,
/// and on every case folding exercises, the folded and unfolded rules
/// agree. An operator wrongly declared pure (an unrecorded effect, a
/// hidden context read) gives a different answer once folded; this makes
/// sure each foldable row has a case where that would show.
#[cfg(all(feature = "serde_json", feature = "templating"))]
#[test]
fn every_foldable_row_is_folded_by_a_suite_case() {
    use serde_json::Value;
    use std::collections::BTreeSet;

    fn foldable_calls(node: &CompiledNode, out: &mut BTreeSet<&'static str>) {
        if let CompiledNode::BuiltinOperator { opcode, args, .. } = node
            && opcode.meta().can_fold(args)
        {
            out.insert(opcode.as_str());
        }
        node.visit_indexed_children(&mut |_, child| foldable_calls(child, out));
    }

    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/suites");
    let index = std::fs::read_to_string(format!("{root}/index.json")).expect("read index.json");
    let files: Vec<String> = serde_json::from_str(&index).expect("parse index.json");

    let mut folded = BTreeSet::new();
    let mut compared = 0;
    for file in files {
        let text = std::fs::read_to_string(format!("{root}/{file}")).expect("read suite");
        let cases: Value = serde_json::from_str(&text).expect("parse suite");
        for case in cases.as_array().into_iter().flatten() {
            let Some(rule) = case.get("rule") else {
                continue;
            };
            if case.get("template_key_escape").is_some() {
                continue;
            }
            let templating = case.get("templating").and_then(Value::as_bool) == Some(true);
            let build = |folding: bool| {
                Engine::builder()
                    .with_templating(templating)
                    .with_constant_folding(folding)
                    .build()
            };
            let (folding, plain) = (build(true), build(false));
            let Ok(unfolded) = plain.compile(rule) else {
                continue;
            };
            let mut here = BTreeSet::new();
            foldable_calls(&unfolded.root, &mut here);
            if here.is_empty() {
                continue;
            }
            let data = case.get("data").cloned().unwrap_or(Value::Null);
            let run = |engine: &Engine| {
                format!(
                    "{:?}",
                    engine
                        .eval_into::<Value, _, _>(rule, &data)
                        .map_err(|e| e.kind)
                )
            };
            assert_eq!(
                run(&folding),
                run(&plain),
                "{file}: folding changes the result of {rule}"
            );
            compared += 1;
            folded.extend(here);
        }
    }
    assert!(compared > 0);

    let never_folded: Vec<&str> = OpCode::ALL
        .iter()
        .filter(|op| !op.catalogue_entry().names.is_empty())
        .filter(|op| {
            let meta = op.meta();
            // Could fold with all-literal arguments.
            meta.can_fold(&[]) && !folded.contains(op.as_str())
        })
        .map(|op| op.as_str())
        .collect();
    assert!(
        never_folded.is_empty(),
        "foldable operators no suite case folds (add a case with literal arguments): {never_folded:?}"
    );
}
