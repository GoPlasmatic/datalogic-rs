/* SPDX-License-Identifier: Apache-2.0 */
package com.goplasmatic.datalogic;

import org.junit.jupiter.api.Test;

import java.util.List;

import static org.junit.jupiter.api.Assertions.*;

class BatchTest {

    @Test
    void evaluateBatch_mixes_successes_and_per_item_failures() {
        try (Engine engine = new Engine();
             Rule rule = engine.compile("{\"if\":[{\"var\":\"fail\"},{\"throw\":\"boom\"},{\"var\":\"v\"}]}");
             Session session = engine.openSession();
             DataHandle ok1 = DataHandle.parse("{\"fail\":false,\"v\":1}");
             DataHandle bad = DataHandle.parse("{\"fail\":true}");
             DataHandle ok2 = DataHandle.parse("{\"fail\":false,\"v\":\"x\"}")) {

            List<EvalResult> results = session.evaluateBatch(rule, List.of(ok1, bad, ok2));

            assertEquals(3, results.size());

            assertTrue(results.get(0).isSuccess());
            assertEquals("1", results.get(0).value());

            EvalResult failed = results.get(1);
            assertFalse(failed.isSuccess());
            assertNull(failed.value());
            assertEquals("Thrown", failed.errorTag());
            assertNotNull(failed.errorMessage());
            assertTrue(failed.errorMessage().contains("boom"), "got: " + failed.errorMessage());

            assertTrue(results.get(2).isSuccess());
            assertEquals("\"x\"", results.get(2).value());
        }
    }

    @Test
    void evaluateBatch_empty_list_returns_empty() {
        try (Engine engine = new Engine();
             Rule rule = engine.compile("{\"+\":[1,2]}");
             Session session = engine.openSession()) {
            assertEquals(List.of(), session.evaluateBatch(rule, List.of()));
        }
    }

    @Test
    void evaluateMany_mixes_successes_and_per_item_failures() {
        try (Engine engine = new Engine();
             Rule sum = engine.compile("{\"+\":[1,2]}");
             Rule boom = engine.compile("{\"throw\":\"kaput\"}");
             Rule var = engine.compile("{\"var\":\"a\"}");
             Session session = engine.openSession();
             DataHandle data = DataHandle.parse("{\"a\":7}")) {

            List<EvalResult> results = session.evaluateMany(List.of(sum, boom, var), data);

            assertEquals(3, results.size());
            assertEquals("3", results.get(0).value());
            assertFalse(results.get(1).isSuccess());
            assertEquals("Thrown", results.get(1).errorTag());
            assertTrue(results.get(1).errorMessage().contains("kaput"));
            assertEquals("7", results.get(2).value());
        }
    }

    @Test
    void evaluateMany_flags_rule_from_foreign_engine_per_item() {
        try (Engine engine = new Engine();
             Engine other = new Engine();
             Rule ours = engine.compile("{\"+\":[1,2]}");
             Rule foreign = other.compile("{\"+\":[1,2]}");
             Session session = engine.openSession();
             DataHandle data = DataHandle.parse("{}")) {

            List<EvalResult> results = session.evaluateMany(List.of(ours, foreign), data);

            assertEquals(2, results.size());
            assertTrue(results.get(0).isSuccess());
            assertEquals("3", results.get(0).value());
            assertFalse(results.get(1).isSuccess());
            assertEquals("InvalidArgument", results.get(1).errorTag());
        }
    }

    @Test
    void evaluateMany_empty_list_returns_empty() {
        try (Engine engine = new Engine();
             Session session = engine.openSession();
             DataHandle data = DataHandle.parse("{}")) {
            assertEquals(List.of(), session.evaluateMany(List.of(), data));
        }
    }

    @Test
    void batch_results_survive_subsequent_session_calls() {
        // Results are copied out of the session buffer immediately — a
        // later evaluation must not corrupt previously returned items.
        try (Engine engine = new Engine();
             Rule echo = engine.compile("{\"var\":\"v\"}");
             Session session = engine.openSession();
             DataHandle a = DataHandle.parse("{\"v\":\"first\"}");
             DataHandle b = DataHandle.parse("{\"v\":\"second\"}")) {
            List<EvalResult> results = session.evaluateBatch(echo, List.of(a, b));
            assertEquals("\"second\"", session.evaluate(echo, b));
            assertEquals("\"first\"", results.get(0).value());
            assertEquals("\"second\"", results.get(1).value());
        }
    }
}
