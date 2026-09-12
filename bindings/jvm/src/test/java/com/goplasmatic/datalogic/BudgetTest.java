/* SPDX-License-Identifier: Apache-2.0 */
package com.goplasmatic.datalogic;

import org.junit.jupiter.api.Test;

import java.util.stream.Collectors;
import java.util.stream.IntStream;

import static org.junit.jupiter.api.Assertions.*;

/**
 * The operation budget reaches this binding through the config wire
 * format alone — there is no per-call budget entry point across the C
 * ABI — so these pin that the key is accepted, that it actually bounds
 * evaluation, and that the failure carries its own error type.
 */
class BudgetTest {
    private static final String RULE = "{\"map\":[{\"var\":\"xs\"},{\"*\":[{\"var\":\"\"},2]}]}";

    /** {@code {"xs":[0,1,...,n-1]}} */
    private static String items(int n) {
        return "{\"xs\":[" + IntStream.range(0, n)
                .mapToObj(Integer::toString)
                .collect(Collectors.joining(",")) + "]}";
    }

    @Test
    void ops_budget_bounds_evaluation() {
        // 100 is comfortably above the three-item run and comfortably
        // below the 200-item one.
        try (Engine engine = Engine.builder().setConfigJson("{\"ops_budget\":100}").build()) {
            assertEquals("[0,2,4]", engine.apply(RULE, items(3)));

            EvaluateException tooBig = assertThrows(EvaluateException.class,
                    () -> engine.apply(RULE, items(200)));
            assertEquals("BudgetExceeded", tooBig.errorType());
            assertTrue(tooBig.getMessage().contains("budget"), "got: " + tooBig.getMessage());
        }
    }

    @Test
    void try_cannot_recover_from_an_exhausted_budget() {
        try (Engine engine = Engine.builder().setConfigJson("{\"ops_budget\":10}").build()) {
            EvaluateException ex = assertThrows(EvaluateException.class,
                    () -> engine.apply("{\"try\":[" + RULE + ",\"fallback\"]}", items(200)));
            assertEquals("BudgetExceeded", ex.errorType());
        }
    }

    @Test
    void a_null_budget_is_unbounded() {
        try (Engine engine = Engine.builder().setConfigJson("{\"ops_budget\":null}").build()) {
            assertTrue(engine.apply(RULE, items(200)).startsWith("[0,2,4,"));
        }
    }

    @Test
    void an_invalid_budget_is_a_configuration_error() {
        for (String bad : new String[]{"0", "-1", "\"many\""}) {
            EvaluateException ex = assertThrows(EvaluateException.class,
                    () -> Engine.builder().setConfigJson("{\"ops_budget\":" + bad + "}"),
                    "budget " + bad + " should be rejected");
            assertEquals("ConfigurationError", ex.errorType());
        }
    }

    @Test
    void tensor_operators_are_priced_by_the_elements_they_move() {
        // `zeros` allocates 256 elements from a three-node rule: the node
        // count alone would price this at nothing.
        try (Engine engine = Engine.builder().setConfigJson("{\"ops_budget\":100}").build()) {
            EvaluateException ex = assertThrows(EvaluateException.class,
                    () -> engine.apply("{\"zeros\":[[16,16],\"f32\"]}", "{}"));
            assertEquals("BudgetExceeded", ex.errorType());
        }
    }

    /**
     * The tensor family crosses this binding as JSON like any other
     * value — the tagged form — so no FFI change was needed for it.
     */
    @Test
    void tensor_round_trips_as_tagged_json() {
        try (Engine engine = new Engine()) {
            String emitted = engine.apply("{\"tensor\":[[1,2,3],\"u8\"]}", "{}");
            assertEquals("{\"tensor\":{\"dtype\":\"u8\",\"shape\":[3],\"data\":\"AQID\"}}", emitted);
            // And the emitted form is accepted back as a rule.
            assertEquals("[1,2,3]", engine.apply("{\"to_list\":[" + emitted + "]}", "{}"));
        }
    }
}
