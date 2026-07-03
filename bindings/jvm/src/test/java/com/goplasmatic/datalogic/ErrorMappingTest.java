/* SPDX-License-Identifier: Apache-2.0 */
package com.goplasmatic.datalogic;

import com.fasterxml.jackson.databind.ObjectMapper;
import com.goplasmatic.datalogic.internal.DatalogicNative;
import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.*;

/**
 * C ABI v2 contract checks: the load-time ABI assert, status → exception
 * mapping with structured fields, and byte-exact UTF-8 marshalling
 * (which the JNA-era default-charset marshalling silently corrupted on
 * non-UTF-8 platform charsets).
 */
class ErrorMappingTest {

    private static final ObjectMapper MAPPER = new ObjectMapper();

    @Test
    void loaded_library_reports_abi_v2() {
        assertEquals(2, DatalogicNative.EXPECTED_ABI_VERSION);
        assertEquals(2, DatalogicNative.abiVersion());
    }

    @Test
    void parse_failure_maps_to_ParseException_with_tag() {
        try (Engine engine = new Engine()) {
            ParseException ex = assertThrows(ParseException.class,
                    () -> engine.compile("{\"var\": }"));
            assertEquals("ParseError", ex.errorType());
            assertNotNull(ex.getMessage());
            assertFalse(ex.getMessage().isEmpty());
        }
    }

    @Test
    void eval_failure_carries_operator_and_path() {
        try (Engine engine = new Engine();
             Rule rule = engine.compile("{\"+\":[1,{\"throw\":\"inner\"}]}")) {
            EvaluateException ex = assertThrows(EvaluateException.class, () -> rule.evaluate("{}"));
            assertEquals("Thrown", ex.errorType());
            assertNotNull(ex.getMessage());
            // operatorName() is the OUTERMOST failing operator.
            assertEquals("+", ex.operatorName());
            assertNotNull(ex.pathJson());
            assertTrue(ex.pathJson().startsWith("["), "path should be a JSON array: " + ex.pathJson());
        }
    }

    @Test
    void session_rejects_rule_from_foreign_engine_as_invalid_arg() {
        try (Engine engine = new Engine();
             Engine other = new Engine();
             Rule foreign = other.compile("{\"+\":[1,2]}");
             Session session = engine.openSession()) {
            DatalogicException ex = assertThrows(DatalogicException.class,
                    () -> session.evaluate(foreign, "{}"));
            // INVALID_ARG maps to the base class, not a parse/eval subclass.
            assertEquals(DatalogicException.class, ex.getClass());
            assertEquals("InvalidArgument", ex.errorType());
        }
    }

    @Test
    void non_ascii_round_trip_is_byte_exact() {
        // Broken under JNA's default-charset marshalling; UTF-8 by
        // construction now. Assert the exact JSON output.
        try (Engine engine = new Engine()) {
            String result = engine.apply(
                    "{\"cat\":[\"héllo–\",{\"var\":\"x\"}]}",
                    "{\"x\":\"wörld✓\"}");
            assertEquals("\"héllo–wörld✓\"", result);
        }
    }

    @Test
    void non_ascii_survives_every_boundary_direction() throws Exception {
        // Compile + session + data handle + custom operator: exercises
        // input encoding, borrowed-result decoding, and the upcall path.
        try (Engine engine = Engine.builder()
                .addOperator("wrap", argsJson -> {
                    String first = MAPPER.readTree(argsJson).get(0).asText();
                    return MAPPER.writeValueAsString("«" + first + "»");
                })
                .build();
             Rule rule = engine.compile("{\"wrap\":[{\"var\":\"emoji\"}]}");
             Session session = engine.openSession();
             DataHandle data = DataHandle.parse("{\"emoji\":\"héllo☂️\"}")) {
            String result = session.evaluate(rule, data);
            assertEquals("«héllo☂️»", MAPPER.readTree(result).asText());
        }
    }

    @Test
    void custom_operator_error_message_preserves_non_ascii() {
        try (Engine engine = Engine.builder()
                .addOperator("nope", argsJson -> { throw new RuntimeException("näh–✗"); })
                .build()) {
            EvaluateException ex = assertThrows(EvaluateException.class,
                    () -> engine.apply("{\"nope\":[]}", "{}"));
            assertTrue(ex.getMessage().contains("näh–✗"), "got: " + ex.getMessage());
        }
    }

    @Test
    void closed_engine_throws_IllegalStateException() {
        Engine engine = new Engine();
        engine.close();
        assertThrows(IllegalStateException.class, () -> engine.apply("{\"+\":[1,2]}", "{}"));
    }

    @Test
    void rule_with_custom_operator_survives_engine_close_and_gc() {
        // Rules hold an Arc on the Rust engine: closing the Engine handle
        // must not tear down the upcall stubs a still-open rule dispatches
        // into (they stay reachable through the rule's owner reference).
        Engine engine = Engine.builder()
                .addOperator("triple", argsJson -> {
                    int n = MAPPER.readTree(argsJson).get(0).asInt();
                    return String.valueOf(n * 3);
                })
                .build();
        Rule rule = engine.compile("{\"triple\":[{\"var\":\"n\"}]}");
        engine.close();
        System.gc();
        try {
            assertEquals("42", rule.evaluate("{\"n\":14}"));
        } finally {
            rule.close();
        }
    }
}
