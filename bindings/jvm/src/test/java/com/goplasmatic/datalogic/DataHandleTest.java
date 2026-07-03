/* SPDX-License-Identifier: Apache-2.0 */
package com.goplasmatic.datalogic;

import org.junit.jupiter.api.Test;

import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.concurrent.Future;

import static org.junit.jupiter.api.Assertions.*;

class DataHandleTest {

    @Test
    void parse_then_evaluate_via_rule_and_session() {
        try (Engine engine = new Engine();
             Rule rule = engine.compile("{\"*\":[{\"var\":\"x\"},2]}");
             Session session = engine.openSession();
             DataHandle data = DataHandle.parse("{\"x\":21}")) {
            // Not consumed by evaluation: use it repeatedly, on both paths.
            assertEquals("42", rule.evaluate(data));
            assertEquals("42", rule.evaluate(data));
            assertEquals("42", session.evaluate(rule, data));
            assertEquals("42", session.evaluate(rule, data));
        }
    }

    @Test
    void handle_is_engine_independent() {
        try (Engine a = new Engine();
             Engine b = new Engine();
             Rule ruleA = a.compile("{\"var\":\"x\"}");
             Rule ruleB = b.compile("{\"+\":[{\"var\":\"x\"},1]}");
             DataHandle data = DataHandle.parse("{\"x\":41}")) {
            assertEquals("41", ruleA.evaluate(data));
            assertEquals("42", ruleB.evaluate(data));
        }
    }

    @Test
    void malformed_json_throws_ParseException() {
        ParseException ex = assertThrows(ParseException.class, () -> DataHandle.parse("not-json{{"));
        assertEquals("ParseError", ex.errorType());
        assertNotNull(ex.getMessage());
    }

    @Test
    void allocated_bytes_is_positive() {
        try (DataHandle data = DataHandle.parse("{\"x\": [1, 2, 3, \"four\"]}")) {
            assertTrue(data.allocatedBytes() > 0);
        }
    }

    @Test
    void closed_handle_throws_IllegalStateException() {
        DataHandle data = DataHandle.parse("{\"x\":1}");
        data.close();
        try (Engine engine = new Engine();
             Rule rule = engine.compile("{\"var\":\"x\"}")) {
            assertThrows(IllegalStateException.class, () -> rule.evaluate(data));
        }
        data.close(); // double-close is a no-op
    }

    @Test
    void one_handle_shared_across_threads() throws Exception {
        try (Engine engine = new Engine();
             Rule rule = engine.compile("{\"*\":[{\"var\":\"x\"},2]}");
             DataHandle data = DataHandle.parse("{\"x\":21}")) {
            int threads = 4;
            ExecutorService pool = Executors.newFixedThreadPool(threads);
            try {
                List<Future<Boolean>> results = new ArrayList<>();
                for (int t = 0; t < threads; t++) {
                    results.add(pool.submit(() -> {
                        // One session per thread; rule + data are shared.
                        try (Session session = engine.openSession()) {
                            for (int i = 0; i < 500; i++) {
                                if (!"42".equals(session.evaluate(rule, data))) return false;
                                if (session.evaluateLong(rule, data) != 42L) return false;
                            }
                        }
                        return true;
                    }));
                }
                for (Future<Boolean> f : results) {
                    assertTrue(f.get(), "worker saw a wrong evaluation result");
                }
            } finally {
                pool.shutdown();
            }
        }
    }
}
