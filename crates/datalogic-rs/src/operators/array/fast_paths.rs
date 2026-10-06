//! Every iterator fast path, in one list.
//!
//! A fast path is a loop specialised to one shape of iterator body: it
//! reads fields straight from each element instead of pushing a frame and
//! dispatching the body per item. Each one must compute exactly what the
//! general path computes for that shape, and each falls back to the
//! general path when it cannot (a value of the wrong type, say).
//!
//! | Fast path | Operator | Fires on | Decided | Gives up when |
//! |---|---|---|---|---|
//! | `FastPredicate` | `filter`, `all`, `some`, `none` | a comparison of an element field (or the element) with a literal; `and` / `or` / `!` / `!!` trees of those; a bare field; `in` against a string list | compile time (`predicate_hint`) | loose `==` meets a non-string |
//! | strict-eq field | `filter` | `===` / `!==` of an element field and a loop-invariant value | per call ([`super::filter::strict_eq_field_shape`]) | never |
//! | map pluck | `map` | a body that is an element field | per call ([`FusedMapBody::detect`]) | never |
//! | map arithmetic | `map` | `+` / `-` / `*` of a field and a literal, or of two fields | per call ([`FusedMapBody::detect`]) | an operand is not a number |
//! | reduce fold | `reduce` | `+` / `-` / `*` of `current` (or `current.path`) and `accumulator` | per call ([`super::reduce::detect_fold_shape`]) | an operand is not a number |
//! | reduce over map | `reduce` | a `reduce` fold over a `map` with a pluck or arithmetic body | per call (`try_fused_reduce_map`) | an operand is not a number |
//! | sort by field | `sort` | a key that is an element field | per call ([`super::sort::sort_key_field`]) | never |
//!
//! Every fast path stands aside when evaluation is traced (so each item
//! shows its steps) and under
//! [`MissingVar::Error`](crate::MissingVar::Error) (where a missing field
//! must raise, which an inline read does not).
//!
//! **Metering.** A fast path charges the operation budget what the general
//! path would have charged for the same data: one per node the body would
//! have dispatched (short-circuiting included), plus any charge the
//! operators in it take themselves (an `in` haystack's length, a
//! structural `===` walk). So the count does not depend on which path
//! ran, on the data's types, on tracing or on `MissingVar`. A path that
//! can give up partway charges only once it has finished, and charges
//! nothing when it gives up; the general path that runs instead charges
//! for itself. The one residue is CSE: a fused `reduce(map(..))` never
//! builds the memoised map's array, so a later occurrence of that map
//! evaluates (and is charged) again instead of reading the memo.
//! `tests::fast_and_general_paths_charge_the_same` pins the parity.
//!
//! **Keeping them honest.** `tests/oracle_test.rs` generates rules in each
//! of these shapes (`arb_expr`'s "the fast paths' shapes" productions) over
//! data with missing, null and mistyped fields, and checks the engine
//! against the reference interpreter, which has no fast paths. The tests
//! below pin that each fast path fires on the shapes listed, so the
//! differential test is known to reach it. A new fast path belongs in this
//! table, in the generator, and in these tests.
//!
//! In 6.0 the shapes become plan specialisations chosen once at compile
//! time; in 5.x an operator body sees only its arguments, so the per-call
//! detectors stay where the body can reach them.

use crate::Engine;
use crate::arena::ContextStack;

/// Whether an iterator may take a fast path this call: not traced and not
/// under [`MissingVar::Error`](crate::MissingVar::Error), as above.
#[inline(always)]
pub(super) fn allowed(ctx: &ContextStack<'_>, engine: &Engine) -> bool {
    !ctx.is_tracing() && engine.reads_fields_inline()
}

#[cfg(all(test, feature = "serde_json"))]
use super::fused::FusedMapBody;

#[cfg(all(test, feature = "serde_json"))]
mod tests {
    use super::super::filter::strict_eq_field_shape;
    use super::super::reduce::detect_fold_shape;
    use super::FusedMapBody;
    use crate::{CompiledNode, Engine, Logic};

    fn compile(rule: &str) -> Logic {
        Engine::new().compile(rule).unwrap()
    }

    /// Argument `i` of the compiled root, an operator call.
    fn arg(logic: &Logic, i: usize) -> &CompiledNode {
        let CompiledNode::BuiltinOperator { args, .. } = &logic.root else {
            panic!("root is not a call: {:?}", logic.root)
        };
        &args[i]
    }

    fn predicate_hinted(logic: &Logic) -> bool {
        matches!(
            arg(logic, 1),
            CompiledNode::BuiltinOperator {
                predicate_hint: Some(_),
                ..
            }
        )
    }

    #[test]
    fn fast_predicate_fires() {
        for rule in [
            r#"{"filter": [{"var": "rows"}, {">": [{"var": "k"}, 1]}]}"#,
            r#"{"filter": [{"var": "rows"}, {"<=": [2, {"var": "k"}]}]}"#,
            r#"{"some": [{"var": "rows"}, {"==": [{"var": "k"}, "a"]}]}"#,
            r#"{"all": [{"var": "xs"}, {">=": [{"var": ""}, 0]}]}"#,
            r#"{"none": [{"var": "rows"}, {"and": [{"var": "k"}, {"!": {"var": "n"}}]}]}"#,
            r#"{"filter": [{"var": "rows"}, {"in": [{"var": "k"}, ["a", "b"]]}]}"#,
        ] {
            assert!(predicate_hinted(&compile(rule)), "{rule}");
        }
    }

    #[test]
    fn strict_eq_field_fires() {
        for rule in [
            r#"{"filter": [{"var": "rows"}, {"===": [{"var": "k"}, 1]}]}"#,
            r#"{"filter": [{"var": "rows"}, {"!==": [null, {"var": "k"}]}]}"#,
            r#"{"filter": [{"var": "rows"}, {"===": [{"var": "k"}, {"val": [[1], "target"]}]}]}"#,
        ] {
            assert!(
                strict_eq_field_shape(arg(&compile(rule), 1)).is_some(),
                "{rule}"
            );
        }
        // The element itself is not a field: the predicate path covers it.
        let whole = compile(r#"{"filter": [{"var": "xs"}, {"===": [{"var": ""}, 1]}]}"#);
        assert!(strict_eq_field_shape(arg(&whole, 1)).is_none());
    }

    #[test]
    fn map_fusions_fire() {
        for rule in [
            r#"{"map": [{"var": "rows"}, {"var": "k"}]}"#,
            r#"{"map": [{"var": "xs"}, {"var": ""}]}"#,
            r#"{"map": [{"var": "rows"}, {"+": [{"var": "k"}, 1]}]}"#,
            r#"{"map": [{"var": "rows"}, {"-": [10, {"var": "k"}]}]}"#,
            r#"{"map": [{"var": "rows"}, {"*": [{"var": "k"}, {"var": "n"}]}]}"#,
        ] {
            assert!(
                FusedMapBody::detect(arg(&compile(rule), 1)).is_some(),
                "{rule}"
            );
        }
        // A body that reads the index is not a fusion.
        let indexed = compile(r#"{"map": [{"var": "xs"}, {"val": [[1], "index"]}]}"#);
        assert!(FusedMapBody::detect(arg(&indexed, 1)).is_none());
    }

    #[test]
    fn reduce_folds_fire() {
        for rule in [
            r#"{"reduce": [{"var": "xs"}, {"+": [{"var": "current"}, {"var": "accumulator"}]}, 0]}"#,
            r#"{"reduce": [{"var": "xs"}, {"-": [{"var": "accumulator"}, {"var": "current"}]}, 0]}"#,
            r#"{"reduce": [{"var": "rows"}, {"*": [{"var": "current.k"}, {"var": "accumulator"}]}, 1]}"#,
        ] {
            assert!(
                detect_fold_shape(arg(&compile(rule), 1)).is_some(),
                "{rule}"
            );
        }
        // reduce over map: the fold reads bare `current` and the map body
        // fuses.
        let fused = compile(
            r#"{"reduce": [{"map": [{"var": "rows"}, {"var": "k"}]}, {"+": [{"var": "accumulator"}, {"var": "current"}]}, 0]}"#,
        );
        let CompiledNode::BuiltinOperator { args: map_args, .. } = arg(&fused, 0) else {
            panic!("source is not a map")
        };
        assert!(FusedMapBody::detect(&map_args[1]).is_some());
        assert!(detect_fold_shape(arg(&fused, 1)).is_some());
    }

    #[cfg(feature = "ext-array")]
    #[test]
    fn sort_by_field_fires() {
        use super::super::sort::sort_key_field;
        let rule = compile(r#"{"sort": [{"var": "rows"}, true, {"var": "k"}]}"#);
        assert!(sort_key_field(arg(&rule, 2)).is_some());
        let computed = compile(r#"{"sort": [{"var": "rows"}, true, {"length": {"var": "k"}}]}"#);
        assert!(sort_key_field(arg(&computed, 2)).is_none());
    }

    /// Every fast path charges what the general path does for the same
    /// data. Under `MissingVar::Error` no fast path runs (see `allowed`),
    /// so with data whose fields are all present the two engines must
    /// report the same count, fast path taken, abandoned partway, or not.
    #[cfg(feature = "budget")]
    #[test]
    fn fast_and_general_paths_charge_the_same() {
        use crate::{EvaluationConfig, MissingVar};
        let fast = Engine::new();
        let general = Engine::builder()
            .with_config(EvaluationConfig::default().with_missing_var(MissingVar::Error))
            .build();
        let ops = |engine: &Engine, rule: &str, data: &str| -> u64 {
            let compiled = engine.compile(rule).unwrap();
            let arena = bumpalo::Bump::new();
            engine
                .evaluate_metered(&compiled, data, &arena, u64::MAX)
                .unwrap_or_else(|e| panic!("{rule} over {data}: {e}"))
                .ops
        };

        let nums = r#"{"xs": [1, 5, 2, 8, 3], "t": 2}"#;
        let mixed = r#"{"xs": [1, 5, "2", 8, 3], "t": 2}"#;
        let rows = r#"{"rows": [{"k": 1, "n": 2, "s": "a"}, {"k": 4, "n": 3, "s": "b"}, {"k": 2, "n": 5, "s": "c"}], "t": 4}"#;
        let mixed_rows = r#"{"rows": [{"k": 1, "n": 2, "s": "a"}, {"k": "4", "n": 3, "s": 7}, {"k": 2, "n": 5, "s": "c"}], "t": 4}"#;
        let obj = r#"{"rows": {"a": {"k": 1}, "b": {"k": 2}}}"#;
        let empty = r#"{"rows": [], "xs": []}"#;
        let cases: &[&str] = &[
            // FastPredicate leaves, in filter and the quantifiers.
            r#"{"filter": [{"var": "xs"}, {">": [{"var": ""}, 2]}]}"#,
            r#"{"filter": [{"var": "xs"}, {"<=": [3, {"var": ""}]}]}"#,
            r#"{"filter": [{"var": "xs"}, {"==": [{"var": ""}, 5]}]}"#,
            r#"{"filter": [{"var": "xs"}, {"===": [{"var": ""}, 5]}]}"#,
            r#"{"filter": [{"var": "xs"}, {"!==": [{"var": ""}, [1]]}]}"#,
            r#"{"filter": [{"var": "rows"}, {"==": [{"var": "s"}, "b"]}]}"#,
            r#"{"filter": [{"var": "rows"}, {"in": [{"var": "s"}, ["a", "c", "z"]]}]}"#,
            r#"{"some": [{"var": "xs"}, {">": [{"var": ""}, 4]}]}"#,
            r#"{"all": [{"var": "xs"}, {"<": [{"var": ""}, 4]}]}"#,
            r#"{"none": [{"var": "rows"}, {"===": [{"var": "k"}, 2]}]}"#,
            // Combinators, short-circuiting.
            r#"{"filter": [{"var": "rows"}, {"and": [{">": [{"var": "k"}, 1]}, {"var": "n"}, {"!": {"var": "s"}}]}]}"#,
            r#"{"filter": [{"var": "rows"}, {"or": [{"==": [{"var": "k"}, 1]}, {"!!": {"var": "n"}}]}]}"#,
            r#"{"some": [{"var": "rows"}, {"and": [{"in": [{"var": "s"}, ["b"]]}, {">=": [{"var": "n"}, 3]}]}]}"#,
            r#"{"filter": [{"var": "rows"}, {"!!": [{"==": [{"var": "s"}, "c"]}]}]}"#,
            // The strict-equality field path, literal and root operand.
            r#"{"filter": [{"var": "rows"}, {"===": [{"var": "k"}, 4]}]}"#,
            r#"{"filter": [{"var": "rows"}, {"!==": [{"val": [[1], "t"]}, {"var": "k"}]}]}"#,
            // map fusions.
            r#"{"map": [{"var": "rows"}, {"var": "k"}]}"#,
            r#"{"map": [{"var": "xs"}, {"var": ""}]}"#,
            r#"{"map": [{"var": "rows"}, {"*": [{"var": "k"}, 2]}]}"#,
            r#"{"map": [{"var": "rows"}, {"-": [10, {"var": "k"}]}]}"#,
            r#"{"map": [{"var": "rows"}, {"+": [{"var": "k"}, {"var": "n"}]}]}"#,
            // reduce folds, and reduce over map.
            r#"{"reduce": [{"var": "xs"}, {"+": [{"var": "current"}, {"var": "accumulator"}]}, 0]}"#,
            r#"{"reduce": [{"var": "rows"}, {"*": [{"var": "accumulator"}, {"var": "current.n"}]}, 1]}"#,
            r#"{"reduce": [{"map": [{"var": "rows"}, {"var": "k"}]}, {"+": [{"var": "accumulator"}, {"var": "current"}]}, 0]}"#,
            r#"{"reduce": [{"map": [{"var": "rows"}, {"*": [{"var": "k"}, {"var": "n"}]}]}, {"+": [{"var": "accumulator"}, {"var": "current"}]}, 0]}"#,
            r#"{"reduce": [{"map": [{"var": "rows"}, {"+": [{"var": "k"}, 1]}]}, {"-": [{"var": "current"}, {"var": "accumulator"}]}, 0]}"#,
            r#"{"reduce": [{"map": [{"var": "rows"}, {"var": "k"}]}, {"+": [{"var": "accumulator"}, {"var": "current"}]}, "0"]}"#,
            r#"{"reduce": [{"map": [{"filter": [{"var": "rows"}, {">": [{"var": "k"}, 1]}]}, {"var": "k"}]}, {"+": [{"var": "accumulator"}, {"var": "current"}]}, 0]}"#,
        ];
        let mut wrong = Vec::new();
        let mut compared = 0;
        for &rule in cases {
            for data in [nums, mixed, rows, mixed_rows, obj, empty] {
                // A case's data must hold every field it reads, or the
                // general engine raises; skip the pairings that don't.
                let Ok(expected) = general.compile(rule).map_err(|_| ()).and_then(|c| {
                    let arena = bumpalo::Bump::new();
                    general
                        .evaluate_metered(&c, data, &arena, u64::MAX)
                        .map(|m| m.ops)
                        .map_err(|_| ())
                }) else {
                    continue;
                };
                compared += 1;
                let got = ops(&fast, rule, data);
                if got != expected {
                    wrong.push(format!(
                        "{rule} over {data}: fast {got}, general {expected}"
                    ));
                }
            }
        }
        #[cfg(feature = "ext-array")]
        for (rule, data) in [
            (r#"{"sort": [{"var": "rows"}, true, {"var": "k"}]}"#, rows),
            (
                r#"{"sort": [{"var": "rows"}, false, {"var": "s"}]}"#,
                mixed_rows,
            ),
        ] {
            let (got, expected) = (ops(&fast, rule, data), ops(&general, rule, data));
            if got != expected {
                wrong.push(format!(
                    "{rule} over {data}: fast {got}, general {expected}"
                ));
            }
        }
        assert!(compared >= 50, "only {compared} pairings ran");
        assert!(wrong.is_empty(), "\n{}", wrong.join("\n"));
    }
}
