# io.github.goplasmatic:datalogic

[![Maven Central](https://img.shields.io/maven-central/v/io.github.goplasmatic/datalogic)](https://central.sonatype.com/artifact/io.github.goplasmatic/datalogic)
[![CI](https://github.com/GoPlasmatic/datalogic-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/GoPlasmatic/datalogic-rs/actions/workflows/ci.yml)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

Part of [datalogic-rs](https://github.com/GoPlasmatic/datalogic-rs) — one engine, every runtime.

Java bindings for [datalogic-rs](https://github.com/GoPlasmatic/datalogic-rs),
the JSONLogic rules engine with one Rust core and official bindings for
Rust, Node.js, the browser (WASM), Python, Go, Java, .NET, and PHP. Same
rules, same semantics: every binding runs the same core and passes the
same 1,565-case conformance battery (54 suites). Compile once, evaluate
many, natively in Java.

For the cross-runtime overview and the API-tier model every binding
implements, see the
[repo README](https://github.com/GoPlasmatic/datalogic-rs#readme).

> **New in v5.** This package is new: there is no v4 Java artifact. If
> you are coming from the v4 Rust crate or the v4
> `@goplasmatic/datalogic` WASM package, the engine's v4 → v5 changes
> are catalogued in
> [MIGRATION.md](https://github.com/GoPlasmatic/datalogic-rs/blob/main/MIGRATION.md).

## Install

```xml
<dependency>
    <groupId>io.github.goplasmatic</groupId>
    <artifactId>datalogic</artifactId>
    <version>5.0.1</version>
</dependency>
```

Gradle: `implementation("io.github.goplasmatic:datalogic:5.0.1")`

The binding speaks to the engine's C ABI directly through the Java FFM
API (`java.lang.foreign`) — no JNA, no JNI glue, zero runtime
dependencies beyond Jackson. The JAR ships the native library for every
supported platform at the classpath root under `<os-arch>/`
(`darwin-aarch64/`, `linux-x86-64/`, …); the binding extracts and loads
the right one for the host OS/arch. No Rust toolchain needed.

| Platform | Architectures   |
|----------|-----------------|
| Linux    | x86_64, aarch64 |
| macOS    | x86_64, arm64   |
| Windows  | x86_64, arm64   |

**JDK 22 or newer is required** (the FFM API is final since 22).

On JDK 24+ the JVM prints a restricted-method warning the first time a
library uses FFM (and future JDKs will refuse by default). Grant native
access explicitly when starting your application:

```
java --enable-native-access=ALL-UNNAMED ...
```

(That flag covers classpath applications, which is how this JAR is
consumed; if you place it on the module path instead, grant native
access to its module name.)

The native library is resolved in this order:

1. `-Ddatalogic.library.path=<dir>` — a directory containing
   `libdatalogic_c.dylib` / `libdatalogic_c.so` / `datalogic_c.dll`
   (useful for in-tree builds and overrides),
2. the JAR's bundled `<os-arch>/` classpath resource (extracted to a
   temp file), which is how the published artifact works out of the box,
3. `System.loadLibrary("datalogic_c")` — `java.library.path` and the
   OS loader paths.

> **Naming:** the Maven `groupId` is `io.github.goplasmatic` (the
> auto-verified Sonatype namespace tied to the GitHub org), but the Java
> *package* is `com.goplasmatic.datalogic`, matching the npm
> `@goplasmatic/` and Composer `goplasmatic/` scopes. Maven permits
> groupId / package divergence; consumers just need both lines correct.

## Quick start

```java
import com.goplasmatic.datalogic.Engine;

try (Engine engine = new Engine()) {
    String result = engine.apply("{\"+\":[1,2]}", "{}");  // "3"
}
```

Rules, data, and results cross the boundary as JSON strings; parse the
result with the JSON library of your choice.

## Compile once, evaluate many

Compile the rule once when you'll evaluate it against many data inputs:

```java
import com.goplasmatic.datalogic.Engine;
import com.goplasmatic.datalogic.Rule;

try (Engine engine = new Engine();
     Rule rule = engine.compile("{\"var\":\"x\"}")) {
    System.out.println(rule.evaluate("{\"x\":42}"));  // "42"
}
```

`Engine` and compiled `Rule` objects are thread-safe: build and compile
once, share them across threads. Sessions (below) are not.

## Sessions (hot loops)

A `Session` reuses one arena across evaluations and resets it at the
start of every call, so peak memory stays bounded:

```java
try (Session session = engine.openSession()) {
    for (String data : inputs) {
        String result = session.evaluate(rule, data);
    }
}
```

Open one session per thread; a `Session` is not thread-safe. Every
public type implements `AutoCloseable`, so use try-with-resources to
avoid leaking native handles.

## Data handles (parse once, evaluate many)

When the same payload feeds many evaluations, parse it once into a
`DataHandle` and skip the per-call JSON parse entirely:

```java
import com.goplasmatic.datalogic.DataHandle;

try (DataHandle data = DataHandle.parse("{\"price\": 100, \"discount\": 0.2}")) {
    rule.evaluate(data);              // one-shot path
    session.evaluate(rule, data);     // hot path: zero parse work per call
}
```

A `DataHandle` is immutable, thread-safe, and engine-independent — one
handle can feed rules compiled by different engines, from any number of
threads. It is not consumed by evaluation; close it after the last use.

## Typed results

When a rule is a predicate or a scoring function, skip the JSON result
string too. The typed evaluations take a `DataHandle` and return Java
scalars:

```java
boolean pass  = session.evaluateBool(rule, data);    // strict JSON boolean
long    count = session.evaluateLong(rule, data);    // exact integer
double  score = session.evaluateDouble(rule, data);  // any JSON number
boolean ok    = session.evaluateTruthy(rule, data);  // engine truthiness, never mismatches
```

A result of the wrong type throws `EvaluateException` with error type
`"TypeMismatch"` (e.g. `evaluateBool` on a rule that returned `3`);
`evaluateTruthy` coerces any result the same way `if`/`and`/`or` do.

## Batch evaluation

Cross the native boundary once for a whole workload. Item failures
never throw — each item of the returned list carries either the result
JSON or its own error info:

```java
import com.goplasmatic.datalogic.EvalResult;

// one rule × many payloads
List<EvalResult> perPayload = session.evaluateBatch(rule, dataHandles);

// many rules × one payload (rule-set / feature-flag shape)
List<EvalResult> perRule = session.evaluateMany(rules, dataHandle);

for (EvalResult r : perPayload) {
    if (r.isSuccess()) {
        use(r.value());                          // result JSON string
    } else {
        log(r.errorTag(), r.errorMessage());     // e.g. "Thrown", "boom"
    }
}
```

## API surface

The binding mirrors the Rust engine's
[API tier model](https://github.com/GoPlasmatic/datalogic-rs#one-api-shape-every-binding).
Methods take and return JSON strings unless noted.

| Tier            | Entry point                                                 | Use when                                              |
|-----------------|-------------------------------------------------------------|-------------------------------------------------------|
| One-shot        | `engine.apply(rule, data)`                                  | Ad-hoc evaluation, one rule + one data shape          |
| Engine + config | `new Engine(templating)` / `Engine.builder()…build()`       | Templating mode, custom operators, evaluation config  |
| Compile once    | `engine.compile(rule)` → `rule.evaluate(data)`              | Same rule evaluated against many data inputs          |
| Data handle     | `DataHandle.parse(json)` → `rule.evaluate(dataHandle)`      | Same payload evaluated by many rules / many times     |
| Session         | `engine.openSession()` → `session.evaluate(rule, data)`     | Hot loops: amortise arena reset across iterations     |
| Typed           | `session.evaluateBool/Long/Double/Truthy(rule, dataHandle)` | Predicates and scores without JSON result parsing     |
| Batch           | `session.evaluateBatch(rule, datas)` / `session.evaluateMany(rules, data)` | Whole workloads in one native call     |
| Traced          | `engine.openTracedSession()` → `session.evaluate(rule, data)` | Step-by-step debugging; feeds the React debugger    |

## Custom operators

Register Java-implemented operators through the builder. Each callback
receives the operator's pre-evaluated arguments as a JSON-array string
and returns a JSON-value string; throwing signals an evaluation error
whose message bubbles back to the caller.

```java
import com.fasterxml.jackson.databind.ObjectMapper;
import com.goplasmatic.datalogic.Engine;

ObjectMapper mapper = new ObjectMapper();

try (Engine engine = Engine.builder()
        .addOperator("double", argsJson -> {
            int n = mapper.readTree(argsJson).get(0).asInt();
            return String.valueOf(n * 2);
        })
        .build()) {
    System.out.println(engine.apply("{\"double\":[21]}", "{}"));  // "42"
}
```

`jackson-databind` is already on the classpath: the binding depends on
it for trace parsing.

**Built-ins win**: a custom registration of a built-in name (`+`, `if`,
`var`, ...) never dispatches at evaluation time; the built-in always
runs.

## Engine configuration

`Engine.builder().setConfigJson(json)` sets the evaluation semantics
from a JSON object string: an optional `preset` plus per-field
overrides. Unknown keys or values throw `EvaluateException` (error type
`ConfigurationError`), so typos fail loudly:

```java
try (Engine lenient = Engine.builder()
        .setConfigJson("{\"division_by_zero\":\"return_null\"}")
        .build()) {
    lenient.apply("{\"/\":[1.5,0]}", "{}");  // "null"
}

try (Engine strict = Engine.builder()
        .setConfigJson("{\"preset\":\"strict\"}")
        .build()) {
    strict.apply("{\"+\":[\"\",1]}", "{}");  // throws: strict rejects non-numeric coercion
}
```

| Key | Values |
|-----|--------|
| `preset` | `"default"`, `"safe_arithmetic"`, `"strict"` |
| `arithmetic_nan_handling` | `"throw_error"`, `"ignore_value"`, `"coerce_to_zero"`, `"return_null"` |
| `division_by_zero` | `"return_saturated"`, `"throw_error"`, `"return_null"`, `"return_infinity"` |
| `loose_equality_errors` | `bool` |
| `truthy_evaluator` | `"javascript"`, `"python"`, `"strict_boolean"` |
| `numeric_coercion` | object of bools: `empty_string_to_zero`, `null_to_zero`, `bool_to_number`, `reject_non_numeric` |
| `max_recursion_depth` | integer >= 1 |

The `preset` applies first; the remaining keys override individual
fields on top of it. Every binding shares this JSON schema and parses it
with the same core code, so a config that works here works in the
Python, Node, and WASM bindings too. The full semantics of each knob are
documented on the Rust crate's
[`EvaluationConfig`](https://docs.rs/datalogic-rs/latest/datalogic_rs/struct.EvaluationConfig.html).

## Error handling

Everything the binding throws extends `DatalogicException` (unchecked):

| Exception           | When                                                          |
|---------------------|---------------------------------------------------------------|
| `ParseException`    | Malformed rule or data JSON, or an unsupported operator       |
| `EvaluateException` | Operator failure at runtime, a rejected engine config, or a typed-result type mismatch (`errorType() == "TypeMismatch"`) |
| `DatalogicException` (base) | Invalid arguments at the boundary (e.g. a rule compiled by a different engine) or an internal engine error |

The structured fields ride on the base class: `errorType()` is the
stable engine tag (e.g. `"ParseError"`, `"Thrown"`, `"NaN"`),
`operatorName()` the outermost failing operator (e.g. `"+"`), and
`pathJson()` the root-to-leaf error path as a JSON array; each is
`null` when not applicable.

```java
import com.goplasmatic.datalogic.EvaluateException;

try (Engine engine = new Engine()) {
    engine.apply("{\"+\":[\"x\",1]}", "{}");  // arithmetic on a non-numeric string
} catch (EvaluateException e) {
    e.errorType();     // runtime error tag, e.g. "NaN"
    e.operatorName();  // "+"
    e.pathJson();      // JSON-array path through the compiled tree
}
```

## Threading

| Type         | Pattern                                  |
|--------------|-------------------------------------------|
| `Engine`     | Build once; share across threads          |
| `Rule`       | Compile once; share across threads        |
| `DataHandle` | Parse once; share across threads          |
| `Session`    | One per worker thread; never share        |

`TracedSession` is thread-safe as well.

## Tracing

```java
try (TracedSession session = engine.openTracedSession()) {
    TracedRun run = session.evaluate("{\"+\":[{\"var\":\"x\"},1]}", "{\"x\":41}");
    System.out.println(run.result());        // 42
    System.out.println(run.steps().size());  // executed node count
}
```

Same trace envelope as every other binding; the
[React debugger](https://github.com/GoPlasmatic/datalogic-rs/tree/main/ui)
consumes it directly. `TracedRun` exposes `result()`,
`expressionTree()`, `steps()`, `error()`, and `structuredError()` as
Jackson `JsonNode`s; runtime failures surface inside the run rather than
as exceptions. Tracing disables the optimizer so every operator appears
in the trace: use it for debugging, not hot paths.

## Performance

<!-- canonical-bench v5.0 -->
Geomean across 50 operator benchmark suites (Apple M2 Pro, median of 3 runs; pairwise shared-suite ratios per the [methodology](https://github.com/GoPlasmatic/datalogic-rs/blob/main/tools/benchmark/BENCHMARK.md)): the native Rust core evaluates at **8.9 ns/op**, 7.9× faster than json-logic-engine (compiled, the fastest JS engine), 30.6× faster than jsonlogic-rs (the closest Rust alternative), and 104.2× faster than the json-logic-js reference implementation. The WASM build under Node measures 901.1 ns geomean (101× native); on Node servers, prefer `@goplasmatic/datalogic-node`.

The FFM boundary adds a small per-call marshalling cost on top of the
core numbers; pre-parsed `DataHandle`s roughly halve it versus JSON
strings, and `evaluateBatch` / `evaluateMany` amortise the crossing over
whole workloads.

## Building from source

The binding lives in
[`bindings/jvm/`](https://github.com/GoPlasmatic/datalogic-rs/tree/main/bindings/jvm)
and loads the C ABI cdylib from `bindings/c/`. Build that once, then use
Maven as usual (Surefire points the `datalogic.library.path` system
property at the cargo target dir for local tests):

```bash
git clone https://github.com/GoPlasmatic/datalogic-rs
cd datalogic-rs/bindings/c && cargo build --release
cd ../jvm      # needs JDK 22+
mvn test
mvn package    # target/datalogic-5.0.1.jar + sources + javadoc
```

## Learn more

- [datalogic-rs repository](https://github.com/GoPlasmatic/datalogic-rs#readme)
- [Rust crate deep-dive](https://github.com/GoPlasmatic/datalogic-rs/tree/main/crates/datalogic-rs#readme)
- [JVM docs chapter](https://goplasmatic.github.io/datalogic-rs/jvm.html)
- [Online playground](https://goplasmatic.github.io/datalogic-rs/playground/)
- [JSONLogic specification](https://jsonlogic.com)
- [C ABI internals](https://github.com/GoPlasmatic/datalogic-rs/tree/main/bindings/c#readme)

## License

Apache-2.0. See the
[main repository](https://github.com/GoPlasmatic/datalogic-rs) for
source and contribution guidelines.
