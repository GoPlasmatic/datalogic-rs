# `com.goplasmatic:datalogic` — Java binding for [`datalogic-rs`](../../crates/datalogic-rs)

JNA wrapper over the shared [`bindings/c`](../c) C ABI. Targets JDK 11+.

## Install

```xml
<dependency>
    <groupId>com.goplasmatic</groupId>
    <artifactId>datalogic</artifactId>
    <version>5.0.0</version>
</dependency>
```

The JAR ships platform binaries under `META-INF/native/<jna-platform>/`
(JNA's standard JAR-resource layout); the runtime auto-extracts and
loads the right one for the host OS/arch.

## Quick start

```java
import com.goplasmatic.datalogic.Engine;

try (Engine engine = new Engine()) {
    String result = engine.apply("{\"+\":[1,2]}", "{}");  // "3"
}
```

Reusing a compiled rule:

```java
try (Engine engine = new Engine();
     Rule rule = engine.compile("{\"var\":\"x\"}")) {
    System.out.println(rule.evaluate("{\"x\":42}"));  // "42"
}
```

Hot-loop session (arena reuse):

```java
try (Session session = engine.openSession()) {
    for (String data : inputs) {
        String result = session.evaluate(rule, data);
    }
}
```

Traced evaluation:

```java
try (TracedSession session = engine.openTracedSession()) {
    TracedRun run = session.evaluate("{\"+\":[{\"var\":\"x\"},1]}", "{\"x\":41}");
    System.out.println(run.result());        // 42
    System.out.println(run.steps().size());  // executed node count
}
```

Custom operator:

```java
try (Engine engine = Engine.builder()
        .addOperator("double", argsJson -> {
            int n = mapper.readTree(argsJson).get(0).asInt();
            return String.valueOf(n * 2);
        })
        .build()) {
    System.out.println(engine.apply("{\"double\":[21]}", "{}"));  // "42"
}
```

## Build & test (development)

The Surefire plugin sets `jna.library.path` to `../c/target/release` so
local tests pick up the in-tree cdylib. So a fresh clone needs the C
ABI built once:

```bash
cd ../c && cargo build --release
cd ../jvm
mvn test
mvn package    # produces target/datalogic-5.0.0.jar + sources + javadoc
```

## Threading & memory

- `Engine`, `Rule`, `TracedSession` are thread-safe — share freely.
- `Session` is NOT thread-safe — open one per thread.
- Every public type implements `AutoCloseable`. Use try-with-resources
  to avoid leaking native handles.

## Layout

```
bindings/jvm/
├── pom.xml
├── src/main/java/com/goplasmatic/datalogic/
│   ├── Engine.java
│   ├── EngineBuilder.java
│   ├── CustomOperator.java
│   ├── Rule.java
│   ├── Session.java
│   ├── TracedSession.java
│   ├── TracedRun.java
│   ├── DatalogicException.java + ParseException + EvaluateException
│   └── internal/
│       └── DatalogicNative.java     # JNA Library interface
├── src/main/resources/META-INF/native/
│   └── (populated at release time)
└── src/test/java/com/goplasmatic/datalogic/
    └── EngineTest.java              # JUnit 5
```
