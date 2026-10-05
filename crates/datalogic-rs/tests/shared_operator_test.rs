//! `Arc<T>` implements `CustomOperator`, so one operator instance can be
//! registered on several engines (a host rebuilding its engine on hot
//! reload) without a wrapper type.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use datalogic_rs::{CustomOperator, DataValue, Engine, Result, operator::EvalContext};

/// Counts its calls, so a test can tell whether two engines share one
/// instance.
#[derive(Default)]
struct Counting {
    calls: AtomicUsize,
}

impl CustomOperator for Counting {
    fn evaluate<'a>(
        &self,
        _args: &[&'a DataValue<'a>],
        _ctx: &mut EvalContext<'_, 'a>,
        arena: &'a datalogic_rs::bumpalo::Bump,
    ) -> Result<&'a DataValue<'a>> {
        let n = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        Ok(arena.alloc(DataValue::from_f64(n as f64)))
    }
}

#[test]
fn one_instance_on_two_engines() {
    let op = Arc::new(Counting::default());
    let first = Engine::builder()
        .add_operator("count", Arc::clone(&op))
        .build();
    let second = Engine::builder()
        .add_operator("count", Arc::clone(&op))
        .build();

    assert_eq!(first.eval_str(r#"{"count": []}"#, "null").unwrap(), "1");
    assert_eq!(second.eval_str(r#"{"count": []}"#, "null").unwrap(), "2");
    assert_eq!(first.eval_str(r#"{"count": []}"#, "null").unwrap(), "3");
    assert_eq!(op.calls.load(Ordering::SeqCst), 3);

    // The engines hold the only other references; dropping them leaves ours.
    drop((first, second));
    assert_eq!(Arc::strong_count(&op), 1);
}

/// A registry of trait objects, the shape a host keeps across reloads.
#[test]
fn shared_trait_objects() {
    let registry: Vec<(&str, Arc<dyn CustomOperator>)> = vec![
        ("count", Arc::new(Counting::default())),
        ("again", Arc::new(Counting::default())),
    ];
    let build = || {
        registry
            .iter()
            .fold(Engine::builder(), |b, (name, op)| {
                b.add_operator(*name, Arc::clone(op))
            })
            .build()
    };
    let before = build();
    let after = build();
    assert_eq!(before.eval_str(r#"{"count": []}"#, "null").unwrap(), "1");
    assert_eq!(after.eval_str(r#"{"count": []}"#, "null").unwrap(), "2");
    assert_eq!(after.eval_str(r#"{"again": []}"#, "null").unwrap(), "1");
}

#[test]
fn try_add_operator_takes_an_arc() {
    let op: Arc<dyn CustomOperator> = Arc::new(Counting::default());
    let engine = Engine::builder()
        .try_add_operator("count", Arc::clone(&op))
        .unwrap()
        .build();
    assert_eq!(engine.eval_str(r#"{"count": []}"#, "null").unwrap(), "1");
    assert!(
        Engine::builder()
            .try_add_operator("if", Arc::clone(&op))
            .is_err()
    );
}
