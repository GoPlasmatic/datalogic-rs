# io.github.goplasmatic:datalogic

[![Maven Central](https://img.shields.io/maven-central/v/io.github.goplasmatic/datalogic)](https://central.sonatype.com/artifact/io.github.goplasmatic/datalogic)
[![CI](https://github.com/GoPlasmatic/datalogic-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/GoPlasmatic/datalogic-rs/actions/workflows/ci.yml)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

Part of [datalogic-rs](https://github.com/GoPlasmatic/datalogic-rs): one engine, every runtime.

Java bindings for [datalogic-rs](https://github.com/GoPlasmatic/datalogic-rs),
the JSONLogic rules engine with one Rust core and official bindings for
Rust, Node.js, the browser (WASM), Python, Go, Java, .NET, and PHP.
Compile a rule once and evaluate it many times, natively in Java. Same
rules, same semantics: every binding runs the same core and passes the
same 2,128-case conformance battery (66 suites).

For the cross-runtime overview and the API-tier model every binding
implements, see the
[repo README](https://github.com/GoPlasmatic/datalogic-rs#readme).

> **New in v5.** This package is new: there is no v4 Java artifact. If
> you are coming from the v4 Rust crate or the v4
> `@goplasmatic/datalogic` WASM package, see
> [MIGRATION.md](https://github.com/GoPlasmatic/datalogic-rs/blob/main/MIGRATION.md)
> for the engine's v4 → v5 changes.

## Install

```xml
<dependency>
    <groupId>io.github.goplasmatic</groupId>
    <artifactId>datalogic</artifactId>
    <version>5.8.1</version>
</dependency>
```

Gradle: `implementation("io.github.goplasmatic:datalogic:5.8.1")`

The binding calls the engine's C ABI through the Java FFM API
(`java.lang.foreign`): no JNA, no JNI glue, and no runtime dependency
besides `jackson-databind`. The JAR ships the native library for every
supported platform at the classpath root under `<os-arch>/`
(`darwin-aarch64/`, `linux-x86-64/`, ...), and the binding loads the one
for the host OS and architecture. You need no Rust toolchain.

| Platform | Architectures   |
|----------|-----------------|
| Linux    | x86_64, aarch64 |
| macOS    | x86_64, arm64   |
| Windows  | x86_64, arm64   |

**JDK 22 or newer is required** (the FFM API is final since 22).

On JDK 24 and newer the JVM prints a restricted-method warning the
first time a library calls FFM, and a later JDK will deny the call by
default. Grant native access when you start the application:

```
java --enable-native-access=ALL-UNNAMED ...
```

That flag covers classpath applications. The JAR declares
`Automatic-Module-Name: com.goplasmatic.datalogic`, so on the module
path you write `requires com.goplasmatic.datalogic;` and grant access
with `--enable-native-access=com.goplasmatic.datalogic`.

The binding resolves the native library in this order:

1. `-Ddatalogic.library.path=<dir>`: a directory holding
   `libdatalogic_c.dylib` / `libdatalogic_c.so` / `datalogic_c.dll`,
   for in-tree builds and overrides.
2. The JAR's bundled `<os-arch>/` resource, the default for the
   published artifact. The first start extracts it to a per-user cache,
   in a directory named by the library's SHA-256 under
   `$XDG_CACHE_HOME/datalogic/native/` (default
   `~/.cache/datalogic/native/`) on Linux,
   `~/Library/Caches/datalogic/native/` on macOS, and
   `%LOCALAPPDATA%\datalogic\native\` on Windows. Later starts load
   that file once its hash matches. If the cache is not writable, the
   binding extracts to a fresh temp directory instead.
3. `System.loadLibrary("datalogic_c")`: `java.library.path` and the OS
   loader paths.

If none of them works, the binding throws `UnsatisfiedLinkError`
listing every attempt. It throws the same error when the loaded library
does not implement C ABI v2.1 or a later v2 minor
(`datalogic_abi_version() == 2`, `datalogic_abi_minor() >= 1`).

> **Naming:** the Maven `groupId` is `io.github.goplasmatic` (the
> Sonatype namespace verified through the GitHub org), and the Java
> package is `com.goplasmatic.datalogic`, matching the npm
> `@goplasmatic/` and Composer `goplasmatic/` scopes. Your build file
> uses the first, your imports the second.

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

### Compile modes and checked compiles

`compile` reads a rule in the engine's own mode. `compileTemplate` and
`compileStrict` choose templating for one compile, so one engine (with
one set of custom operators) can compile both conditions and output
templates. `compileMode(rule, mode)` takes the mode as a `CompileMode`
(`ENGINE`, `STRICT`, `TEMPLATE`).

```java
try (Rule shape = engine.compileTemplate("{\"user\": {\"var\": \"name\"}, \"n\": 1}")) {
    shape.evaluate("{\"name\": \"ana\"}");  // {"user":"ana","n":1}
}
```

`check(rule, mode)` reports every problem the engine can see before the
rule runs, as a JSON array of `{code, severity, message, pointer,
operator}`, where `pointer` is an RFC 6901 JSON Pointer into the rule.
Errors are what will fail (an unknown operator, with a suggestion one
edit away; an argument count the operator rejects; a timezone that does
not exist); warnings run but are probably mistakes. A bad rule does not
make `check` throw. `compileChecked(rule)` compiles only a rule with no
error and otherwise throws `ParseException` with error type
`"CompileError"`, whose `diagnosticsJson()` holds the same array:

```java
engine.check("{\"if\": [{\">\": [{\"var\": \"age\"}, 17]}, \"adult\", {\"vr\": \"x\"}]}",
        CompileMode.ENGINE);
// [{"code":"UnknownOperator","severity":"error","message":"unknown operator `vr`; did you mean `var`?",
//   "pointer":"/if/2","operator":"vr"}]

try (Rule rule = engine.compileChecked(ruleJson)) {
    // use the rule
} catch (ParseException e) {
    e.errorType();        // "CompileError"
    e.diagnosticsJson();  // every diagnostic, errors and warnings
}
```

### What a rule reads

`rule.facts()` returns, as JSON, the data paths the rule reads (each as
its segments), the operators it calls, and whether its result depends
only on its data:

```java
try (Rule rule = engine.compile(
        "{\"and\": [{\">=\": [{\"var\": \"user.age\"}, 18]},"
        + " {\"in\": [{\"var\": \"user.plan\"}, [\"pro\", \"team\"]]}]}")) {
    rule.facts();
    // {"reads":[["user","age"],["user","plan"]],"computed_reads":false,"reads_complete":true,
    //  "reads_data":true,"operators":[">=","and","in","val"],"custom_operators":[],
    //  "deterministic":true}
}
```

`reads_complete` is `false` when the rule builds a path at runtime or
calls a custom operator, and `deterministic` is `false` for `now` and
for any custom operator. The facts describe the compiled rule, so a
branch the optimizer removed is neither read nor listed.

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
handle type (`Engine`, `Rule`, `Session`, `TracedSession`,
`DataHandle`) implements `AutoCloseable`; use try-with-resources to
free native memory when the block ends. A shared `java.lang.ref.Cleaner` frees a
handle you never close once it becomes unreachable.

## Data handles (parse once, evaluate many)

When the same payload feeds many evaluations, parse it once into a
`DataHandle` and skip the per-call JSON parse:

```java
import com.goplasmatic.datalogic.DataHandle;

try (DataHandle data = DataHandle.parse("{\"price\": 100, \"discount\": 0.2}")) {
    rule.evaluate(data);              // one-shot path
    session.evaluate(rule, data);     // hot path: zero parse work per call
}
```

A `DataHandle` is immutable, thread-safe, and engine-independent: one
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
`"TypeMismatch"` (for example `evaluateBool` on a rule that returned
`3`); `evaluateTruthy` coerces any result the same way `if`/`and`/`or`
do.

`engine.truthy(valueJson)` applies the same truthiness to a value you
already hold, so a host check agrees with the engine. The argument is
JSON text: `engine.truthy("[]")` and `engine.truthy("{}")` are `false`
under the default rules, and a configured `truthy_evaluator` applies.

## Batch evaluation

Cross the native boundary once for a whole workload. Item failures
never throw: each item of the returned list carries either the result
JSON or its own error:

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
        log(r.errorTag(), r.errorMessage());     // e.g. "Thrown" and its message
    }
}
```

A failed item also carries `errorOperator()`, the innermost failing
operator. An item whose error the binding cannot decode reads as tag
`"InternalError"` with the raw item JSON as its message, as in the Go,
.NET and PHP bindings. A `null` element in either list throws
`NullPointerException` for the whole call.

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
| Metered         | `session.evaluateMetered(rule, data, budget)` → `Metered(value, ops)` | What an evaluation costs, under an optional cap |
| Checked         | `engine.check(rule, mode)` / `engine.compileChecked(rule)`  | Reject a bad rule before it runs                      |
| Introspection   | `engine.operators()` / `rule.facts()` / `engine.truthy(json)` | Tooling: operator catalogue, what a rule reads      |

## Custom operators

Register Java-implemented operators through the builder. Each callback
receives the operator's pre-evaluated arguments as a JSON-array string
and returns a JSON-value string. If the callback throws, the evaluation
fails with error type `"Custom"` and a message that names the operator
and carries yours. The callback runs on whichever thread evaluates the
rule, so make it thread-safe if you share the engine.

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

**Built-ins win**: registering a name that one of the engine's built-in
operators answers to (`+`, `if`, `var`, an alias such as `?:`) has no
effect at evaluation time; the built-in runs. Call
`withStrictOperatorNames(true)` before `addOperator` to make such a
registration throw `EvaluateException` (error type
`"ConfigurationError"`) instead:

```java
Engine.builder()
        .withStrictOperatorNames(true)
        .addOperator("length", argsJson -> "0");  // throws: `length` is a built-in
```

A name from an operator family the engine leaves out (see
[Operator families](#operator-families)) is free for a custom operator.
Rules and sessions hold the engine that made them, so its custom
operators keep working after `engine.close()`.

## Engine configuration

`Engine.builder().setConfigJson(json)` sets the evaluation semantics
from a JSON object string: an optional `preset` plus per-field
overrides. An unknown key or value makes `setConfigJson` throw
`EvaluateException` (error type `ConfigurationError`), so a typo fails
before the engine exists:

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
| `missing_var` | `"null"` (default: a missing variable reads as `null`), `"error"` (raises `VariableNotFound`) |
| `truthy_evaluator` | `"javascript"`, `"python"`, `"strict_boolean"` |
| `numeric_coercion` | object of bools: `empty_string_to_zero`, `null_to_zero`, `bool_to_number`, `reject_non_numeric` |
| `max_recursion_depth` | integer >= 1 |
| `ops_budget` | integer >= 1, or `null` for unbounded (caps the work one evaluation may do; crossing it raises `BudgetExceeded`) |

The `preset` applies first; the remaining keys override individual
fields on top of it. Every binding shares this JSON schema and parses it
with the same core code, so a config that works here works in the
Python, Node, and WASM bindings too. The Rust crate's
[`EvaluationConfig`](https://docs.rs/datalogic-rs/latest/datalogic_rs/struct.EvaluationConfig.html)
documents the full semantics of each knob.

With `"missing_var": "error"`, a `var` / `val` read that finds nothing
throws `EvaluateException` with error type `"VariableNotFound"` naming
the path, so a typo in a path fails instead of evaluating as `null`. A
default (`{"var": ["x", 0]}`), a present `null`, `missing`,
`missing_some` and `exists` are not misses, and `try` in the rule
catches the error.

The builder also takes `withTemplating(boolean)`,
`withTemplateKeyEscape(int codePoint)` (a prefix that marks a template
key as an output field: with `'$'`, `{"$type": ...}` emits the key
`type`), `withStrictOperatorNames(boolean)` (see
[Custom operators](#custom-operators)) and `withFamilies(String...)`
(below).

### Metering: what a rule costs

`session.evaluateMetered(rule, dataJson, budget)` returns a `Metered`
record: the result JSON and the operations the evaluation charged. A
`budget` of `0` uses the engine's `ops_budget` (unbounded if it has
none); any other value caps that one call.

```java
try (Session session = engine.openSession()) {
    Metered m = session.evaluateMetered(rule, "{\"xs\": [1, 2, 3]}", 0);
    m.value();  // the result JSON
    m.ops();    // operations charged
}
```

One operation is one node the engine dispatches, one item an iterator
walks, or what an operator charges for the data it moves. Crossing the
budget throws `EvaluateException` with error type `"BudgetExceeded"`
before the work is done, and a `try` in the rule cannot catch it.

### Operator families

`engine.operators()` returns the operator catalogue as a JSON array,
one object per built-in operator the engine evaluates (`name`,
`aliases`, `family`, `feature`, `min_args`, `max_args`,
`reads_context`, `effect`, `cost`, `scoped_arg`), in the schema of
[`operators.json`](https://github.com/GoPlasmatic/datalogic-rs/blob/main/docs/src/operators/operators.json).

`withFamilies(...)` keeps an engine to the JSONLogic core plus the
families you name: `ExtString`, `ExtArray`, `ExtObject`, `ExtControl`,
`ExtMath`, `ErrorHandling`, `DateTime`, `Tensor`, `Flagd`. By default an
engine has every family. A name from a family left out compiles as an
unknown operator, which `check` reports and evaluation rejects with
error type `"InvalidOperator"`:

```java
try (Engine strings = Engine.builder().withFamilies("ExtString").build()) {
    strings.apply("{\"length\": \"abc\"}", "null");  // "3"
    strings.apply("{\"now\": []}", "null");            // throws: InvalidOperator
}
```

An unknown family name throws `EvaluateException` with error type
`"ConfigurationError"`. With strict operator names on, call
`withFamilies` before `addOperator`.

## Error handling

Engine errors are thrown as subclasses of the unchecked
`DatalogicException`:

| Exception           | When                                                          |
|---------------------|---------------------------------------------------------------|
| `ParseException`    | Malformed rule or data JSON (`"ParseError"`), or a rule `compileChecked` refused (`"CompileError"`) |
| `EvaluateException` | A failure while evaluating, an unknown operator (`"InvalidOperator"`, also raised at compile for a multi-key object outside templating), a rejected config or builder option (`"ConfigurationError"`), or a typed-result mismatch (`"TypeMismatch"`) |
| `DatalogicException` (base) | A rule compiled by a different engine than the session's (`"InvalidArgument"`), or an internal error |

Misuse of the Java API throws the standard exceptions:
`IllegalStateException` for a closed handle or a builder that already
built, `NullPointerException` for a `null` argument.

The structured fields ride on the base class: `errorType()` is the
stable engine tag (for example `"Thrown"`, `"TypeError"`,
`"VariableNotFound"`, `"BudgetExceeded"`, or the binding-level
`"TypeMismatch"` / `"InvalidArgument"`), `operatorName()` the innermost
failing operator (custom operators included), `pathJson()` the
root-to-leaf error path as a JSON array, and `nodeIdsJson()` the
compiled-node ids from leaf to root; each is `null` when not
applicable. `diagnosticsJson()` is set only for `"CompileError"`.
Arithmetic NaN surfaces as `errorType()` `"Thrown"` with a message
carrying `{"type":"NaN"}`; there is no `"NaN"` tag.

```java
import com.goplasmatic.datalogic.EvaluateException;

try (Engine engine = new Engine()) {
    engine.apply("{\"+\":[\"x\",1]}", "{}");  // arithmetic on a non-numeric string throws {"type":"NaN"}
} catch (EvaluateException e) {
    e.errorType();     // "Thrown"
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

`TracedSession` is thread-safe as well. `close()` is idempotent and safe
to call from several threads at once: exactly one call frees the native
handle. The shared `Cleaner` frees a handle you never close, including
an `EngineBuilder` dropped before `build()`. Closing a handle while
another thread is still using it is not supported; finish that work
first.

## Tracing

```java
try (TracedSession session = engine.openTracedSession()) {
    TracedRun run = session.evaluate("{\"+\":[{\"var\":\"x\"},1]}", "{\"x\":41}");
    System.out.println(run.result());        // 42
    System.out.println(run.steps().size());  // number of execution steps
    System.out.println(run.pointers());      // {"1":"/+/0/var","2":"/+/0","3":"/+/1","4":""}
}
```

Same trace envelope as every other binding; the
[React debugger](https://github.com/GoPlasmatic/datalogic-rs/tree/main/ui)
reads it as is. `TracedRun` exposes `result()`,
`expressionTree()`, `steps()`, `structuredError()` and `pointers()` as
Jackson `JsonNode`s, plus `error()` as the message `String` (`null` on
success); runtime failures surface inside the run rather than as
exceptions. `pointers()` maps each node id to the RFC 6901 JSON Pointer
of the rule value that node was compiled from, so a debugger can place
each step in the rule as written; it is a missing node when the rule
does not compile. `session.evaluate(rule, data, mode)` compiles the rule
in a `CompileMode`, so a template traces as one. Tracing disables the
optimizer so every operator appears in the trace: use it for
debugging, not hot paths.

## Performance

<!-- canonical-bench v5.1 -->
Geomean across 51 operator benchmark suites (Apple M2 Pro, median of 3 runs; pairwise shared-suite ratios per the [methodology](https://github.com/GoPlasmatic/datalogic-rs/blob/main/tools/benchmark/BENCHMARK.md)): the native Rust core evaluates at **10.3 ns/op**, 7.0× faster than json-logic-engine (compiled, the fastest JS engine), 28.1× faster than jsonlogic-rs (the closest Rust alternative), and 83.6× faster than the json-logic-js reference implementation. The WASM build under Node measures 900.5 ns geomean (88× native); on Node servers, prefer `@goplasmatic/datalogic-node`.

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
mvn package    # target/datalogic-5.8.1.jar + sources + javadoc
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
