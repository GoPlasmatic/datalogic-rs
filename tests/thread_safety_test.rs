#[cfg(test)]
mod thread_safety_tests {
    use datalogic_rs::DataLogic;
    use serde_json::json;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_compiled_logic_is_send_sync() {
        // This test verifies at compile time that CompiledLogic implements Send + Sync
        fn assert_send_sync<T: Send + Sync>() {}

        let engine = DataLogic::new();
        let logic = json!({"==": [1, 1]});
        let compiled = engine.compile(&logic).unwrap();

        // This will fail to compile if CompiledLogic doesn't implement Send + Sync
        assert_send_sync::<datalogic_rs::CompiledLogic>();

        // The compile method already returns Arc<CompiledLogic>
        let _arc_compiled = compiled;
    }

    #[test]
    fn test_parallel_evaluation_with_threads() {
        let engine = Arc::new(DataLogic::new());

        // Compile some logic
        let logic = json!({
            "if": [
                {">": [{"var": "score"}, 90]},
                "excellent",
                {"if": [
                    {">": [{"var": "score"}, 70]},
                    "good",
                    "needs improvement"
                ]}
            ]
        });

        let compiled = engine.compile(&logic).unwrap();

        // Test data sets
        let test_cases = vec![
            (json!({"score": 95}), "excellent"),
            (json!({"score": 85}), "good"),
            (json!({"score": 65}), "needs improvement"),
            (json!({"score": 100}), "excellent"),
            (json!({"score": 75}), "good"),
        ];

        // Spawn threads to evaluate in parallel
        let handles: Vec<_> = test_cases
            .into_iter()
            .map(|(data, expected)| {
                let engine = Arc::clone(&engine);
                let compiled = Arc::clone(&compiled);
                let data = Arc::new(data);

                thread::spawn(move || {
                    let result = engine.evaluate(&compiled, data).unwrap();
                    assert_eq!(result.as_str().unwrap(), expected);
                })
            })
            .collect();

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_multiple_engines_in_parallel() {
        // Each thread gets its own engine instance
        let handles: Vec<_> = (0..5)
            .map(|i| {
                thread::spawn(move || {
                    let engine = DataLogic::new();
                    let logic = json!({"+": [{"var": "a"}, {"var": "b"}]});
                    let data = json!({"a": i, "b": i * 2});
                    let data = Arc::new(data);

                    let compiled = engine.compile(&logic).unwrap();
                    let result = engine.evaluate(&compiled, data).unwrap();

                    assert_eq!(result.as_i64().unwrap(), i + i * 2);
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }
    }

    #[test]
    fn test_shared_compiled_logic_across_threads() {
        let engine = DataLogic::new();

        // Complex logic with nested operations
        let logic = json!({
            "map": [
                {"var": "items"},
                {
                    "*": [
                        {"var": ""},
                        2  // Simple multiplication by 2
                    ]
                }
            ]
        });

        let compiled = engine.compile(&logic).unwrap();

        // Different data sets for each thread
        let datasets = vec![
            json!({"items": [1, 2, 3]}),
            json!({"items": [4, 5, 6]}),
            json!({"items": [7, 8, 9]}),
        ];

        let handles: Vec<_> = datasets
            .into_iter()
            .enumerate()
            .map(|(idx, data)| {
                let compiled = Arc::clone(&compiled);
                let data = Arc::new(data);

                thread::spawn(move || {
                    // Each thread creates its own engine
                    let engine = DataLogic::new();
                    let result = engine.evaluate(&compiled, data).unwrap();

                    // Verify results based on thread index (multiply by 2)
                    let arr = result.as_array().unwrap();
                    match idx {
                        0 => assert_eq!(arr, &vec![json!(2), json!(4), json!(6)]),
                        1 => assert_eq!(arr, &vec![json!(8), json!(10), json!(12)]),
                        2 => assert_eq!(arr, &vec![json!(14), json!(16), json!(18)]),
                        _ => panic!("Unexpected index"),
                    }
                })
            })
            .collect();

        for handle in handles {
            handle.join().unwrap();
        }
    }
}

// Async tests with Tokio
#[cfg(test)]
mod async_tests {
    use datalogic_rs::DataLogic;
    use serde_json::json;
    use std::sync::Arc;

    // Note: Add tokio to Cargo.toml dev-dependencies to run these tests
    // [dev-dependencies]
    // tokio = { version = "1", features = ["full"] }

    #[tokio::test]
    async fn test_async_evaluation_with_tokio() {
        let engine = Arc::new(DataLogic::new());

        let logic = json!({
            "filter": [
                {"var": "users"},
                {">": [{"var": "age"}, 18]}
            ]
        });

        let compiled = engine.compile(&logic).unwrap();

        // Spawn multiple async tasks
        let mut tasks = vec![];

        for i in 0..5 {
            let engine = Arc::clone(&engine);
            let compiled = Arc::clone(&compiled);

            let task = tokio::spawn(async move {
                let data = json!({
                    "users": [
                        {"name": format!("User{}", i), "age": 20 + i},
                        {"name": format!("Kid{}", i), "age": 10 + i},
                        {"name": format!("Adult{}", i), "age": 30 + i},
                    ]
                });
                let data = Arc::new(data);

                let result = engine.evaluate(&compiled, data).unwrap();
                let filtered = result.as_array().unwrap();

                // Should filter out the "Kid" entries
                assert_eq!(filtered.len(), 2);
                filtered.len()
            });

            tasks.push(task);
        }

        // Wait for all tasks
        let results = futures::future::join_all(tasks).await;
        for result in results {
            assert_eq!(result.unwrap(), 2);
        }
    }

    #[tokio::test]
    async fn test_concurrent_evaluation_with_shared_engine() {
        // Single shared engine across all tasks
        let engine = Arc::new(DataLogic::new());

        // Multiple different logic patterns
        let logics = vec![
            json!({"==": [1, 1]}),
            json!({"+": [2, 3]}),
            json!({"*": [4, 5]}),
            json!({">": [10, 5]}),
            json!({"and": [true, true]}),
        ];

        let mut tasks = vec![];

        for logic in logics {
            let engine = Arc::clone(&engine);
            let data = Arc::new(json!({})); // Empty data for these tests

            let task = tokio::spawn(async move {
                let compiled = engine.compile(&logic).unwrap();
                let result = engine.evaluate(&compiled, data).unwrap();

                // Return the result for verification
                result
            });

            tasks.push(task);
        }

        let results = futures::future::join_all(tasks).await;

        // Verify expected results
        assert_eq!(results[0].as_ref().unwrap(), &json!(true));
        assert_eq!(results[1].as_ref().unwrap(), &json!(5));
        assert_eq!(results[2].as_ref().unwrap(), &json!(20));
        assert_eq!(results[3].as_ref().unwrap(), &json!(true));
        assert_eq!(results[4].as_ref().unwrap(), &json!(true));
    }

    #[tokio::test]
    async fn test_blocking_evaluation_in_spawn_blocking() {
        let engine = Arc::new(DataLogic::new());

        let logic = json!({
            "reduce": [
                {"var": "numbers"},
                {"+": [{"var": "accumulator"}, {"var": "current"}]},
                0
            ]
        });

        let compiled = engine.compile(&logic).unwrap();

        // CPU-intensive operation in spawn_blocking
        let handle = tokio::task::spawn_blocking(move || {
            let data = json!({
                "numbers": (1..=1000).collect::<Vec<i32>>()
            });
            let data = Arc::new(data);

            engine.evaluate(&compiled, data).unwrap()
        });

        let result = handle.await.unwrap();

        // Sum of 1..=1000 is 500500
        assert_eq!(result.as_i64().unwrap(), 500500);
    }
}
