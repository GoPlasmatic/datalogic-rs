# Integration: Spring Boot (JVM)

This guide wires [`io.github.goplasmatic:datalogic`](../jvm.md) into a
Spring Boot service: **the engine as a singleton bean, rules compiled
once and cached, evaluation per request**. The binding uses the Java 22+
Foreign Function & Memory API (no JNI/JNA), and every native type is
`AutoCloseable`.

The running example: eligibility rules stored as JSONLogic in a
database column, changeable by ops without a deployment.

## Dependencies

```xml
<dependency>
  <groupId>io.github.goplasmatic</groupId>
  <artifactId>datalogic</artifactId>
  <version>5.1.0</version>
</dependency>
```

Native libraries are bundled in the jar per platform. JDK 22+ is
required; run with native access enabled:

```
--enable-native-access=ALL-UNNAMED
```

(In `application.properties`-land this usually means adding it to your
launch script or `JAVA_TOOL_OPTIONS`; Boot itself needs nothing else.)

## The engine as a bean

`Engine` and compiled `Rule` objects are thread-safe: build once,
share across the whole application. Sessions are the per-thread tier;
you don't need them until you have a measured hot loop.

```java
@Configuration
public class DatalogicConfig {

    @Bean(destroyMethod = "close")
    public Engine datalogicEngine() {
        return new Engine();
    }
}
```

## A rule cache keyed by version

Compilation is the expensive step (microseconds; pay it once per rule
version, not per request):

```java
@Service
public class RuleService {

    private final Engine engine;
    private final ConcurrentHashMap<String, Rule> cache = new ConcurrentHashMap<>();

    public RuleService(Engine engine) {
        this.engine = engine;
    }

    /** row.id() + row.version() identify one immutable rule body. */
    public Rule compiled(RuleRow row) {
        return cache.computeIfAbsent(
            row.id() + "@" + row.version(),
            key -> engine.compile(row.logic()));
    }
}
```

Compiled rules are shared safely across request threads. If rule churn
is high, evict old versions (Caffeine or a bounded LinkedHashMap) and
`close()` evicted rules to release their native handles promptly.

## The endpoint

```java
@RestController
public class EligibilityController {

    private final RuleService rules;
    private final RuleRepository repo;

    EligibilityController(RuleService rules, RuleRepository repo) {
        this.rules = rules;
        this.repo = repo;
    }

    @PostMapping("/eligibility")
    public String check(@RequestBody String applicantJson) {
        Rule rule = rules.compiled(repo.activeEligibilityRule());
        return rule.evaluate(applicantJson);   // JSON in, JSON out
    }
}
```

The JVM surface is JSON-string in/out, which composes naturally with
Spring endpoints that already hold the request body as JSON. If you're
mapping through Jackson anyway, serialize once and reuse: for payloads
evaluated repeatedly, parse once into a `DataHandle` (immutable,
thread-safe) and use the handle-based evaluations to skip the per-call
parse.

## Validating rules at ingestion

Treat rule ingestion as untrusted input: bound the size, compile, and
run golden tests before activating.

```java
@PostMapping("/rules")
public ResponseEntity<?> saveRule(@RequestBody @Size(max = 65_536) String logic) {
    try (Rule candidate = engine.compile(logic)) {
        for (GoldenCase c : goldenCases) {
            if (!candidate.evaluate(c.input()).equals(c.expected())) {
                return ResponseEntity.unprocessableEntity()
                        .body("rule fails golden case: " + c.name());
            }
        }
    } catch (DatalogicException e) {
        return ResponseEntity.unprocessableEntity().body(e.getMessage());
    }
    // persist with a bumped version...
    return ResponseEntity.noContent().build();
}
```

Evaluation itself is sandboxed (rules have no I/O and can only read
the data document you pass), so ingestion bounds (size limits, golden
tests) are where your review effort belongs.

## Hot paths: sessions and batch

For a measured hot loop (scoring a stream, evaluating a rule set per
message), open a `Session` per worker thread (it reuses one native
arena across calls) and use the typed (`evaluateBoolean`-style) and
batch entry points to skip JSON result parsing. Patterns and the full
tier table are in the [JVM chapter](../jvm.md).

## One more thing polyglot teams get for free

The rule your Spring service enforces is the same rule (same bytes,
same semantics, same conformance battery) that your React admin UI
can render and step-debug with the
[visual editor](../react-ui/installation.md), and that a Node or Python
service can evaluate with its own binding. One engine, no drift.
