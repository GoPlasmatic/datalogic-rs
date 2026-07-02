/* SPDX-License-Identifier: Apache-2.0 */
package com.goplasmatic.datalogic;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.*;

class EngineTest {
    private static final ObjectMapper MAPPER = new ObjectMapper();

    @Test
    void version_is_non_empty() {
        assertFalse(Engine.version().isEmpty());
    }

    @Test
    void apply_one_shot_returns_json_result() {
        try (Engine engine = new Engine()) {
            assertEquals("3", engine.apply("{\"+\":[1,2]}", "{}"));
        }
    }

    @Test
    void compile_then_evaluate_reuses_rule() {
        try (Engine engine = new Engine();
             Rule rule = engine.compile("{\"var\":\"x\"}")) {
            for (int x : new int[]{1, 7, 42}) {
                assertEquals(String.valueOf(x), rule.evaluate("{\"x\":" + x + "}"));
            }
        }
    }

    @Test
    void session_reuses_arena_across_calls() {
        try (Engine engine = new Engine();
             Rule rule = engine.compile("{\"*\":[{\"var\":\"x\"},2]}");
             Session session = engine.openSession()) {
            for (int x : new int[]{3, 5, 8}) {
                assertEquals(String.valueOf(x * 2), session.evaluate(rule, "{\"x\":" + x + "}"));
            }
            assertTrue(session.allocatedBytes() > 0);
        }
    }

    @Test
    void parse_error_throws_ParseException() {
        try (Engine engine = new Engine()) {
            ParseException ex = assertThrows(ParseException.class, () -> engine.compile("not-json{{"));
            assertEquals("ParseError", ex.errorType());
            assertNotNull(ex.getMessage());
        }
    }

    @Test
    void evaluate_error_throws_with_operator_and_path() {
        try (Engine engine = new Engine();
             Rule rule = engine.compile("{\"throw\":\"boom\"}")) {
            EvaluateException ex = assertThrows(EvaluateException.class, () -> rule.evaluate("{}"));
            assertEquals("Thrown", ex.errorType());
            assertNotNull(ex.pathJson());
            assertTrue(ex.pathJson().startsWith("["));
        }
    }

    @Test
    void templating_engine_constructs() {
        try (Engine engine = new Engine(true)) {
            assertNotNull(engine.apply("{\"+\":[1,1]}", "{}"));
        }
    }

    @Test
    void flagd_sem_ver_operator_is_available() {
        try (Engine engine = new Engine()) {
            assertEquals("true", engine.apply("{\"sem_ver\":[\"1.2.3\",\"<\",\"2.0.0\"]}", "{}"));
        }
    }

    @Test
    void traced_session_returns_result_and_steps() throws Exception {
        try (Engine engine = new Engine();
             TracedSession session = engine.openTracedSession()) {
            TracedRun run = session.evaluate("{\"+\":[{\"var\":\"x\"},1]}", "{\"x\":41}");
            assertTrue(run.isSuccess());
            assertEquals(42, run.result().asInt());
            assertTrue(run.steps().isArray() && run.steps().size() > 0);
            assertTrue(run.expressionTree().isObject());
            assertNull(run.error());
        }
    }

    @Test
    void traced_session_surfaces_runtime_error_in_payload() {
        try (Engine engine = new Engine();
             TracedSession session = engine.openTracedSession()) {
            TracedRun run = session.evaluate("{\"throw\":\"boom\"}", "{}");
            assertFalse(run.isSuccess());
            assertNotNull(run.error());
            assertTrue(run.structuredError().isObject());
        }
    }

    @Test
    void builder_registers_custom_operator() {
        try (Engine engine = Engine.builder()
                .addOperator("double", argsJson -> {
                    JsonNode arr = MAPPER.readTree(argsJson);
                    return String.valueOf(arr.get(0).asInt() * 2);
                })
                .build()) {
            assertEquals("42", engine.apply("{\"double\":[21]}", "{}"));
        }
    }

    @Test
    void builder_custom_operator_error_propagates() {
        try (Engine engine = Engine.builder()
                .addOperator("boom", argsJson -> { throw new RuntimeException("custom-failure"); })
                .build()) {
            EvaluateException ex = assertThrows(EvaluateException.class,
                    () -> engine.apply("{\"boom\":[]}", "{}"));
            assertTrue(ex.getMessage().contains("custom-failure"), "got: " + ex.getMessage());
        }
    }

    @Test
    void builder_set_config_json_strict_preset_takes_effect() {
        // Default config: null coerces to 0 and the sum evaluates.
        try (Engine engine = new Engine()) {
            assertEquals("1", engine.apply("{\"+\":[null,1]}", "{}"));
        }
        // Strict preset: the same rule rejects the non-numeric null.
        try (Engine engine = Engine.builder()
                .setConfigJson("{\"preset\":\"strict\"}")
                .build()) {
            assertThrows(EvaluateException.class, () -> engine.apply("{\"+\":[null,1]}", "{}"));
        }
    }

    @Test
    void builder_set_config_json_rejects_bad_input() {
        // Malformed JSON surfaces the parser's message.
        EvaluateException malformed = assertThrows(EvaluateException.class,
                () -> Engine.builder().setConfigJson("not-json{{"));
        assertEquals("ConfigurationError", malformed.errorType());
        assertNotNull(malformed.getMessage());
        assertFalse(malformed.getMessage().isEmpty());

        // Unknown enum values fail loudly instead of being ignored.
        EvaluateException bogus = assertThrows(EvaluateException.class,
                () -> Engine.builder().setConfigJson("{\"preset\":\"bogus\"}"));
        assertTrue(bogus.getMessage().contains("bogus"), "got: " + bogus.getMessage());
    }

    @Test
    void builder_set_config_json_chains_with_templating() {
        try (Engine engine = Engine.builder()
                .withTemplating(true)
                .setConfigJson("{\"preset\":\"strict\"}")
                .build()) {
            assertEquals("3", engine.apply("{\"+\":[1,2]}", "{}"));
            assertThrows(EvaluateException.class, () -> engine.apply("{\"+\":[null,1]}", "{}"));
        }
    }
}
