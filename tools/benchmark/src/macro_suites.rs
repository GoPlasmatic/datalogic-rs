//! Macro-benchmark tier: suites synthesized in code, nothing large
//! checked in.
//!
//! The JSON operator suites under `tests/suites/` use payloads of at most
//! a few hundred bytes, so they measure operator dispatch, not data-volume
//! behaviour. The macro tier fills that gap for the self benchmark:
//!
//! - `macro/array-1k`, `macro/array-10k`: numeric and object arrays with
//!   filter/map/reduce/sort/in-membership rules.
//! - `macro/object-128key`: a 128-key object with shallow and dotted-deep
//!   `var` lookups plus a `merge` of two 64-element arrays.
//! - `macro/deep-48`: 48 levels of nesting with a 49-segment `var` path.
//! - `macro/string-10kb`: 10 KB strings through `cat`, `substr`, and
//!   substring `in`.
//! - `macro/eligibility`: one realistic eligibility rule combining
//!   and/or/comparisons/missing/reduce over a medium application object.
//!
//! Because a single evaluation here can cost microseconds instead of
//! nanoseconds, the runner in `bin/self.rs` scales the per-suite iteration
//! count from a pilot pass (the same idea `bin/compare.rs` uses per cell)
//! instead of using the fixed micro-suite iteration count.

use serde_json::{Value, json};

use crate::SuiteCase;

/// One synthesized suite: a name for the per-suite report line plus the
/// same (rule, data) string pairs the JSON suite loader produces.
pub struct MacroSuite {
    pub name: &'static str,
    pub cases: Vec<SuiteCase>,
}

/// All macro suites, in run order.
pub fn macro_suites() -> Vec<MacroSuite> {
    vec![
        array_suite("macro/array-1k", 1_000),
        array_suite("macro/array-10k", 10_000),
        wide_object_suite(),
        deep_nesting_suite(),
        big_string_suite(),
        eligibility_suite(),
    ]
}

fn case(rule: &Value, data: &Value) -> SuiteCase {
    SuiteCase {
        rule_json: rule.to_string(),
        data_json: data.to_string(),
    }
}

/// Numeric + object arrays of `n` elements. The numeric array is a full
/// permutation of `0..n` (7919 is prime and coprime to both sizes), so
/// `sort` does real work instead of fast-pathing pre-sorted input.
fn array_suite(name: &'static str, n: usize) -> MacroSuite {
    let nums: Vec<Value> = (0..n).map(|i| json!((i * 7919) % n)).collect();
    let users: Vec<Value> = (0..n)
        .map(|i| {
            json!({
                "id": i,
                "score": (i * 37) % 100,
                "active": i % 3 != 0,
            })
        })
        .collect();
    let data = json!({ "nums": nums, "users": users, "needle": n - 1 });

    let rules = [
        // Keep the upper half.
        json!({"filter": [{"var": "nums"}, {">": [{"var": ""}, n / 2]}]}),
        // Arithmetic on every element.
        json!({"map": [{"var": "nums"}, {"*": [{"var": ""}, 2]}]}),
        // Sum via reduce.
        json!({"reduce": [
            {"var": "nums"},
            {"+": [{"var": "current"}, {"var": "accumulator"}]},
            0
        ]}),
        // Sort the shuffled permutation ascending.
        json!({"sort": [{"var": "nums"}]}),
        // Membership scan; the needle maps to the last insertion.
        json!({"in": [{"var": "needle"}, {"var": "nums"}]}),
        // Object rows: predicate over two fields, then a pluck.
        json!({"filter": [
            {"var": "users"},
            {"and": [{"var": "active"}, {">=": [{"var": "score"}, 50]}]}
        ]}),
        json!({"map": [{"var": "users"}, {"var": "score"}]}),
    ];
    MacroSuite {
        name,
        cases: rules.iter().map(|r| case(r, &data)).collect(),
    }
}

/// A 128-key object plus a small nested subtree and two 64-element lists.
fn wide_object_suite() -> MacroSuite {
    let mut obj = serde_json::Map::new();
    for i in 0..128 {
        obj.insert(format!("k{i:03}"), json!(i));
    }
    obj.insert(
        "nested".to_string(),
        json!({"a": {"b": {"c": {"leaf": 42}}}}),
    );
    obj.insert("list_a".to_string(), json!((0..64).collect::<Vec<i64>>()));
    obj.insert("list_b".to_string(), json!((64..128).collect::<Vec<i64>>()));
    let data = Value::Object(obj);

    let rules = [
        // Shallow lookups at the start, middle, and end of the key range.
        json!({"var": "k000"}),
        json!({"var": "k064"}),
        json!({"var": "k127"}),
        // Dotted deep path through the nested subtree.
        json!({"var": "nested.a.b.c.leaf"}),
        // Merge two 64-element arrays pulled from the wide object.
        json!({"merge": [{"var": "list_a"}, {"var": "list_b"}]}),
    ];
    MacroSuite {
        name: "macro/object-128key",
        cases: rules.iter().map(|r| case(r, &data)).collect(),
    }
}

/// 48 levels of nesting, addressed by one long dotted `var` path.
fn deep_nesting_suite() -> MacroSuite {
    const DEPTH: usize = 48;
    let mut data = json!({"leaf": 42});
    for _ in 0..DEPTH {
        data = json!({"n": data});
    }
    let path = format!("{}.leaf", vec!["n"; DEPTH].join("."));

    let rules = [json!({"var": path}), json!({"==": [{"var": path}, 42]})];
    MacroSuite {
        name: "macro/deep-48",
        cases: rules.iter().map(|r| case(r, &data)).collect(),
    }
}

/// Two distinct ~10 KB strings through `cat`, `substr`, and substring `in`.
fn big_string_suite() -> MacroSuite {
    let s: String = "lorem ipsum dolor sit amet 0123456789 "
        .chars()
        .cycle()
        .take(10 * 1024)
        .collect();
    let t: String = "the quick brown fox jumps over the lazy dog "
        .chars()
        .cycle()
        .take(10 * 1024)
        .collect();
    let data = json!({ "s": s, "t": t });

    let rules = [
        // Concatenate the two 10 KB strings (20 KB result per eval).
        json!({"cat": [{"var": "s"}, {"var": "t"}]}),
        // Slice out of the middle, and from the tail via negative start.
        json!({"substr": [{"var": "s"}, 5000, 512]}),
        json!({"substr": [{"var": "s"}, -512]}),
        // Substring membership scan across the 10 KB haystack.
        json!({"in": ["0123456789", {"var": "s"}]}),
    ];
    MacroSuite {
        name: "macro/string-10kb",
        cases: rules.iter().map(|r| case(r, &data)).collect(),
    }
}

/// One realistic eligibility-style rule over a medium application object:
/// required-field check (`missing`), age band, income or
/// credit-plus-guarantor alternatives, country allowlist, flag blocklist,
/// and a debt-to-income cap computed with `reduce`.
fn eligibility_suite() -> MacroSuite {
    let data = json!({
        "applicant": {
            "age": 34,
            "income": 72000,
            "employment_years": 6,
            "credit_score": 715,
            "country": "DE",
            "has_guarantor": false,
            "email": "a@example.com",
            "flags": ["verified"],
            "dependents": 2,
            "existing_loans": [
                {"balance": 3200, "monthly": 140},
                {"balance": 900, "monthly": 45}
            ]
        },
        "loan": {"amount": 25000, "term_months": 60, "purpose": "auto"},
        "limits": {"min_age": 18, "max_age": 70, "min_income": 30000, "dti_max": 0.4}
    });

    let rule = json!({"and": [
        {"!": {"missing": [
            "applicant.age", "applicant.income", "applicant.country", "loan.amount"
        ]}},
        {">=": [{"var": "applicant.age"}, {"var": "limits.min_age"}]},
        {"<=": [{"var": "applicant.age"}, {"var": "limits.max_age"}]},
        {"or": [
            {">=": [{"var": "applicant.income"}, {"var": "limits.min_income"}]},
            {"and": [
                {">=": [{"var": "applicant.credit_score"}, 700]},
                {"==": [{"var": "applicant.has_guarantor"}, true]}
            ]}
        ]},
        {"in": [{"var": "applicant.country"}, ["US", "CA", "GB", "DE", "FR", "NL"]]},
        {"!": {"in": ["fraud", {"var": "applicant.flags"}]}},
        {"<": [
            {"reduce": [
                {"var": "applicant.existing_loans"},
                {"+": [{"var": "current.monthly"}, {"var": "accumulator"}]},
                0
            ]},
            {"*": [{"var": "applicant.income"}, {"var": "limits.dti_max"}]}
        ]}
    ]});

    MacroSuite {
        name: "macro/eligibility",
        cases: vec![case(&rule, &data)],
    }
}
