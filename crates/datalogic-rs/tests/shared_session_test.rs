//! `SharedSession`: a session that holds its engine by `Arc`, so it is
//! `'static + Send` and can live in a struct, a thread or an async task
//! without borrowing the engine.

use std::sync::Arc;

use datalogic_rs::{Engine, Session, SharedSession};
use datavalue::OwnedDataValue;

fn assert_static_send<T: 'static + Send>() {}

#[test]
fn a_shared_session_is_static_and_send() {
    assert_static_send::<SharedSession>();
}

#[test]
fn it_evaluates_like_a_borrowed_session() {
    let engine = Arc::new(Engine::new());
    let logic = engine.compile(r#"{"+": [{"var": "x"}, 1]}"#).unwrap();
    let mut shared = SharedSession::new(engine.clone());
    let mut borrowed = engine.session();
    for x in 0..3 {
        let data = format!(r#"{{"x": {x}}}"#);
        assert_eq!(
            shared.eval_str(&logic, data.as_str()).unwrap(),
            borrowed.eval_str(&logic, data.as_str()).unwrap()
        );
        shared.reset();
    }
}

#[test]
fn it_keeps_the_engine_alive() {
    let engine = Arc::new(Engine::new());
    let logic = engine.compile(r#"{"cat": ["a", {"var": "s"}]}"#).unwrap();
    let mut session = SharedSession::from(engine.clone());
    assert_eq!(Arc::strong_count(&engine), 2);
    drop(engine);
    assert_eq!(
        session.eval_str(&logic, r#"{"s": "b"}"#).unwrap(),
        r#""ab""#
    );
    assert!(session.engine().builtin_operator_names().count() > 0);
}

#[test]
fn it_moves_to_another_thread() {
    let engine = Arc::new(Engine::new());
    let logic = Arc::new(engine.compile(r#"{"*": [{"var": "n"}, 2]}"#).unwrap());
    let mut session = SharedSession::new(engine);
    let handle = std::thread::spawn(move || {
        let data = OwnedDataValue::from_json(r#"{"n": 21}"#).unwrap();
        session.eval(&logic, &data).unwrap()
    });
    assert_eq!(handle.join().unwrap(), OwnedDataValue::from(42i64));
}

#[test]
fn it_lives_in_a_struct_without_a_lifetime() {
    struct Worker {
        session: SharedSession,
        logic: datalogic_rs::Logic,
    }
    impl Worker {
        fn run(&mut self, n: i64) -> String {
            let out = self
                .session
                .eval_str(&self.logic, format!(r#"{{"n": {n}}}"#).as_str())
                .unwrap();
            self.session.reset();
            out
        }
    }
    let engine = Arc::new(Engine::new());
    let logic = engine.compile(r#"{">": [{"var": "n"}, 2]}"#).unwrap();
    let mut worker = Worker {
        session: SharedSession::new(engine),
        logic,
    };
    assert_eq!(worker.run(3), "true");
    assert_eq!(worker.run(1), "false");
}

#[test]
fn borrowed_results_and_arena_controls_work() {
    let engine = Arc::new(Engine::new());
    let logic = engine.compile(r#"{"merge": [[1], [2]]}"#).unwrap();
    let mut session = SharedSession::new(engine);
    let data = OwnedDataValue::Null;
    {
        let v = session.eval_borrowed(&logic, &data).unwrap();
        assert_eq!(v.as_array().map(|a| a.len()), Some(2));
    }
    assert!(session.allocated_bytes() > 0);
    session.reset_with_capacity(1 << 12);
    let typed: String = session
        .eval_as(&engine_compile(r#"{"+": [1, 2]}"#), &data)
        .unwrap();
    assert_eq!(typed, "3");
}

fn engine_compile(rule: &str) -> datalogic_rs::Logic {
    Engine::new().compile(rule).unwrap()
}

/// The borrowed form is unchanged: `Session<'_>` still names it.
#[test]
fn the_borrowed_session_type_is_unchanged() {
    let engine = Engine::new();
    let logic = engine.compile(r#"{"var": "x"}"#).unwrap();
    let mut session: Session<'_> = engine.session();
    assert_eq!(session.eval_str(&logic, r#"{"x": 5}"#).unwrap(), "5");
}

#[test]
fn it_runs_in_an_async_task() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .build()
        .unwrap();
    let engine = Arc::new(Engine::new());
    let logic = Arc::new(engine.compile(r#"{"var": "x"}"#).unwrap());
    let out = rt.block_on(async move {
        let mut session = SharedSession::new(engine);
        tokio::spawn(async move {
            tokio::task::yield_now().await;
            session.eval_str(&logic, r#"{"x": "ok"}"#).unwrap()
        })
        .await
        .unwrap()
    });
    assert_eq!(out, r#""ok""#);
}
