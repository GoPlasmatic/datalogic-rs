//! Budget audit, generated from the operator table: every operator whose
//! row declares a cost other than `node` must charge more for a larger
//! input. Catches the "charged far less than the work" class of bug as a
//! test failure instead of a manual audit.
//!
//! Each costed operator needs a size-parameterised probe below. A new row
//! with a cost and no probe fails `every_costed_operator_has_a_probe`, so
//! the audit cannot be skipped by omission.
#![cfg(all(feature = "budget", feature = "all-operators"))]

use bumpalo::Bump;
use datalogic_rs::Engine;

/// A string of `n` bytes.
fn text(n: usize) -> String {
    "a".repeat(n)
}

/// A JSON array `[0, 1, ..., n-1]`.
fn ints(n: usize) -> String {
    let items: Vec<String> = (0..n).map(|i| i.to_string()).collect();
    format!("[{}]", items.join(","))
}

/// A JSON object with `n` keys.
fn object(n: usize) -> String {
    let pairs: Vec<String> = (0..n).map(|i| format!("\"k{i}\":{i}")).collect();
    format!("{{{}}}", pairs.join(","))
}

/// An `n`-element zero `f32` tensor in datavalue's tagged wire form. `n`
/// must be a multiple of 3 so the base64 payload needs no padding.
fn tensor(n: usize) -> String {
    assert!(n.is_multiple_of(3));
    // `4n` zero bytes encode to `16n / 3` base64 `A`s.
    let data = "A".repeat(16 * n / 3);
    format!("{{\"tensor\":{{\"dtype\":\"f32\",\"shape\":[{n}],\"data\":\"{data}\"}}}}")
}

/// `(rule, data)` for operator `name` at input size `n`.
fn probe(name: &str, n: usize) -> Option<(&'static str, String)> {
    let s = || format!("{{\"s\":\"{}\"}}", text(n));
    let xs = || format!("{{\"xs\":{}}}", ints(n));
    let o = || format!("{{\"o\":{}}}", object(n));
    let n_ = || format!("{{\"n\":{n},\"xs\":{}}}", ints(n));
    let t = || format!("{{\"t\":{}}}", tensor(n));
    // `n` ones: a divisor list with no zero in it.
    let ys = || format!("{{\"ys\":[{}]}}", vec!["1"; n].join(","));
    let ps = || {
        let paths: Vec<String> = (0..n).map(|i| format!("\"p{i}\"")).collect();
        format!("{{\"ps\":[{}]}}", paths.join(","))
    };
    Some(match name {
        "cat" => (r#"{"cat":[{"var":"s"},{"var":"s"}]}"#, s()),
        "substr" => (r#"{"substr":[{"var":"s"},1]}"#, s()),
        "in" => (r#"{"in":["zz",{"var":"s"}]}"#, s()),
        "starts_with" => (r#"{"starts_with":[{"var":"s"},{"var":"s"}]}"#, s()),
        "ends_with" => (r#"{"ends_with":[{"var":"s"},{"var":"s"}]}"#, s()),
        "upper" => (r#"{"upper":[{"var":"s"}]}"#, s()),
        "lower" => (r#"{"lower":[{"var":"s"}]}"#, s()),
        "trim" => (r#"{"trim":[{"var":"s"}]}"#, s()),
        "split" => (r#"{"split":[{"var":"s"},","]}"#, s()),
        "merge" => (r#"{"merge":[{"var":"xs"},{"var":"xs"}]}"#, xs()),
        "filter" => (r#"{"filter":[{"var":"xs"},true]}"#, xs()),
        "map" => (r#"{"map":[{"var":"xs"},1]}"#, xs()),
        "reduce" => (
            r#"{"reduce":[{"var":"xs"},{"+":[{"var":"accumulator"},1]},0]}"#,
            xs(),
        ),
        "all" => (r#"{"all":[{"var":"xs"},true]}"#, xs()),
        "some" => (r#"{"some":[{"var":"xs"},false]}"#, xs()),
        "none" => (r#"{"none":[{"var":"xs"},false]}"#, xs()),
        "sort" => (r#"{"sort":[{"var":"xs"}]}"#, xs()),
        // A contiguous slice borrows a sub-slice and is rightly uncharged;
        // a stepped one copies, one item at a time.
        "slice" => (r#"{"slice":[{"var":"xs"},0,1000000,2]}"#, xs()),
        "group_by" => (r#"{"group_by":[{"var":"xs"},{"var":""}]}"#, xs()),
        "distinct" => (r#"{"distinct":[{"var":"xs"}]}"#, xs()),
        "keys" => (r#"{"keys":[{"var":"o"}]}"#, o()),
        "values" => (r#"{"values":[{"var":"o"}]}"#, o()),
        "entries" => (r#"{"entries":[{"var":"o"}]}"#, o()),
        "tensor" => (r#"{"tensor":[{"var":"xs"},"f32"]}"#, xs()),
        "zeros" => (r#"{"zeros":[[{"var":"n"}],"f32"]}"#, n_()),
        "full" => (r#"{"full":[[{"var":"n"}],"f32",1]}"#, n_()),
        "scatter" => (r#"{"scatter":[[[0]],[{"var":"n"}],"f32"]}"#, n_()),
        "rle_expand" => (
            r#"{"rle_expand":[[1,{"var":"n"}],[{"var":"n"}],"f32"]}"#,
            n_(),
        ),
        "one_hot" => (r#"{"one_hot":[{"var":"xs"},3,"f32"]}"#, n_()),
        "stack" => (r#"{"stack":[[{"var":"t"},{"var":"t"}],0]}"#, t()),
        "concat" => (r#"{"concat":[[{"var":"t"},{"var":"t"}],0]}"#, t()),
        "unstack" => (r#"{"unstack":[{"var":"t"},0]}"#, t()),
        "transpose" => (r#"{"transpose":[{"var":"t"}]}"#, t()),
        "pad" => (r#"{"pad":[{"var":"t"},[1],[1]]}"#, t()),
        "crop" => (r#"{"crop":[{"var":"t"},[0],[1]]}"#, t()),
        "cast" => (r#"{"cast":[{"var":"t"},"f64"]}"#, t()),
        "normalize" => (r#"{"normalize":[{"var":"t"},0]}"#, t()),
        "argmax" => (r#"{"argmax":[{"var":"t"},0]}"#, t()),
        "gather" => (r#"{"gather":[{"var":"t"},[0]]}"#, t()),
        "to_list" => (r#"{"to_list":[{"var":"t"}]}"#, t()),
        // Equality charges the containers it walks.
        "==" | "===" | "!=" | "!==" => {
            let rule = match name {
                "==" => r#"{"==":[{"var":"o"},{"var":"p"}]}"#,
                "===" => r#"{"===":[{"var":"o"},{"var":"p"}]}"#,
                "!=" => r#"{"!=":[{"var":"o"},{"var":"p"}]}"#,
                _ => r#"{"!==":[{"var":"o"},{"var":"p"}]}"#,
            };
            (rule, format!("{{\"o\":{0},\"p\":{0}}}", object(n)))
        }
        // The arithmetic operators fold a lone computed array per item.
        "+" => (r#"{"+":[{"var":"xs"}]}"#, xs()),
        "-" => (r#"{"-":[{"var":"xs"}]}"#, xs()),
        "*" => (r#"{"*":[{"var":"xs"}]}"#, xs()),
        "/" => (r#"{"/":[{"var":"ys"}]}"#, ys()),
        "%" => (r#"{"%":[{"var":"ys"}]}"#, ys()),
        "max" => (r#"{"max":[{"var":"xs"}]}"#, xs()),
        "min" => (r#"{"min":[{"var":"xs"}]}"#, xs()),
        // A path list from data is charged per path.
        "missing" => (r#"{"missing":{"var":"ps"}}"#, ps()),
        "missing_some" => (r#"{"missing_some":[1,{"var":"ps"}]}"#, ps()),
        "length" => (r#"{"length":[{"var":"s"}]}"#, s()),
        // Every item is compared against every group so far.
        "group_by" => (r#"{"group_by":[{"var":"xs"},{"var":""}]}"#, xs()),
        _ => return None,
    })
}

fn ops_spent(engine: &Engine, rule: &str, data: &str) -> u64 {
    let compiled = engine.compile(rule).expect("probe compiles");
    let arena = Bump::new();
    engine
        .evaluate_metered(&compiled, data, &arena, u64::MAX)
        .unwrap_or_else(|e| panic!("probe {rule} failed: {e}"))
        .ops
}

fn costed() -> impl Iterator<Item = datalogic_rs::OperatorInfo> {
    Engine::new().operators().filter(|op| op.cost != "node")
}

#[test]
fn every_costed_operator_has_a_probe() {
    let missing: Vec<&str> = costed()
        .filter(|op| probe(op.name, 3).is_none())
        .map(|op| op.name)
        .collect();
    assert!(
        missing.is_empty(),
        "costed operators with no audit probe: {missing:?}"
    );
}

#[test]
fn costed_operators_charge_in_proportion_to_their_input() {
    let engine = Engine::new();
    for op in costed() {
        let Some((rule, small)) = probe(op.name, 3) else {
            continue;
        };
        let (_, large) = probe(op.name, 3000).expect("same probe");
        let small_ops = ops_spent(&engine, rule, &small);
        let large_ops = ops_spent(&engine, rule, &large);
        // 1000x the input must cost clearly more than a constant charge
        // would. Strings are priced in 64-byte units, so 3000 bytes is ~47
        // units; collections and tensors are priced per item or element.
        assert!(
            large_ops >= small_ops + 10,
            "`{}` (cost `{}`) charged {small_ops} ops at size 3 and {large_ops} at size 3000",
            op.name,
            op.cost
        );
    }
}
