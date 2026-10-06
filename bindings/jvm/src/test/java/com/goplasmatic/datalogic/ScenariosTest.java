/* SPDX-License-Identifier: Apache-2.0 */
package com.goplasmatic.datalogic;

import static org.junit.jupiter.api.Assertions.*;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.node.ArrayNode;
import com.fasterxml.jackson.databind.node.JsonNodeFactory;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.TreeSet;
import java.util.stream.Stream;
import org.junit.jupiter.api.DynamicTest;
import org.junit.jupiter.api.TestFactory;

/**
 * The cross-binding scenarios in bindings/scenarios/api.json, through the
 * JVM API. Every binding runs the same file (see bindings/BINDINGS.md).
 */
class ScenariosTest {
    private static final ObjectMapper MAPPER = new ObjectMapper();

    /** The call's value, or an error marker. */
    private record Outcome(JsonNode value, String errorType) {}

    private static Engine engineFor(JsonNode c) {
        JsonNode o = c.path("engine");
        EngineBuilder b = Engine.builder();
        if (o.has("families")) {
            List<String> names = new ArrayList<>();
            o.get("families").forEach(n -> names.add(n.asText()));
            b.withFamilies(names.toArray(new String[0]));
        }
        if (o.path("templating").asBoolean(false)) b.withTemplating(true);
        if (o.hasNonNull("template_key_escape")) {
            b.withTemplateKeyEscape(o.get("template_key_escape").asText().codePointAt(0));
        }
        if (o.hasNonNull("config")) b.setConfigJson(o.get("config").toString());
        return b.build();
    }

    private static CompileMode mode(JsonNode c) {
        return switch (c.path("mode").asText("engine")) {
            case "strict" -> CompileMode.STRICT;
            case "template" -> CompileMode.TEMPLATE;
            default -> CompileMode.ENGINE;
        };
    }

    private static Outcome run(JsonNode c) throws Exception {
        try (Engine e = engineFor(c)) {
            String rule = c.path("rule").toString();
            String data = c.path("data").toString();
            switch (c.get("call").asText()) {
                case "check": {
                    ArrayNode pairs = JsonNodeFactory.instance.arrayNode();
                    for (JsonNode d : MAPPER.readTree(e.check(rule, mode(c)))) {
                        pairs.addArray().add(d.get("code")).add(d.get("pointer"));
                    }
                    return new Outcome(pairs, null);
                }
                case "truthy":
                    return new Outcome(JsonNodeFactory.instance.booleanNode(e.truthy(c.get("value").toString())), null);
                case "facts":
                    try (Rule r = e.compile(rule)) {
                        return new Outcome(MAPPER.readTree(r.facts()), null);
                    }
                case "trace":
                    try (TracedSession t = e.openTracedSession()) {
                        TracedRun run = t.evaluate(rule, data, mode(c));
                        TreeSet<String> seen = new TreeSet<>();
                        run.pointers().fields().forEachRemaining(f -> seen.add(f.getValue().asText()));
                        var out = JsonNodeFactory.instance.objectNode();
                        out.set("result", run.result());
                        ArrayNode pointers = out.putArray("pointers");
                        seen.forEach(pointers::add);
                        return new Outcome(out, null);
                    }
                case "metered":
                    try (Rule r = e.compile(rule); Session s = e.openSession()) {
                        return new Outcome(MAPPER.readTree(s.evaluateMetered(r, data, c.get("budget").asLong()).value()), null);
                    }
                default: {
                    Rule r = switch (c.get("call").asText()) {
                        case "compile_template" -> e.compileTemplate(rule);
                        case "compile_strict" -> e.compileStrict(rule);
                        case "compile_checked" -> e.compileChecked(rule);
                        default -> e.compile(rule);
                    };
                    try (r) {
                        return new Outcome(MAPPER.readTree(r.evaluate(data)), null);
                    }
                }
            }
        } catch (DatalogicException ex) {
            return new Outcome(null, ex.errorType());
        }
    }

    @TestFactory
    Stream<DynamicTest> scenarios() throws Exception {
        JsonNode all = MAPPER.readTree(Files.readString(Path.of("../scenarios/api.json")));
        List<DynamicTest> tests = new ArrayList<>();
        for (JsonNode c : all) {
            if (!c.isObject()) continue;
            tests.add(DynamicTest.dynamicTest(c.get("call").asText() + ": " + c.get("description").asText(), () -> {
                Outcome got = run(c);
                if (c.has("error")) {
                    assertEquals(c.get("error").asText(), got.errorType());
                    return;
                }
                assertNull(got.errorType(), "unexpected error");
                if (c.has("diagnostics")) {
                    assertEquals(c.get("diagnostics"), got.value());
                } else if (c.has("trace")) {
                    assertEquals(c.get("trace"), got.value());
                } else if (c.has("facts")) {
                    c.get("facts").fields().forEachRemaining(f -> assertEquals(f.getValue(), got.value().get(f.getKey()), f.getKey()));
                } else {
                    assertEquals(c.get("result"), got.value());
                }
            }));
        }
        assertTrue(tests.size() >= 25);
        return tests.stream();
    }
}
