//! Tensor feature tests that the JSON suites cannot express.
//!
//! The suites in `suites/tensor/` cover operator semantics. This file
//! covers the things that need Rust: the typed error chain, the value's
//! behaviour at the JSON boundary, and the dtype coverage split between
//! the byte-moving and element-wise operators.

#![cfg(all(feature = "tensor", feature = "serde_json"))]

use datalogic_rs::{Engine, datavalue::TensorError};

fn eval(rule: &str) -> String {
    Engine::new().eval_str(rule, "{}").expect("eval")
}

fn eval_err(rule: &str) -> datalogic_rs::Error {
    Engine::new()
        .eval_str(rule, "{}")
        .expect_err("expected error")
}

/// The `TensorError` a failure came from, recovered by walking the source
/// chain rather than by matching on a rendered message.
fn tensor_error(rule: &str) -> TensorError {
    let err = eval_err(rule);
    let mut source: Option<&(dyn std::error::Error + 'static)> = std::error::Error::source(&err);
    while let Some(e) = source {
        if let Some(te) = e.downcast_ref::<TensorError>() {
            return te.clone();
        }
        source = std::error::Error::source(e);
    }
    panic!("no TensorError in the source chain of {err:?}");
}

// ---------------------------------------------------------------------------
// The typed error chain
// ---------------------------------------------------------------------------

#[test]
fn unknown_dtype_names_the_spelling_it_was_given() {
    assert_eq!(
        tensor_error(r#"{"zeros": [[2], "float32"]}"#),
        TensorError::UnknownDType("float32".to_string())
    );
}

#[test]
fn an_element_that_does_not_fit_is_rejected_not_truncated() {
    // 256 has no u8; datavalue reports which element and which dtype.
    assert!(matches!(
        tensor_error(r#"{"tensor": [[1, 256], "u8"]}"#),
        TensorError::Element {
            expected: datalogic_rs::datavalue::DType::U8,
            ..
        }
    ));
}

#[test]
fn ragged_nested_arrays_report_their_depth() {
    assert!(matches!(
        tensor_error(r#"{"tensor": [[[1, 2], [3]], "u8"]}"#),
        TensorError::Ragged { .. }
    ));
}

#[test]
fn a_shape_that_overflows_is_refused_before_allocating() {
    // Two dimensions whose product overflows usize. The point is that
    // this answers in constant time rather than trying to allocate.
    let rule = r#"{"zeros": [[9223372036854775807, 4], "u8"]}"#;
    assert_eq!(tensor_error(rule), TensorError::ShapeOverflow);
}

#[test]
fn rank_is_capped() {
    let dims = (0..300).map(|_| "1").collect::<Vec<_>>().join(",");
    assert!(matches!(
        tensor_error(&format!(r#"{{"zeros": [[{dims}], "u8"]}}"#)),
        TensorError::RankTooHigh { .. }
    ));
}

#[test]
fn the_tagged_decoder_stays_strict_about_unknown_fields() {
    // The guard that matters is at the data boundary, where a producer
    // could hand us a body carrying a field this version does not
    // understand — `strides` would change the meaning of `data`, so it is
    // rejected rather than ignored.
    let engine = Engine::new();
    let data = r#"{"t": {"tensor": {"dtype": "u8", "shape": [1], "data": "AA==",
                                    "strides": [1]}}}"#;
    let err = engine
        .eval_str(r#"{"tensor": [{"val": "t"}]}"#, data)
        .expect_err("expected a decode error");
    let mut source: Option<&(dyn std::error::Error + 'static)> = std::error::Error::source(&err);
    let mut found = None;
    while let Some(e) = source {
        if let Some(te) = e.downcast_ref::<TensorError>() {
            found = Some(te.clone());
            break;
        }
        source = std::error::Error::source(e);
    }
    assert!(
        matches!(found, Some(TensorError::UnexpectedField(_))),
        "got {found:?}"
    );
}

#[test]
fn only_the_emitters_exact_shape_is_read_as_a_wire_body_in_a_rule() {
    // `{"tensor": <object>}` compiles that object as a literal body only
    // when it is exactly what the serializer writes. Any other object
    // keeps its ordinary meaning as a nested rule, which is what lets a
    // rule read a tensor out of its data.
    let engine = Engine::new();
    let data = r#"{"t": {"tensor": {"dtype": "u8", "shape": [2], "data": "AQI="}}}"#;
    assert_eq!(
        engine
            .eval_str(r#"{"shape": [{"tensor": {"val": "t"}}]}"#, data)
            .expect("a single-key rule object still evaluates as a rule"),
        "[2]"
    );
    // And the literal path really is the literal path: no data needed.
    assert_eq!(
        eval(r#"{"shape": [{"tensor": {"dtype": "u8", "shape": [2], "data": "AQI="}}]}"#),
        "[2]"
    );
}

// ---------------------------------------------------------------------------
// The JSON boundary: a tensor never escapes as null
// ---------------------------------------------------------------------------

#[test]
fn a_tensor_renders_as_the_tagged_form() {
    assert_eq!(
        eval(r#"{"tensor": [[1, 2], "u8"]}"#),
        r#"{"tensor":{"dtype":"u8","shape":[2],"data":"AQI="}}"#
    );
}

#[test]
fn a_tensor_nested_in_a_result_still_renders() {
    // Inside an array — a composite arm that could have dropped it.
    assert_eq!(
        eval(r#"[{"tensor": [[1], "u8"]}]"#),
        r#"[{"tensor":{"dtype":"u8","shape":[1],"data":"AQ=="}}]"#
    );
}

/// The object-field half of the case above. Split out and gated: a
/// template is only a template with the `templating` feature on, and
/// without the split this file failed under `--features tensor,serde_json`.
#[cfg(feature = "templating")]
#[test]
fn a_tensor_in_a_template_field_still_renders() {
    let templated = Engine::builder()
        .with_templating(true)
        .build()
        .eval_str(r#"{"out": {"tensor": [[1], "u8"]}}"#, "{}")
        .expect("eval");
    assert_eq!(
        templated,
        r#"{"out":{"tensor":{"dtype":"u8","shape":[1],"data":"AQ=="}}}"#
    );
}

#[test]
fn the_emitted_form_evaluates_back_to_the_same_tensor() {
    let once = eval(r#"{"tensor": [[[1, 2], [3, 4]], "f32"]}"#);
    // Feed the emitted text straight back in as a rule.
    let twice = Engine::new().eval_str(&once, "{}").expect("eval");
    assert_eq!(once, twice);
}

#[test]
fn a_tagged_tensor_in_the_data_decodes() {
    let engine = Engine::new();
    let data = r#"{"t": {"tensor": {"dtype": "u8", "shape": [2], "data": "AQI="}}}"#;
    assert_eq!(
        engine
            .eval_str(r#"{"to_list": [{"tensor": [{"val": "t"}]}]}"#, data)
            .expect("eval"),
        "[1,2]"
    );
}

// ---------------------------------------------------------------------------
// Behaviour as a value
// ---------------------------------------------------------------------------

#[test]
fn cross_type_equality_follows_the_loose_equality_config() {
    let rule = r#"{"==": [{"tensor": [[1], "u8"]}, 1]}"#;
    // Default config raises on incompatible operands, as it does for an
    // object compared with a number.
    assert!(Engine::new().eval_str(rule, "{}").is_err());
    // With the check off, incompatible means false rather than an error.
    let lenient = Engine::builder()
        .with_config(datalogic_rs::EvaluationConfig::default().with_loose_equality_errors(false))
        .build();
    assert_eq!(lenient.eval_str(rule, "{}").expect("eval"), "false");
}

#[test]
fn strict_equality_is_structural_without_any_coercion() {
    assert_eq!(
        eval(r#"{"===": [{"tensor": [[1, 2], "u8"]}, {"tensor": [[1, 2], "u8"]}]}"#),
        "true"
    );
    assert_eq!(
        eval(r#"{"===": [{"tensor": [[1], "u8"]}, {"tensor": [[1], "i8"]}]}"#),
        "false"
    );
}

#[test]
fn a_tensor_is_never_coerced_to_a_number() {
    // A one-element array coerces; a one-element tensor deliberately does
    // not, so arithmetic on a tensor is an error rather than a surprise.
    assert!(
        Engine::new()
            .eval_str(r#"{"+": [{"tensor": [[1], "u8"]}, 1]}"#, "{}")
            .is_err()
    );
}

// ---------------------------------------------------------------------------
// dtype coverage
// ---------------------------------------------------------------------------

#[test]
fn byte_moving_operators_work_on_every_dtype() {
    // No `Element` impl is needed to move f16 cells around, so these work
    // whether or not `tensor-half` is on.
    for dt in ["f16", "bf16", "f32", "u8", "bool"] {
        let src = format!(r#"{{"zeros": [[2, 3], "{dt}"]}}"#);
        assert_eq!(
            eval(&format!(r#"{{"shape": [{{"transpose": [{src}]}}]}}"#)),
            "[3,2]",
            "transpose {dt}"
        );
        assert_eq!(
            eval(&format!(r#"{{"shape": [{{"reshape": [{src}, [6]]}}]}}"#)),
            "[6]",
            "reshape {dt}"
        );
        assert_eq!(
            eval(&format!(
                r#"{{"shape": [{{"pad": [{src}, [1, 0], [0, 1]]}}]}}"#
            )),
            "[3,4]",
            "pad {dt}"
        );
    }
}

#[test]
#[cfg(not(feature = "tensor-half"))]
fn element_operators_refuse_half_without_the_feature() {
    assert_eq!(
        tensor_error(r#"{"to_list": [{"zeros": [[1], "f16"]}]}"#),
        TensorError::UnsupportedDType(datalogic_rs::datavalue::DType::F16)
    );
}

#[test]
#[cfg(feature = "tensor-half")]
fn tensor_half_lifts_the_element_restriction() {
    assert_eq!(
        eval(r#"{"to_list": [{"full": [[2], "f16", 1.5]}]}"#),
        "[1.5,1.5]"
    );
    assert_eq!(
        eval(r#"{"to_list": [{"cast": [{"tensor": [[1.5], "f32"]}, "bf16"]}]}"#),
        "[1.5]"
    );
}

// ---------------------------------------------------------------------------
// The compiler treats the family as runtime-only
// ---------------------------------------------------------------------------

#[test]
fn a_constant_tensor_expression_is_not_folded_into_the_rule() {
    // If `zeros` were folded, the compiled rule would carry a 4 MB
    // literal. Compiling must stay cheap; only evaluating pays.
    let engine = Engine::new();
    let big = r#"{"shape": [{"zeros": [[1024, 1024], "f32"]}]}"#;
    let started = std::time::Instant::now();
    let logic = engine.compile(big).expect("compile");
    let compile_time = started.elapsed();
    assert!(
        compile_time < std::time::Duration::from_millis(50),
        "compiling a constant tensor took {compile_time:?}, which suggests it was folded"
    );
    let arena = bumpalo::Bump::new();
    let out = engine.evaluate(&logic, "{}", &arena).expect("eval");
    assert_eq!(out.to_string(), "[1024,1024]");
}

#[test]
fn repeated_identical_tensor_subtrees_each_evaluate() {
    // CSE must not memoize the family. Both branches are the same rule,
    // and both have to produce the same answer.
    assert_eq!(
        eval(
            r#"{"==": [{"shape": [{"zeros": [[3], "u8"]}]}, {"shape": [{"zeros": [[3], "u8"]}]}]}"#
        ),
        "true"
    );
}
