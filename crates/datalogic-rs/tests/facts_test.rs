//! `Logic::facts()`: what a compiled rule reads, which operators it uses,
//! and whether it is deterministic.
//!
//! Two halves. The hand-written cases pin the classification of every node
//! shape that touches the data context (precision: a read that is not a
//! root read must not be reported as one). The generated check runs every
//! suite case through `facts()` and proves the reported reads are
//! sufficient (soundness): evaluating against data pruned down to the
//! reported paths gives the same outcome as evaluating against the full
//! data.

use datalogic_rs::{DataPath, Engine, Facts};

fn facts(engine: &Engine, rule: &str) -> Facts {
    engine
        .compile(rule)
        .unwrap_or_else(|e| panic!("compile {rule}: {e}"))
        .facts()
}

/// The reported reads as segment lists, in the order `facts()` returns them.
fn segs(facts: &Facts) -> Vec<Vec<&str>> {
    facts
        .reads()
        .iter()
        .map(|p| p.segments().iter().map(String::as_str).collect())
        .collect()
}

/// The reported reads, dotted. Only for rules whose segments hold no dots.
fn dotted(facts: &Facts) -> Vec<String> {
    facts.reads().iter().map(DataPath::to_string).collect()
}

fn no_fold() -> Engine {
    Engine::builder().with_constant_folding(false).build()
}

// ── paths ───────────────────────────────────────────────────────────────

#[test]
fn var_path_splits_on_dots() {
    let f = facts(&Engine::new(), r#"{"var": "user.address.city"}"#);
    assert_eq!(segs(&f), [vec!["user", "address", "city"]]);
    assert_eq!(dotted(&f), ["user.address.city"]);
    assert!(!f.has_computed_reads());
    assert!(f.reads_complete());
    assert!(f.reads_data());
}

/// `val` takes one segment per argument and never splits on dots, so a key
/// that contains a dot stays one segment.
#[test]
fn val_segments_are_not_split() {
    let e = Engine::new();
    assert_eq!(segs(&facts(&e, r#"{"val": "a.b"}"#)), [vec!["a.b"]]);
    assert_eq!(segs(&facts(&e, r#"{"val": ["a.b"]}"#)), [vec!["a.b"]]);
    assert_eq!(segs(&facts(&e, r#"{"val": ["a", "b"]}"#)), [vec!["a", "b"]]);
    assert_eq!(segs(&facts(&e, r#"{"val": ["xs", 0]}"#)), [vec!["xs", "0"]]);
}

#[test]
fn numeric_var_path() {
    let e = Engine::new();
    assert_eq!(segs(&facts(&e, r#"{"var": 1}"#)), [vec!["1"]]);
    assert_eq!(segs(&facts(&e, r#"{"var": "xs.2"}"#)), [vec!["xs", "2"]]);
}

/// An empty path is the whole data context: one read with no segments,
/// which covers every other path.
#[test]
fn whole_context_read() {
    let e = Engine::new();
    for rule in [
        r#"{"var": ""}"#,
        r#"{"var": []}"#,
        r#"{"val": []}"#,
        r#"{"and": [{"var": ""}, {"var": "a.b"}]}"#,
    ] {
        let f = facts(&e, rule);
        assert_eq!(segs(&f), [Vec::<&str>::new()], "{rule}");
        assert!(f.reads()[0].is_root(), "{rule}");
        assert_eq!(dotted(&f), [""], "{rule}");
    }
}

/// A read that a shorter read already covers is dropped: observing `user`
/// observes everything under it.
#[test]
fn covered_reads_are_dropped() {
    let f = facts(
        &Engine::new(),
        r#"{"and": [{"var": "user.name"}, {"var": "user"}, {"var": "userx"}]}"#,
    );
    assert_eq!(dotted(&f), ["user", "userx"]);
}

#[test]
fn reads_are_sorted_and_deduplicated() {
    let f = facts(
        &Engine::new(),
        r#"{"+": [{"var": "b"}, {"var": "a"}, {"var": "a"}, {"var": "c.d"}]}"#,
    );
    assert_eq!(dotted(&f), ["a", "b", "c.d"]);
}

#[test]
fn var_default_is_read_too() {
    let f = facts(&Engine::new(), r#"{"var": ["x", {"var": "fallback"}]}"#);
    assert_eq!(dotted(&f), ["fallback", "x"]);
}

// ── computed paths ──────────────────────────────────────────────────────

#[test]
fn computed_var_path() {
    let e = Engine::new();
    for rule in [
        r#"{"var": {"var": "key"}}"#,
        r#"{"var": [{"var": "key"}, 0]}"#,
        r#"{"val": [{"var": "key"}]}"#,
        r#"{"val": ["a", {"var": "key"}]}"#,
        r#"{"val": [[1], {"var": "key"}]}"#,
    ] {
        let f = facts(&e, rule);
        assert!(f.has_computed_reads(), "{rule}");
        assert!(!f.reads_complete(), "{rule}");
        assert!(f.reads_data(), "{rule}");
        // The expression that computes the path is itself a read.
        assert!(
            dotted(&f).contains(&"key".to_string()),
            "{rule}: {:?}",
            dotted(&f)
        );
    }
}

/// A computed path inside an iterator body can still name a level that
/// climbs to the root, so it is computed wherever it appears.
#[test]
fn computed_path_in_iterator_body() {
    let f = facts(
        &Engine::new(),
        r#"{"map": [{"var": "xs"}, {"val": {"var": "k"}}]}"#,
    );
    assert!(f.has_computed_reads());
    assert_eq!(dotted(&f), ["xs"]);
}

/// Folding resolves a path computed from literals, so it is a plain read on
/// the default path and a computed one without folding.
#[test]
fn folded_path_is_static() {
    let rule = r#"{"var": {"cat": ["a", ".", "b"]}}"#;
    let f = facts(&Engine::new(), rule);
    assert_eq!(dotted(&f), ["a.b"]);
    assert!(!f.has_computed_reads());

    let f = facts(&no_fold(), rule);
    assert!(f.has_computed_reads());
    assert!(f.reads().is_empty());
}

// ── scoped arguments ────────────────────────────────────────────────────

/// An iterator body reads the current element, not the root. The source
/// read covers whatever the body reads from the element.
#[test]
fn iterator_body_reads_are_not_root_reads() {
    let e = Engine::new();
    for rule in [
        r#"{"map": [{"var": "items"}, {"var": "price"}]}"#,
        r#"{"filter": [{"var": "items"}, {">": [{"var": "qty"}, 0]}]}"#,
        r#"{"all": [{"var": "items"}, {"var": ""}]}"#,
        r#"{"some": [{"var": "items"}, {"missing": ["sku"]}]}"#,
        r#"{"none": [{"var": "items"}, {"val": "flag"}]}"#,
    ] {
        let f = facts(&e, rule);
        assert_eq!(dotted(&f), ["items"], "{rule}");
        assert!(f.reads_complete(), "{rule}");
    }
}

#[test]
fn reduce_reads_source_and_initial_value() {
    let f = facts(
        &Engine::new(),
        r#"{"reduce": [{"var": "xs"}, {"+": [{"var": "current"}, {"var": "accumulator"}]}, {"var": "start"}]}"#,
    );
    assert_eq!(dotted(&f), ["start", "xs"]);
}

/// Outside `reduce`, `current` and `accumulator` are ordinary field names.
#[test]
fn current_at_root_is_a_field() {
    let f = facts(
        &Engine::new(),
        r#"{"+": [{"var": "current"}, {"var": "accumulator.total"}]}"#,
    );
    assert_eq!(dotted(&f), ["accumulator.total", "current"]);
}

/// Inside one iterator, level 1 climbs to the root.
#[test]
fn level_marker_climbing_to_root() {
    let e = Engine::new();
    let rule = r#"{"map": [{"var": "xs"}, {"*": [{"var": ""}, {"val": [[1], "rate"]}]}]}"#;
    let f = facts(&e, rule);
    assert_eq!(dotted(&f), ["rate", "xs"]);
    assert_eq!(
        e.eval_str(rule, r#"{"xs": [1, 2], "rate": 10}"#).unwrap(),
        "[10,20]"
    );
}

/// Two iterators deep, level 2 names the outer element (not a root read)
/// and level 3 climbs to the root.
#[test]
fn level_markers_in_nested_iterators() {
    let e = Engine::new();
    let outer_element =
        r#"{"map": [{"var": "rows"}, {"map": [{"var": "cells"}, {"val": [[2], "id"]}]}]}"#;
    assert_eq!(dotted(&facts(&e, outer_element)), ["rows"]);

    let root = r#"{"map": [{"var": "rows"}, {"map": [{"var": "cells"}, {"val": [[3], "id"]}]}]}"#;
    assert_eq!(dotted(&facts(&e, root)), ["id", "rows"]);
}

/// `index` / `key` at a metadata level read iteration metadata, not data.
#[test]
fn metadata_reads_are_not_data_reads() {
    let e = Engine::new();
    let f = facts(&e, r#"{"map": [{"var": "xs"}, {"val": [[1], "index"]}]}"#);
    assert_eq!(dotted(&f), ["xs"]);
}

#[test]
fn level_marker_at_root() {
    // No frame is pushed, so every level clamps to the root.
    let f = facts(&Engine::new(), r#"{"val": [[2], "a"]}"#);
    assert_eq!(dotted(&f), ["a"]);
}

#[cfg(feature = "ext-array")]
#[test]
fn key_expressions_are_scoped() {
    let e = Engine::new();
    assert_eq!(
        dotted(&facts(
            &e,
            r#"{"sort": [{"var": "xs"}, true, {"var": "k"}]}"#
        )),
        ["xs"]
    );
    assert_eq!(
        dotted(&facts(&e, r#"{"group_by": [{"var": "xs"}, {"var": "k"}]}"#)),
        ["xs"]
    );
    assert_eq!(
        dotted(&facts(&e, r#"{"distinct": [{"var": "xs"}, {"var": "k"}]}"#)),
        ["xs"]
    );
}

/// `try`'s catch arm runs under the caught-error frame: a level-0 read
/// there reads the error, a level-1 read the root.
#[cfg(feature = "error-handling")]
#[test]
fn try_catch_arm_is_scoped() {
    let e = Engine::new();
    let f = facts(&e, r#"{"try": [{"var": "a"}, {"var": "type"}]}"#);
    assert_eq!(dotted(&f), ["a"]);

    let rule = r#"{"try": [{"throw": "boom"}, {"val": [[1], "fallback"]}]}"#;
    assert_eq!(dotted(&facts(&e, rule)), ["fallback"]);
    assert_eq!(e.eval_str(rule, r#"{"fallback": 7}"#).unwrap(), "7");

    // A single-argument `try` has no catch arm, so nothing is scoped.
    assert_eq!(dotted(&facts(&e, r#"{"try": [{"var": "a"}]}"#)), ["a"]);
}

// ── missing / exists / fractional ───────────────────────────────────────

#[test]
fn missing_paths() {
    let e = Engine::new();
    assert_eq!(
        dotted(&facts(&e, r#"{"missing": ["a", "b.c"]}"#)),
        ["a", "b.c"]
    );
    // A literal list of paths is as static as separate arguments.
    assert_eq!(
        dotted(&facts(&e, r#"{"missing": [["a", "b"]]}"#)),
        ["a", "b"]
    );
    assert_eq!(
        dotted(&facts(&no_fold(), r#"{"missing": [["a", "b"]]}"#)),
        ["a", "b"]
    );
    let f = facts(&e, r#"{"missing": [{"var": "which"}]}"#);
    assert!(f.has_computed_reads());
    assert_eq!(dotted(&f), ["which"]);
}

#[test]
fn missing_some_paths() {
    let e = Engine::new();
    let f = facts(&e, r#"{"missing_some": [1, ["a", "b.c"]]}"#);
    assert_eq!(dotted(&f), ["a", "b.c"]);
    assert!(f.reads_complete());

    let f = facts(&e, r#"{"missing_some": [{"var": "min"}, ["a"]]}"#);
    assert_eq!(dotted(&f), ["a", "min"]);
    assert!(!f.has_computed_reads());

    let f = facts(&e, r#"{"missing_some": [1, {"var": "paths"}]}"#);
    assert!(f.has_computed_reads());
    assert_eq!(dotted(&f), ["paths"]);
}

#[cfg(feature = "ext-control")]
#[test]
fn exists_paths() {
    let e = Engine::new();
    assert_eq!(
        segs(&facts(&e, r#"{"exists": ["a", "b"]}"#)),
        [vec!["a", "b"]]
    );
    // `exists` never splits on dots either.
    assert_eq!(segs(&facts(&e, r#"{"exists": "a.b"}"#)), [vec!["a.b"]]);
    // No path names the data itself, which always exists: nothing is read.
    assert!(facts(&e, r#"{"exists": []}"#).reads().is_empty());

    let f = facts(&e, r#"{"exists": [{"var": "k"}]}"#);
    assert!(f.has_computed_reads());

    let f = facts(&e, r#"{"map": [{"var": "xs"}, {"exists": "id"}]}"#);
    assert_eq!(dotted(&f), ["xs"]);
}

/// Without a bucketing expression `fractional` hashes `targetingKey` and
/// `$flagd.flagKey` from the root, at any depth. Which form a call takes is
/// decided at runtime, so both are always reported.
#[cfg(feature = "flagd")]
#[test]
fn fractional_reads_targeting_fields() {
    let f = facts(
        &Engine::new(),
        r#"{"fractional": [{"var": "email"}, ["a", 50], ["b", 50]]}"#,
    );
    assert_eq!(
        segs(&f),
        [
            vec!["$flagd", "flagKey"],
            vec!["email"],
            vec!["targetingKey"]
        ]
    );
    assert!(f.reads_complete());
    assert!(f.is_deterministic());
}

// ── templating ──────────────────────────────────────────────────────────

#[cfg(feature = "templating")]
#[test]
fn structured_object_fields() {
    let e = Engine::builder().with_templating(true).build();
    let f = facts(&e, r#"{"name": {"var": "n"}, "age": {"var": "a"}, "k": 1}"#);
    assert_eq!(dotted(&f), ["a", "n"]);
}

// ── operators ───────────────────────────────────────────────────────────

#[test]
fn operators_are_canonical_sorted_and_unique() {
    let e = Engine::new();
    let f = facts(
        &e,
        r#"{"?:": [{">": [{"var": "x"}, 1]}, {"val": "a"}, {">": [{"var": "y"}, 2]}]}"#,
    );
    assert_eq!(f.operators(), [">", "if", "val"]);
    assert!(f.custom_operators().is_empty());
}

#[test]
fn specialised_nodes_report_their_operator() {
    let e = Engine::new();
    assert_eq!(facts(&e, r#"{"missing": ["a"]}"#).operators(), ["missing"]);
    assert_eq!(
        facts(&e, r#"{"missing_some": [1, ["a"]]}"#).operators(),
        ["missing_some"]
    );
    // A computed `var` path with a default compiles to an internal opcode;
    // it is still reported as `val`.
    assert_eq!(
        facts(&e, r#"{"var": [{"var": "k"}, 0]}"#).operators(),
        ["val"]
    );
}

#[cfg(feature = "ext-control")]
#[test]
fn exists_is_reported() {
    let e = Engine::new();
    assert_eq!(facts(&e, r#"{"exists": "a"}"#).operators(), ["exists"]);
}

#[cfg(feature = "error-handling")]
#[test]
fn throw_is_reported() {
    let e = Engine::new();
    let f = facts(&e, r#"{"throw": "boom"}"#);
    assert_eq!(f.operators(), ["throw"]);
    assert!(f.is_deterministic());
    assert!(!f.reads_data());
}

/// A malformed call still names its operator, and reads nothing: its
/// arguments are never evaluated.
#[test]
fn invalid_arguments_name_the_operator() {
    let f = facts(&Engine::new(), r#"{"if": {"var": "x"}}"#);
    assert_eq!(f.operators(), ["if"]);
    assert!(f.reads().is_empty());
}

/// Folded subtrees are gone from the compiled rule, so their operators are
/// too. Without folding they stay.
#[test]
fn folded_operators_are_not_reported() {
    let rule = r#"{"+": [1, {"*": [2, 3]}]}"#;
    let f = facts(&Engine::new(), rule);
    assert!(f.operators().is_empty());
    assert!(!f.reads_data());
    assert!(f.reads_complete());

    assert_eq!(facts(&no_fold(), rule).operators(), ["*", "+"]);
}

#[test]
fn dead_branches_are_not_reported() {
    let rule = r#"{"if": [true, {"var": "a"}, {"var": "b"}]}"#;
    assert_eq!(dotted(&facts(&Engine::new(), rule)), ["a"]);
    assert_eq!(dotted(&facts(&no_fold(), rule)), ["a", "b"]);
}

#[test]
fn shared_subexpressions_are_read_once() {
    let agg =
        r#"{"reduce": [{"var": "xs"}, {"+": [{"var": "accumulator"}, {"var": "current"}]}, 0]}"#;
    let rule = format!(r#"{{"+": [{agg}, {agg}]}}"#);
    let logic = Engine::new().compile(rule.as_str()).unwrap();
    assert_eq!(logic.cse_slot_count(), 1);
    let f = logic.facts();
    assert_eq!(dotted(&f), ["xs"]);
    assert_eq!(f.operators(), ["+", "reduce", "val"]);
}

// ── determinism and custom operators ────────────────────────────────────

#[test]
fn literal_rule() {
    let f = facts(&Engine::new(), r#"[1, "two", null]"#);
    assert!(f.reads().is_empty());
    assert!(f.operators().is_empty());
    assert!(f.is_deterministic());
    assert!(f.reads_complete());
    assert!(!f.reads_data());
}

#[cfg(feature = "datetime")]
#[test]
fn now_is_not_deterministic() {
    let f = facts(
        &Engine::new(),
        r#"{"date_diff": [{"now": []}, {"var": "since"}, "days"]}"#,
    );
    assert!(!f.is_deterministic());
    assert_eq!(f.operators(), ["date_diff", "now", "val"]);
    assert_eq!(dotted(&f), ["since"]);
}

struct Double;

impl datalogic_rs::CustomOperator for Double {
    fn evaluate<'a>(
        &self,
        args: &[&'a datalogic_rs::DataValue<'a>],
        _ctx: &mut datalogic_rs::operator::EvalContext<'_, 'a>,
        arena: &'a datalogic_rs::bumpalo::Bump,
    ) -> datalogic_rs::Result<&'a datalogic_rs::DataValue<'a>> {
        let n = args.first().and_then(|v| v.as_f64()).unwrap_or(0.0);
        Ok(arena.alloc(datalogic_rs::DataValue::from_f64(n * 2.0)))
    }
}

/// A custom operator can read the whole context through
/// `EvalContext::root_input`, and may be anything but deterministic, so the
/// facts cannot vouch for it.
#[test]
fn custom_operators_are_opaque() {
    let e = Engine::builder().add_operator("double", Double).build();
    let f = facts(&e, r#"{"+": [{"double": [{"var": "x"}]}, 1]}"#);
    assert_eq!(f.custom_operators(), ["double"]);
    assert_eq!(f.operators(), ["+", "val"]);
    assert_eq!(dotted(&f), ["x"]);
    assert!(!f.has_computed_reads());
    assert!(!f.reads_complete());
    assert!(!f.is_deterministic());
    assert!(f.reads_data());
}

// ── soundness over every suite case ─────────────────────────────────────

#[cfg(all(
    feature = "serde_json",
    feature = "templating",
    feature = "all-operators"
))]
mod suites {
    use super::*;
    use serde_json::{Map, Value};
    use std::path::Path;

    /// Copy the part of `data` that `path` names into `out`, keeping every
    /// container along the way. A path that runs off the data copies the
    /// deepest value it reached, which is what the lookup observed.
    fn graft(data: &Value, path: &[String], out: &mut Value) {
        let Some((head, rest)) = path.split_first() else {
            *out = data.clone();
            return;
        };
        match data {
            Value::Object(fields) => {
                let Some(child) = fields.get(head) else {
                    return;
                };
                if !out.is_object() {
                    *out = Value::Object(Map::new());
                }
                let slot = out
                    .as_object_mut()
                    .unwrap()
                    .entry(head.clone())
                    .or_insert(Value::Null);
                graft(child, rest, slot);
            }
            Value::Array(items) => {
                let Some(child) = head.parse::<usize>().ok().and_then(|i| items.get(i)) else {
                    *out = data.clone();
                    return;
                };
                let index = head.parse::<usize>().unwrap();
                if !out.is_array() {
                    *out = Value::Array(Vec::new());
                }
                let arr = out.as_array_mut().unwrap();
                if arr.len() <= index {
                    arr.resize(index + 1, Value::Null);
                }
                graft(child, rest, &mut arr[index]);
            }
            _ => *out = data.clone(),
        }
    }

    /// `data` cut down to the reported reads.
    fn prune(data: &Value, facts: &Facts) -> Value {
        // An empty object stands for "nothing read": no rule that reads
        // nothing can tell it apart from the original.
        let mut out = match data {
            Value::Array(_) => Value::Array(Vec::new()),
            _ => Value::Object(Map::new()),
        };
        for path in facts.reads() {
            graft(data, path.segments(), &mut out);
        }
        out
    }

    fn outcome(engine: &Engine, logic: &datalogic_rs::Logic, data: &Value) -> String {
        match engine.session().eval_str(logic, data.to_string().as_str()) {
            Ok(v) => format!("ok {v}"),
            Err(e) => format!("err {}", e.tag()),
        }
    }

    fn suite_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                suite_files(&path, out);
            } else if path.extension().is_some_and(|x| x == "json")
                && path.file_name().is_some_and(|n| n != "index.json")
            {
                out.push(path);
            }
        }
    }

    /// For every suite case whose reads are complete and whose result is a
    /// function of the data: the reported reads are enough to reproduce the
    /// outcome, on both the folded and the unfolded compile. And the folded
    /// compile never reports a read the unfolded one does not cover.
    #[test]
    fn reported_reads_are_sufficient() {
        let mut files = Vec::new();
        suite_files(Path::new("tests/suites"), &mut files);
        files.sort();

        let mut engines: std::collections::HashMap<(bool, Option<char>, bool), Engine> =
            Default::default();
        let mut checked = 0usize;
        let mut failures = Vec::new();

        for file in &files {
            let cases: Value = serde_json::from_str(&std::fs::read_to_string(file).unwrap())
                .unwrap_or_else(|e| panic!("{}: {e}", file.display()));
            for (index, case) in cases.as_array().unwrap().iter().enumerate() {
                let Some(case) = case.as_object() else {
                    continue;
                };
                let rule = &case["rule"];
                let data = case
                    .get("data")
                    .cloned()
                    .unwrap_or(Value::Object(Map::new()));
                let templating = case
                    .get("templating")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let escape = case
                    .get("template_key_escape")
                    .and_then(Value::as_str)
                    .and_then(|s| s.chars().next());

                let mut per_mode = Vec::new();
                for folding in [true, false] {
                    let engine =
                        engines
                            .entry((templating, escape, folding))
                            .or_insert_with(|| {
                                let mut b = Engine::builder()
                                    .with_templating(templating)
                                    .with_constant_folding(folding);
                                if let Some(c) = escape {
                                    b = b.with_template_key_escape(c);
                                }
                                b.build()
                            });
                    let Ok(logic) = engine.compile(rule) else {
                        continue;
                    };
                    let f = logic.facts();
                    let label = format!(
                        "{}#{index} ({}) folding={folding}",
                        file.display(),
                        case.get("description")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                    );
                    if f.reads_complete() && f.is_deterministic() {
                        let pruned = prune(&data, &f);
                        let full = outcome(engine, &logic, &data);
                        let cut = outcome(engine, &logic, &pruned);
                        if full != cut {
                            failures.push(format!(
                                "{label}\n  rule:   {rule}\n  reads:  {:?}\n  data:   {data}\n  pruned: {pruned}\n  full:   {full}\n  pruned: {cut}",
                                f.reads().iter().map(|p| p.segments()).collect::<Vec<_>>()
                            ));
                        }
                        checked += 1;
                    }
                    per_mode.push((label, f));
                }

                if let [(label, folded), (_, unfolded)] = &per_mode[..]
                    && !unfolded.has_computed_reads()
                {
                    for read in folded.reads() {
                        let covered = unfolded
                            .reads()
                            .iter()
                            .any(|u| read.segments().starts_with(u.segments()));
                        if !covered {
                            failures.push(format!(
                                "{label}\n  folded read {read} is not covered by the unfolded reads {:?}",
                                unfolded.reads().iter().map(ToString::to_string).collect::<Vec<_>>()
                            ));
                        }
                    }
                }
            }
        }

        assert!(checked > 1500, "only {checked} cases checked");
        assert!(
            failures.is_empty(),
            "{} of {checked} checks failed:\n\n{}",
            failures.len(),
            failures.join("\n\n")
        );
    }
}
