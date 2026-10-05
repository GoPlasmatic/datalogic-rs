/* SPDX-License-Identifier: Apache-2.0 */
package com.goplasmatic.datalogic;

import static org.junit.jupiter.api.Assertions.*;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import java.nio.file.Files;
import java.nio.file.Path;
import org.junit.jupiter.api.Test;

/** The ABI v2.1 surface: check, compileChecked, modes, operators, facts, truthy, metering. */
class IntrospectionTest {
    private static final ObjectMapper MAPPER = new ObjectMapper();

    @Test
    void checkReportsProblemsWithPointers() throws Exception {
        try (Engine e = new Engine()) {
            JsonNode diags = MAPPER.readTree(
                    e.check("{\"if\": [true, {\"vr\": \"x\"}, {\"map\": [1]}]}", CompileMode.ENGINE));
            assertEquals("UnknownOperator", diags.get(0).get("code").asText());
            assertEquals("/if/1", diags.get(0).get("pointer").asText());
            assertEquals("ArgumentCount", diags.get(1).get("code").asText());
            assertEquals("[]", e.check("{\"a\": {\"var\": \"x\"}, \"b\": 1}", CompileMode.TEMPLATE));
        }
    }

    @Test
    void compileCheckedCarriesDiagnostics() throws Exception {
        try (Engine e = new Engine()) {
            ParseException ex = assertThrows(ParseException.class,
                    () -> e.compileChecked("{\"if\": [{\"bogus\": 1}, {\"map\": [1]}]}"));
            assertEquals("CompileError", ex.errorType());
            assertEquals(2, MAPPER.readTree(ex.diagnosticsJson()).size());
            try (Rule r = e.compileChecked("{\"+\": [1, {\"var\": \"x\"}]}")) {
                assertEquals("3", r.evaluate("{\"x\": 2}"));
            }
        }
    }

    @Test
    void compileModesAndFacts() throws Exception {
        try (Engine e = new Engine()) {
            String tpl = "{\"user\": {\"var\": \"name\"}, \"n\": 1}";
            assertThrows(DatalogicException.class, () -> e.compile(tpl));
            try (Rule r = e.compileTemplate(tpl)) {
                assertEquals("{\"user\":\"ana\",\"n\":1}", r.evaluate("{\"name\": \"ana\"}"));
                assertEquals("[[\"name\"]]", MAPPER.readTree(r.facts()).get("reads").toString());
            }
            assertThrows(DatalogicException.class, () -> e.compileStrict(tpl));
        }
    }

    @Test
    void operatorsIsTheDocumentedCatalogue() throws Exception {
        try (Engine e = new Engine()) {
            JsonNode docs = MAPPER.readTree(Files.readString(Path.of("../../docs/src/operators/operators.json")));
            assertEquals(docs, MAPPER.readTree(e.operators()));
            assertFalse(e.truthy("{}"));
            assertTrue(e.truthy("{\"a\": 1}"));
        }
    }

    @Test
    void meteredSession() {
        try (Engine e = new Engine();
             Rule r = e.compile("{\"map\": [{\"var\": \"xs\"}, {\"+\": [{\"var\": \"\"}, 1]}]}");
             Session s = e.openSession()) {
            Metered m = s.evaluateMetered(r, "{\"xs\": [1, 2, 3]}", 0);
            assertEquals("[2,3,4]", m.value());
            assertTrue(m.ops() > 0);
            EvaluateException ex = assertThrows(EvaluateException.class,
                    () -> s.evaluateMetered(r, "{\"xs\": [1, 2, 3]}", 2));
            assertEquals("BudgetExceeded", ex.errorType());
        }
    }

    @Test
    void builderEscapeAndStrictNames() {
        CustomOperator one = args -> "1";
        EvaluateException ex = assertThrows(EvaluateException.class,
                () -> Engine.builder().withStrictOperatorNames(true).addOperator("length", one));
        assertEquals("ConfigurationError", ex.errorType());
        try (Engine e = Engine.builder().withTemplating(true).withTemplateKeyEscape('$')
                .withStrictOperatorNames(true).addOperator("uno", one).build()) {
            assertEquals("{\"type\":1,\"k\":2}", e.apply("{\"$type\": {\"uno\": []}, \"k\": 2}", "null"));
        }
    }

    @Test
    void errorsCarryNodeIds() {
        try (Engine e = new Engine()) {
            EvaluateException ex = assertThrows(EvaluateException.class,
                    () -> e.apply("{\"+\": [\"a\", 1]}", "null"));
            assertNotNull(ex.nodeIdsJson());
        }
    }
}
