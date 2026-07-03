/* SPDX-License-Identifier: Apache-2.0 */
package com.goplasmatic.datalogic;

import org.junit.jupiter.api.Test;

import static org.junit.jupiter.api.Assertions.*;

class TypedEvalTest {

    @Test
    void evaluateBool_strict_boolean() {
        try (Engine engine = new Engine();
             Rule rule = engine.compile("{\">\":[{\"var\":\"x\"},10]}");
             Session session = engine.openSession();
             DataHandle hi = DataHandle.parse("{\"x\":11}");
             DataHandle lo = DataHandle.parse("{\"x\":9}")) {
            assertTrue(session.evaluateBool(rule, hi));
            assertFalse(session.evaluateBool(rule, lo));
        }
    }

    @Test
    void evaluateBool_mismatch_on_number_result() {
        try (Engine engine = new Engine();
             Rule rule = engine.compile("{\"+\":[1,2]}");
             Session session = engine.openSession();
             DataHandle data = DataHandle.parse("{}")) {
            EvaluateException ex = assertThrows(EvaluateException.class,
                    () -> session.evaluateBool(rule, data));
            assertEquals("TypeMismatch", ex.errorType());
            assertTrue(ex.getMessage().contains("not a boolean"), "got: " + ex.getMessage());
        }
    }

    @Test
    void evaluateLong_exact_integer() {
        try (Engine engine = new Engine();
             Rule rule = engine.compile("{\"+\":[{\"var\":\"x\"},1]}");
             Session session = engine.openSession();
             DataHandle data = DataHandle.parse("{\"x\":41}")) {
            assertEquals(42L, session.evaluateLong(rule, data));
        }
    }

    @Test
    void evaluateLong_mismatch_on_fractional_result() {
        try (Engine engine = new Engine();
             Rule rule = engine.compile("{\"+\":[1,0.5]}");
             Session session = engine.openSession();
             DataHandle data = DataHandle.parse("{}")) {
            EvaluateException ex = assertThrows(EvaluateException.class,
                    () -> session.evaluateLong(rule, data));
            assertEquals("TypeMismatch", ex.errorType());
        }
    }

    @Test
    void evaluateDouble_accepts_any_number() {
        try (Engine engine = new Engine();
             Rule intRule = engine.compile("{\"+\":[1,2]}");
             Rule fracRule = engine.compile("{\"/\":[3,2]}");
             Session session = engine.openSession();
             DataHandle data = DataHandle.parse("{}")) {
            assertEquals(3.0, session.evaluateDouble(intRule, data));
            assertEquals(1.5, session.evaluateDouble(fracRule, data));
        }
    }

    @Test
    void evaluateDouble_mismatch_on_string_result() {
        try (Engine engine = new Engine();
             Rule rule = engine.compile("{\"cat\":[\"a\",\"b\"]}");
             Session session = engine.openSession();
             DataHandle data = DataHandle.parse("{}")) {
            EvaluateException ex = assertThrows(EvaluateException.class,
                    () -> session.evaluateDouble(rule, data));
            assertEquals("TypeMismatch", ex.errorType());
            assertTrue(ex.getMessage().contains("not a number"), "got: " + ex.getMessage());
        }
    }

    @Test
    void evaluateTruthy_never_mismatches() {
        try (Engine engine = new Engine();
             Rule rule = engine.compile("{\"var\":\"v\"}");
             Session session = engine.openSession();
             DataHandle zero = DataHandle.parse("{\"v\":0}");
             DataHandle str = DataHandle.parse("{\"v\":\"x\"}");
             DataHandle emptyStr = DataHandle.parse("{\"v\":\"\"}")) {
            assertFalse(session.evaluateTruthy(rule, zero));
            assertTrue(session.evaluateTruthy(rule, str));
            assertFalse(session.evaluateTruthy(rule, emptyStr));
        }
    }

    @Test
    void typed_eval_propagates_evaluation_errors() {
        try (Engine engine = new Engine();
             Rule rule = engine.compile("{\"throw\":\"boom\"}");
             Session session = engine.openSession();
             DataHandle data = DataHandle.parse("{}")) {
            EvaluateException ex = assertThrows(EvaluateException.class,
                    () -> session.evaluateBool(rule, data));
            assertEquals("Thrown", ex.errorType());
        }
    }
}
