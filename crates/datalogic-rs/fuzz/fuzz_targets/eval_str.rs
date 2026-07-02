//! Coverage-guided fuzzing of the string-in/string-out boundary.
//!
//! The harness feeds arbitrary (mode, rule, data) triples straight into
//! `Engine::eval_str`. Errors are the expected outcome for garbage input;
//! the crash condition is a panic, abort, or stack overflow anywhere in
//! parse, compile, optimize, or evaluate. Complements the bounded
//! proptest generator in `tests/property_test.rs`: libFuzzer mutates
//! bytes with coverage feedback, so it explores parser corners the
//! structured generator never produces.
//!
//! Run (needs nightly + cargo-fuzz):
//! ```text
//! cd crates/datalogic-rs && cargo +nightly fuzz run eval_str
//! ```
#![no_main]

use std::sync::OnceLock;

use libfuzzer_sys::fuzz_target;

fn plain() -> &'static datalogic_rs::Engine {
    static ENGINE: OnceLock<datalogic_rs::Engine> = OnceLock::new();
    ENGINE.get_or_init(datalogic_rs::Engine::new)
}

fn templating() -> &'static datalogic_rs::Engine {
    static ENGINE: OnceLock<datalogic_rs::Engine> = OnceLock::new();
    ENGINE.get_or_init(|| {
        datalogic_rs::Engine::builder()
            .with_templating(true)
            .build()
    })
}

fuzz_target!(|input: (bool, &str, &str)| {
    let (templated, rule, data) = input;
    let engine = if templated { templating() } else { plain() };
    let _ = engine.eval_str(rule, data);
});
