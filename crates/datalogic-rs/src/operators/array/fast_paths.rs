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
}
