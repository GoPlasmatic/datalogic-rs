# Java / Kotlin (JVM)

The JVM binding `io.github.goplasmatic:datalogic` calls the shared C ABI through the Java FFM API (`java.lang.foreign`), with no JNA or JNI glue. It requires **JDK 22 or newer** (the FFM API is final since 22) and works from Java, Kotlin, and Scala.

## Installation

Add the dependency to your project:

### Maven (`pom.xml`)

```xml
<dependency>
    <groupId>io.github.goplasmatic</groupId>
    <artifactId>datalogic</artifactId>
    <version>5.8.1</version>
</dependency>
```

### Gradle (`build.gradle`)

```groovy
implementation 'io.github.goplasmatic:datalogic:5.8.1'
```

On JDK 24 and newer the JVM prints a restricted-method warning the first time a library calls FFM, and a later JDK will deny the call by default. Grant native access when you start your application:

```
java --enable-native-access=ALL-UNNAMED ...
```

That flag covers classpath applications. The JAR declares `Automatic-Module-Name: com.goplasmatic.datalogic`, so on the module path you write `requires com.goplasmatic.datalogic;` and grant access with `--enable-native-access=com.goplasmatic.datalogic`.

The JAR bundles the native library for Linux, macOS and Windows on x86_64 and arm64. On first start the binding extracts the one for your platform to a per-user cache (`$XDG_CACHE_HOME/datalogic/native/`, default `~/.cache/datalogic/native/`, on Linux; `~/Library/Caches/datalogic/native/` on macOS; `%LOCALAPPDATA%\datalogic\native\` on Windows), in a directory named by the library's SHA-256, and later starts reuse that file. If the cache is not writable, it extracts to a temp directory instead. Set `-Ddatalogic.library.path=<dir>` to load a library you built yourself.

*The Maven `groupId` is `io.github.goplasmatic`, but the Java package path is `com.goplasmatic.datalogic`.*

## Quick Start

### One-Shot Evaluation

```java
import com.goplasmatic.datalogic.Engine;

public class Main {
    public static void main(String[] args) {
        try (Engine engine = new Engine()) {
            String result = engine.apply("{\"+\": [1, 2, 3]}", "{}");
            System.out.println(result); // "6"
        }
    }
}
```

### Reusable Compiled Rules

Compile rules that you evaluate repeatedly. Use Java's `try-with-resources` statement to close native resources when you are done with them:

```java
import com.goplasmatic.datalogic.Engine;
import com.goplasmatic.datalogic.Rule;

public class Main {
    public static void main(String[] args) {
        try (Engine engine = new Engine();
             Rule rule = engine.compile("{\"if\": [{ \">\": [{\"var\": \"score\"}, 50] }, \"pass\", \"fail\"]}")) {
            
            System.out.println(rule.evaluate("{\"score\": 75}")); // "pass"
            System.out.println(rule.evaluate("{\"score\": 30}")); // "fail"
        }
    }
}
```

### Arena Recycling with `Session`

To recycle memory allocations in hot loops, open a `Session`:

```java
import com.goplasmatic.datalogic.Engine;
import com.goplasmatic.datalogic.Rule;
import com.goplasmatic.datalogic.Session;

public class Main {
    public static void main(String[] args) {
        try (Engine engine = new Engine();
             Rule rule = engine.compile("{\"var\": \"user.name\"}")) {
            
            try (Session session = engine.openSession()) {
                for (String input : dataset) {
                    // Reuses the session's arena, reset at the start of each call
                    String name = session.evaluate(rule, input);
                    System.out.println(name);
                }
            }
        }
    }
}
```

## Checking and Inspecting Rules

The engine can report problems in a rule before it runs, and describe a compiled rule:

```java
import com.goplasmatic.datalogic.CompileMode;
import com.goplasmatic.datalogic.ParseException;

String diagnostics = engine.check("{\"if\": [true, {\"vr\": \"x\"}]}", CompileMode.ENGINE);
// [{"code":"UnknownOperator","severity":"error",
//   "message":"unknown operator `vr`; did you mean `var`?","pointer":"/if/1","operator":"vr"}]

try (Rule rule = engine.compileChecked(ruleJson)) {
    String facts = rule.facts();  // {"reads":[["user","age"]], "operators":[...], "deterministic":true, ...}
} catch (ParseException e) {
    // e.errorType() is "CompileError"; e.diagnosticsJson() lists every problem
}
```

| Method | Returns |
|--------|---------|
| `engine.check(rule, mode)` | Every problem the engine can see, as a JSON array of `{code, severity, message, pointer, operator}`; it does not throw for a bad rule |
| `engine.compileChecked(rule)` | A `Rule`, or `ParseException` with error type `"CompileError"` if `check` finds an error |
| `engine.compileTemplate(rule)` / `compileStrict(rule)` | A `Rule` compiled with templating on or off, whatever the engine's own mode |
| `rule.facts()` | The data paths the rule reads, the operators it calls, and whether it is deterministic, as JSON |
| `engine.operators()` | The operator catalogue, in the schema of [`operators.json`](https://github.com/GoPlasmatic/datalogic-rs/blob/main/docs/src/operators/operators.json) |
| `engine.truthy(valueJson)` | Whether a JSON value is truthy under the engine's configured truthiness |

`Engine.builder()` also takes `withFamilies("ExtString", ...)`, which keeps the engine to the JSONLogic core plus the operator families you name, and `withStrictOperatorNames(true)`, which makes `addOperator` throw for a name a built-in operator answers to. The engine config accepts `"missing_var": "error"`, which turns a `var` read that finds nothing into a `VariableNotFound` error.

## Concurrency

*   `Engine` and `Rule` instances are thread-safe; you can share them globally.
*   `Session` instances are **not** thread-safe; keep each one local to a single thread.
*   `close()` is idempotent and frees the native handle exactly once, even when several threads call it. A shared `java.lang.ref.Cleaner` frees a handle you never close, including an `EngineBuilder` dropped before `build()`.
*   Rules and sessions keep their engine's custom operators alive, so they keep working after `engine.close()`.

## Going deeper

- [C ABI internals: memory management & thread safety](c-abi.md): the native-heap ownership rules every FFI binding shares
- [Engine configuration semantics](advanced/configuration.md)
- [Rule analysis](advanced/rule-analysis.md): diagnostic codes, the facts fields and the operator catalogue
- [JVM binding README](https://github.com/GoPlasmatic/datalogic-rs/tree/main/bindings/jvm#readme): full API surface, error types, and platform table
