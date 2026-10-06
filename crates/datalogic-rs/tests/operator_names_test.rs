//! Integration tests for `Engine::builtin_operator_names` (issue #65).
//!
//! Deliberately NOT gated on `serde_json`: the per-family assertions below
//! run in both directions (`cfg(feature)` and `cfg(not(feature))`), and
//! the `not` legs only execute under `--no-default-features`, where a
//! `serde_json` gate would skip the whole file.

use bumpalo::Bump;
use datalogic_rs::__private::CATALOGUE;
#[cfg(feature = "templating")]
use datalogic_rs::datavalue::OwnedDataValue;
use datalogic_rs::operator::EvalContext;
use datalogic_rs::{CustomOperator, DataValue, Engine, Result, ScopedArg};

fn names() -> Vec<&'static str> {
    Engine::new().builtin_operator_names().collect()
}

/// JSON-quote an operator name. Rust's `{:?}` is close but not JSON: it
/// escapes `'` as `\'` and emits `\u{1f600}` for non-ASCII, neither of
/// which parses. Every name is plain ASCII today, so a `{:?}` rule would
/// silently become unparseable — and the test below would pass vacuously
/// — the first time one is not.
#[cfg(feature = "templating")]
fn json_quoted(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 2);
    out.push('"');
    for c in name.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[test]
fn baseline_names_are_always_present() {
    let names = names();
    for expected in [
        "val",
        "var",
        "==",
        "!==",
        "!",
        "!!",
        "and",
        "or",
        "if",
        "?:",
        "+",
        "%",
        "max",
        "cat",
        "substr",
        "in",
        "merge",
        "filter",
        "map",
        "reduce",
        "all",
        "some",
        "none",
        "missing",
        "missing_some",
    ] {
        assert!(names.contains(&expected), "baseline {expected:?} missing");
    }
}

#[test]
fn canonical_name_precedes_its_alias() {
    let names = names();
    let pos = |n: &str| names.iter().position(|x| *x == n).unwrap();
    assert!(pos("val") < pos("var"));
    assert!(pos("if") < pos("?:"));
}

#[test]
fn iterator_does_not_borrow_the_engine() {
    // `+ use<>` on the return type: the iterator must outlive the engine
    // it was obtained from, since the vocabulary is a build-time fact.
    let iter = {
        let engine = Engine::new();
        engine.builtin_operator_names()
    };
    assert!(iter.count() > 0);
}

#[test]
fn builtin_and_custom_sets_are_disjoint() {
    struct Double;
    impl CustomOperator for Double {
        fn evaluate<'a>(
            &self,
            args: &[&'a DataValue<'a>],
            _ctx: &mut EvalContext<'_, 'a>,
            arena: &'a Bump,
        ) -> Result<&'a DataValue<'a>> {
            let n = args.first().and_then(|v| v.as_f64()).unwrap_or(0.0);
            Ok(arena.alloc(DataValue::from_f64(n * 2.0)))
        }
    }

    let engine = Engine::builder().add_operator("double", Double).build();
    assert!(!engine.builtin_operator_names().any(|n| n == "double"));
    assert!(engine.custom_operator_names().any(|n| n == "double"));
    // Registration does not change the built-in vocabulary.
    assert_eq!(
        engine.builtin_operator_names().count(),
        Engine::new().builtin_operator_names().count()
    );
}

/// The issue's motivating case: under templating mode an unknown key is
/// not an error, the object echoes back as a literal. Every name the
/// iterator reports must therefore be *live* — evaluating `{name: [1]}`
/// must never return the echoed object. An evaluation error is fine (it
/// proves the operator ran); only a silent echo is a drift bug.
#[cfg(feature = "templating")]
#[test]
fn every_reported_name_is_live_under_templating() {
    let engine = Engine::builder().with_templating(true).build();
    for name in engine.builtin_operator_names() {
        let rule = format!("{{{}: [1]}}", json_quoted(name));
        if let Ok(OwnedDataValue::Object(fields)) = engine.eval(rule.as_str(), "{}") {
            assert!(
                !(fields.len() == 1 && fields[0].0 == name),
                "{name:?} is reported as built-in but echoed back as a literal"
            );
        }
    }
    // Control: a name that is not in the vocabulary does echo.
    let echoed = engine.eval(r#"{"lenght": [1]}"#, "{}").unwrap();
    assert!(matches!(echoed, OwnedDataValue::Object(ref f) if f.len() == 1 && f[0].0 == "lenght"));
}

// ---------------------------------------------------------------------
// Per-family presence, asserted in both directions so a family can be
// neither leaked into a build that lacks it nor dropped from one that has
// it. Driven by the operator table's catalogue, which lists every family in
// the source whether or not this build compiled it in.
// ---------------------------------------------------------------------

/// Representative names per feature, written out here rather than read
/// from the table under test, so a name that moves to another family (or
/// loses its gate) fails. Asserted in both directions: present with the
/// feature, absent without it.
#[test]
fn representative_names_follow_their_feature() {
    let names = names();
    let families: [(&str, bool, &[&str]); 8] = [
        (
            "datetime",
            cfg!(feature = "datetime"),
            &[
                "datetime",
                "timestamp",
                "parse_date",
                "format_date",
                "date_diff",
                "now",
            ],
        ),
        (
            "ext-string",
            cfg!(feature = "ext-string"),
            &[
                "length",
                "starts_with",
                "ends_with",
                "upper",
                "lower",
                "trim",
                "split",
            ],
        ),
        (
            "ext-array",
            cfg!(feature = "ext-array"),
            &["sort", "slice", "group_by", "distinct"],
        ),
        (
            "ext-object",
            cfg!(feature = "ext-object"),
            &["keys", "values", "entries"],
        ),
        (
            "ext-control",
            cfg!(feature = "ext-control"),
            &["exists", "??", "switch", "match", "type"],
        ),
        (
            "error-handling",
            cfg!(feature = "error-handling"),
            &["try", "throw"],
        ),
        (
            "ext-math",
            cfg!(feature = "ext-math"),
            &["abs", "ceil", "floor"],
        ),
        ("flagd", cfg!(feature = "flagd"), &["fractional", "sem_ver"]),
    ];
    for (feature, enabled, expected) in families {
        for name in expected {
            assert_eq!(
                names.contains(name),
                enabled,
                "{name:?} with `{feature}` {}",
                if enabled { "on" } else { "off" }
            );
            let entry = CATALOGUE
                .iter()
                .find(|e| e.names.contains(name))
                .unwrap_or_else(|| panic!("{name:?} is not in the catalogue"));
            assert_eq!(entry.feature(), Some(feature), "{name:?}");
        }
    }
}

#[test]
fn compiled_families_are_reported_and_gated_ones_are_not() {
    let names = names();
    for entry in CATALOGUE {
        for name in entry.names {
            assert_eq!(
                names.contains(name),
                entry.enabled,
                "{name:?} ({} family, `{}`)",
                entry.family,
                entry.gate
            );
        }
    }
}

/// `[features]` of this crate's manifest: feature name → its list.
fn cargo_features() -> Vec<(String, Vec<String>)> {
    let manifest = include_str!("../Cargo.toml");
    let section = manifest
        .split("\n[features]\n")
        .nth(1)
        .expect("Cargo.toml has a [features] table");
    let section = section.split("\n[").next().unwrap_or(section);
    let mut features = Vec::new();
    let mut current: Option<(String, String)> = None;
    for line in section.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if let Some((name, rest)) = line.split_once('=') {
            if let Some(done) = current.take() {
                features.push(done);
            }
            current = Some((name.trim().to_string(), rest.trim().to_string()));
        } else if let Some((_, body)) = current.as_mut() {
            body.push_str(line);
        }
    }
    features.extend(current);
    features
        .into_iter()
        .map(|(name, body)| {
            let items = body
                .trim_matches(|c| c == '[' || c == ']')
                .split(',')
                .map(|item| item.trim().trim_matches('"').to_string())
                .filter(|item| !item.is_empty())
                .collect();
            (name, items)
        })
        .collect()
}

/// Every gated family names a real Cargo feature, and `all-operators`
/// enables exactly the operator families.
#[test]
fn catalogue_gates_match_cargo_features() {
    let features = cargo_features();
    let declared = |name: &str| features.iter().any(|(f, _)| f == name);
    let mut families: Vec<&str> = CATALOGUE.iter().filter_map(|e| e.feature()).collect();
    families.sort_unstable();
    families.dedup();
    for feature in &families {
        assert!(
            declared(feature),
            "family feature `{feature}` is not in Cargo.toml"
        );
    }
    let (_, all) = features
        .iter()
        .find(|(f, _)| f == "all-operators")
        .expect("`all-operators` feature");
    let mut all: Vec<&str> = all.iter().map(String::as_str).collect();
    all.sort_unstable();
    assert_eq!(
        all, families,
        "`all-operators` must enable exactly the operator families"
    );
    // Core needs no feature.
    assert!(
        CATALOGUE
            .iter()
            .any(|e| e.family == "Core" && e.feature().is_none())
    );
}

#[test]
fn operators_describe_every_named_builtin() {
    let engine = Engine::new();
    let mut from_ops: Vec<&str> = engine
        .operators()
        .flat_map(|op| std::iter::once(op.name).chain(op.aliases.iter().copied()))
        .collect();
    let mut from_names = names();
    from_ops.sort_unstable();
    from_names.sort_unstable();
    assert_eq!(from_ops, from_names);

    let val = engine.operators().find(|op| op.name == "val").unwrap();
    assert_eq!(val.aliases, ["var"]);
    assert!(val.reads_context);
    assert_eq!(val.effect, "pure");

    let map = engine.operators().find(|op| op.name == "map").unwrap();
    assert_eq!(map.scoped_arg, Some(ScopedArg::Index(1)));
    assert_eq!(map.cost, "per_item");

    let substr = engine.operators().find(|op| op.name == "substr").unwrap();
    assert_eq!((substr.min_args, substr.max_args), (1, Some(3)));

    for op in engine.operators() {
        let entry = CATALOGUE
            .iter()
            .find(|e| e.names.first() == Some(&op.name))
            .unwrap();
        assert_eq!(op.family, entry.family);
        assert_eq!(op.feature, entry.feature());
    }
}

#[cfg(feature = "error-handling")]
#[test]
fn try_reports_its_catch_arm_and_effect() {
    let engine = Engine::new();
    let try_ = engine.operators().find(|op| op.name == "try").unwrap();
    assert_eq!(try_.scoped_arg, Some(ScopedArg::LastOfMany));
    assert_eq!(try_.effect, "catches");
    assert_eq!(try_.feature, Some("error-handling"));
}
