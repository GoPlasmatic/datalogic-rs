//! A `reduce` that wraps its accumulator once per item used to build a
//! value as deep as the list was long. Serialising it then overflowed the
//! stack and aborted the process (100,000 items was enough on an 8 MiB
//! main thread in a release build). The accumulator's nesting is now
//! capped, and passing the cap is an ordinary error.

use datalogic_rs::Engine;

const WRAP: &str = r#"{"reduce": [{"var": "xs"}, [{"var": "accumulator"}], null]}"#;

fn zeros(n: usize) -> String {
    let items = vec!["0"; n].join(",");
    format!(r#"{{"xs": [{items}]}}"#)
}

/// Runs `f` on a 2 MiB thread, the default for spawned Rust threads and
/// smaller than a main thread: an overflow aborts the whole test binary.
fn on_small_stack<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap()
}

#[test]
fn a_long_wrapping_reduce_is_an_error_not_an_abort() {
    on_small_stack(|| {
        let engine = Engine::new();
        let rule = engine.compile(WRAP).unwrap();
        let err = engine
            .session()
            .eval_str(&rule, zeros(200_000).as_str())
            .unwrap_err();
        assert!(err.to_string().contains("nests deeper than"), "{err}");
    });
}

#[test]
fn nesting_up_to_the_cap_still_works() {
    on_small_stack(|| {
        let engine = Engine::new();
        let rule = engine.compile(WRAP).unwrap();
        // 1,000 wraps around null: 1,000 levels, under the 1,024 cap.
        let out = engine
            .session()
            .eval_str(&rule, zeros(1_000).as_str())
            .unwrap();
        assert_eq!(
            out,
            format!("{}null{}", "[".repeat(1_000), "]".repeat(1_000))
        );
        // 1,100 is past it.
        assert!(
            engine
                .session()
                .eval_str(&rule, zeros(1_100).as_str())
                .is_err()
        );
    });
}

#[test]
fn shallow_accumulators_are_unaffected() {
    let engine = Engine::new();
    // Building a flat list and an object over many items stays legal.
    let list = engine
        .compile(r#"{"reduce": [{"var": "xs"}, {"merge": [{"var": "accumulator"}, [{"var": "current"}]]}, []]}"#)
        .unwrap();
    let out = engine
        .session()
        .eval_str(&list, zeros(5_000).as_str())
        .unwrap();
    assert_eq!(out.matches('0').count(), 5_000);

    let sum = engine
        .compile(r#"{"reduce": [{"var": "xs"}, {"+": [{"var": "accumulator"}, 1]}, 0]}"#)
        .unwrap();
    assert_eq!(
        engine
            .session()
            .eval_str(&sum, zeros(50_000).as_str())
            .unwrap(),
        "50000"
    );
}

#[test]
fn reducing_an_object_is_capped_too() {
    on_small_stack(|| {
        let engine = Engine::new();
        let rule = engine
            .compile(r#"{"reduce": [{"var": "obj"}, [{"var": "accumulator"}], null]}"#)
            .unwrap();
        let fields: Vec<String> = (0..5_000).map(|i| format!(r#""k{i}": 0"#)).collect();
        let data = format!(r#"{{"obj": {{{}}}}}"#, fields.join(","));
        assert!(engine.session().eval_str(&rule, data.as_str()).is_err());
    });
}

#[cfg(feature = "templating")]
#[test]
fn a_template_wrapping_the_accumulator_is_capped() {
    on_small_stack(|| {
        let engine = Engine::builder().with_templating(true).build();
        let rule = engine
            .compile(r#"{"reduce": [{"var": "xs"}, {"next": {"var": "accumulator"}, "v": {"var": "current"}}, null]}"#)
            .unwrap();
        assert!(
            engine
                .session()
                .eval_str(&rule, zeros(200_000).as_str())
                .is_err()
        );
        // A short linked list is fine.
        assert!(
            engine
                .session()
                .eval_str(&rule, zeros(500).as_str())
                .is_ok()
        );
    });
}

#[test]
fn a_body_that_wraps_deeply_every_step_is_capped() {
    on_small_stack(|| {
        let engine = Engine::new();
        // Each step wraps the accumulator in 30 arrays. (Kept shallow:
        // evaluating a rule nested ~100 deep already overflows a 2 MiB
        // stack in a debug build.)
        let body = format!(
            "{}{{\"var\": \"accumulator\"}}{}",
            "[".repeat(30),
            "]".repeat(30)
        );
        let rule = engine
            .compile(format!(r#"{{"reduce": [{{"var": "xs"}}, {body}, null]}}"#).as_str())
            .unwrap();
        // 34 steps: 1,020 levels, within the cap.
        assert!(engine.session().eval_str(&rule, zeros(34).as_str()).is_ok());
        // 35 steps: 1,050 levels, past it.
        assert!(
            engine
                .session()
                .eval_str(&rule, zeros(35).as_str())
                .is_err()
        );
        // Far past it: an error, not an abort.
        assert!(
            engine
                .session()
                .eval_str(&rule, zeros(5_000).as_str())
                .is_err()
        );
    });
}

#[cfg(all(feature = "error-handling", feature = "templating"))]
#[test]
fn a_body_routing_the_accumulator_through_try_is_capped() {
    on_small_stack(|| {
        // The thrown object carries the accumulator; the catch arm wraps
        // it again: two levels a step, through an error.
        let engine = Engine::builder().with_templating(true).build();
        let rule = engine
            .compile(r#"{"reduce": [{"var": "xs"}, {"try": [{"throw": {"next": {"var": "accumulator"}}}, [{"var": ""}]]}, null]}"#)
            .unwrap();
        assert!(engine.session().eval_str(&rule, zeros(10).as_str()).is_ok());
        assert!(
            engine
                .session()
                .eval_str(&rule, zeros(100_000).as_str())
                .is_err()
        );
    });
}

/// Wraps its argument in 100 arrays: a host operator can deepen the
/// accumulator by any amount, so a body that calls one is measured every
/// step.
struct Bury;

impl datalogic_rs::CustomOperator for Bury {
    fn evaluate<'a>(
        &self,
        args: &[&'a datalogic_rs::DataValue<'a>],
        _ctx: &mut datalogic_rs::operator::EvalContext<'_, 'a>,
        arena: &'a datalogic_rs::bumpalo::Bump,
    ) -> datalogic_rs::Result<&'a datalogic_rs::DataValue<'a>> {
        let mut v: &datalogic_rs::DataValue<'a> = args[0];
        for _ in 0..100 {
            v = arena.alloc(datalogic_rs::DataValue::Array(
                arena.alloc_slice_copy(&[*v]),
            ));
        }
        Ok(v)
    }
}

#[test]
fn a_body_calling_a_custom_operator_is_measured_every_step() {
    on_small_stack(|| {
        let engine = Engine::builder().add_operator("bury", Bury).build();
        let rule = engine
            .compile(r#"{"reduce": [{"var": "xs"}, {"bury": [{"var": "accumulator"}]}, null]}"#)
            .unwrap();
        // 10 steps: 1,000 levels.
        assert!(engine.session().eval_str(&rule, zeros(10).as_str()).is_ok());
        // 11 steps: 1,100, caught on the step that passes the cap.
        assert!(
            engine
                .session()
                .eval_str(&rule, zeros(11).as_str())
                .is_err()
        );
        assert!(
            engine
                .session()
                .eval_str(&rule, zeros(5_000).as_str())
                .is_err()
        );
    });
}
