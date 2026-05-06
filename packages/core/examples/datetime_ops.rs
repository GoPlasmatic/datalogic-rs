//! DateTime operators — parse, format, compare, and do arithmetic on dates.
//!
//! The `datetime` feature pulls in `chrono` and adds first-class
//! datetime / duration values. Comparisons and `+` / `-` / `date_diff`
//! all work on them natively.
//!
//! Run:
//!
//!     cargo run --example datetime_ops --features datetime

use datalogic_rs::Engine;

fn main() {
    let engine = Engine::new();

    // ----- parse + format -------------------------------------------
    let r = engine
        .evaluate_str(r#"{"datetime": "2026-05-06T09:00:00Z"}"#, r#"{}"#)
        .unwrap();
    println!("datetime literal -> {r}");

    let r = engine
        .evaluate_str(r#"{"parse_date": ["2026-05-06", "yyyy-MM-dd"]}"#, r#"{}"#)
        .unwrap();
    println!("parse_date       -> {r}");

    let r = engine
        .evaluate_str(
            r#"{"format_date": [{"datetime": "2026-05-06T09:00:00Z"}, "yyyy-MM-dd"]}"#,
            r#"{}"#,
        )
        .unwrap();
    println!("format_date      -> {r}");

    // ----- arithmetic with `timestamp` durations --------------------
    let r = engine
        .evaluate_str(
            r#"{"+": [{"datetime": "2026-05-06T09:00:00Z"}, {"timestamp": "1d"}]}"#,
            r#"{}"#,
        )
        .unwrap();
    println!("\n+1 day           -> {r}");

    let r = engine
        .evaluate_str(
            r#"{"date_diff": [
                {"datetime": "2026-05-08T09:00:00Z"},
                {"datetime": "2026-05-06T09:00:00Z"},
                "days"
            ]}"#,
            r#"{}"#,
        )
        .unwrap();
    println!("date_diff (days) -> {r}");

    // ----- now + comparison -----------------------------------------
    let r = engine.evaluate_str(r#"{"now": []}"#, r#"{}"#).unwrap();
    println!("\nnow              -> {r}");

    // Is the scheduled date in the future?
    let r = engine
        .evaluate_str(
            r#"{">": [{"var": "scheduled"}, {"now": []}]}"#,
            r#"{"scheduled": {"datetime": "2099-01-01T00:00:00Z"}}"#,
        )
        .unwrap();
    println!("scheduled > now  -> {r}");
}
