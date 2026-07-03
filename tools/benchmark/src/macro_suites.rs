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
//! - `macro/checkout-40`: one large checkout-decision rule (completeness,
//!   risk screen, cart validation, promo pricing with cap, shipping,
//!   loyalty adjustment) over a 40-item order — **spec-compatible
//!   operators only**, so every cross-engine subject runs 100% of it and
//!   the row compares identical work across all engines.
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
        checkout_suite(),
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

/// One large, realistic checkout-decision rule over a 40-item order:
/// completeness (`missing` / `missing_some`), a risk screen
/// (verification, chargebacks, country allowlist, tenure-or-history with
/// a returns-ratio division, billing-address `some`, blocked-category
/// `none`, per-item `all` validation), then pricing: subtotal via
/// map+reduce, promo discount over the `filter`ed discountable lines
/// with a `min` cap, weight-based shipping with a `max` floor, an
/// every-5th-order perk (`%`), a loyalty-tier multiplier, and an order
/// limit gate.
///
/// **Fair-comparison suite**: it deliberately uses only spec-compatible
/// operators (the `compatible.json` operator set), so every subject in
/// the cross-engine matrix runs 100% of the work — no partial-coverage
/// cells in either direction. Pure-spec JSONLogic has no local
/// bindings, so aggregate subexpressions (the subtotal in particular)
/// are recomputed where referenced, exactly as deployed spec rules do.
/// The payload stays clear of the shapes measured elsewhere (no 10 KB
/// strings, no 10k-element arrays) so the row measures rule-logic
/// evaluation, not one operator's data-volume behaviour.
fn checkout_suite() -> MacroSuite {
    const CATS: [&str; 8] = [
        "electronics",
        "home",
        "toys",
        "books",
        "garden",
        "sports",
        "beauty",
        "grocery",
    ];
    let items: Vec<Value> = (0..40)
        .map(|i| {
            json!({
                "sku": format!("SKU-{i:04}"),
                "name": format!("Item number {i}"),
                "category": CATS[i % CATS.len()],
                "unit_price": 4.0 + ((i * 13) % 90) as f64 + 0.99,
                "qty": 1 + (i % 4),
                "weight_g": 120 + (i * 37) % 2200,
                "discountable": i % 5 != 0,
                "flags": if i % 11 == 0 { json!(["clearance"]) } else { json!([]) },
            })
        })
        .collect();

    // `years: 2` fails the tenure branch so the history branch (with its
    // returns-ratio division) stays on the executed path; the shipping
    // threshold is above the subtotal so the weight arithmetic runs too.
    let data = json!({
        "customer": {
            "id": "C-90211", "tier": "gold", "years": 2,
            "email": "ada@example.com", "email_verified": true,
            "country": "DE", "chargebacks_12m": 0,
            "orders_12m": 14, "returns_12m": 2,
            "addresses": [
                {"type": "billing", "country": "DE"},
                {"type": "shipping", "country": "DE"}
            ]
        },
        "cart": { "coupon": "SAVE15", "currency": "EUR", "items": items },
        "promo": { "code": "SAVE15", "min_subtotal": 150, "percent": 15,
                   "max_discount": 60 },
        "shipping": { "method": "express", "base": 12.9, "per_kg": 1.2,
                      "free_threshold": 10000 },
        "limits": { "max_order_value": 25000 }
    });

    let line_total = json!({"*": [{"var": "unit_price"}, {"var": "qty"}]});
    let subtotal = json!({"reduce": [
        {"map": [{"var": "cart.items"}, line_total]},
        {"+": [{"var": "current"}, {"var": "accumulator"}]},
        0
    ]});
    let discountable_sum = json!({"reduce": [
        {"map": [
            {"filter": [{"var": "cart.items"}, {"and": [
                {"var": "discountable"},
                {"!": {"in": [{"var": "category"}, ["giftcard", "alcohol"]]}}
            ]}]},
            line_total
        ]},
        {"+": [{"var": "current"}, {"var": "accumulator"}]},
        0
    ]});
    let discount = json!({"if": [
        {"and": [
            {"==": [{"var": "cart.coupon"}, {"var": "promo.code"}]},
            {">=": [subtotal, {"var": "promo.min_subtotal"}]}
        ]},
        {"min": [
            {"var": "promo.max_discount"},
            {"*": [discountable_sum, {"/": [{"var": "promo.percent"}, 100]}]}
        ]},
        0
    ]});
    let weight_kg = json!({"/": [
        {"reduce": [
            {"map": [{"var": "cart.items"}, {"*": [{"var": "weight_g"}, {"var": "qty"}]}]},
            {"+": [{"var": "current"}, {"var": "accumulator"}]},
            0
        ]},
        1000
    ]});
    let net = json!({"-": [subtotal, discount]});
    let shipping_cost = json!({"if": [
        {">=": [net, {"var": "shipping.free_threshold"}]},
        0,
        {"max": [0, {"+": [
            {"var": "shipping.base"},
            {"*": [{"var": "shipping.per_kg"}, weight_kg]}
        ]}]}
    ]});
    let perk = json!({"if": [
        {"==": [{"%": [{"var": "customer.orders_12m"}, 5]}, 0]},
        2.5,
        0
    ]});
    let tier_mult = json!({"if": [
        {"in": [{"var": "customer.tier"}, ["gold", "platinum"]]}, 0.98,
        {"==": [{"var": "customer.tier"}, "silver"]}, 0.99,
        1
    ]});
    let total = json!({"*": [
        {"-": [{"+": [net, shipping_cost]}, perk]},
        tier_mult
    ]});

    let risk_ok = json!({"and": [
        {"var": "customer.email_verified"},
        {"<=": [{"var": "customer.chargebacks_12m"}, 0]},
        {"in": [{"var": "customer.country"},
                ["US", "CA", "GB", "DE", "FR", "NL", "ES", "IT"]]},
        {"or": [
            {">": [{"var": "customer.years"}, 2]},
            {"and": [
                {">=": [{"var": "customer.orders_12m"}, 5]},
                {"<": [
                    {"/": [{"var": "customer.returns_12m"}, {"var": "customer.orders_12m"}]},
                    0.5
                ]}
            ]}
        ]},
        {"some": [{"var": "customer.addresses"}, {"==": [{"var": "type"}, "billing"]}]},
        {"none": [{"var": "cart.items"},
                  {"in": [{"var": "category"}, ["weapon", "tobacco"]]}]},
        {"all": [{"var": "cart.items"}, {"and": [
            {">=": [{"var": "qty"}, 1]},
            {"<=": [{"var": "qty"}, 10]},
            {">": [{"var": "unit_price"}, 0]}
        ]}]}
    ]});

    // Rejection branches rely on JSONLogic truthiness: `missing` /
    // `missing_some` return arrays, and a non-empty array is truthy.
    let rule = json!({"if": [
        {"or": [
            {"missing": ["customer.id", "customer.country", "cart.items",
                         "shipping.method", "promo.code"]},
            {"missing_some": [1, ["customer.email", "customer.phone"]]}
        ]},
        "rejected:incomplete",
        {"!": risk_ok},
        "rejected:risk",
        {">": [total, {"var": "limits.max_order_value"}]},
        "review:limit",
        total
    ]});

    MacroSuite {
        name: "macro/checkout-40",
        cases: vec![case(&rule, &data)],
    }
}
