# Java / Kotlin (JVM)

The JVM binding `io.github.goplasmatic:datalogic` reaches the shared C ABI directly through the Java FFM API (`java.lang.foreign`), with no JNA or JNI glue. It requires **JDK 22 or newer** (the FFM API is final since 22) and works from Java, Kotlin, and Scala.

## Installation

Add the dependency to your project:

### Maven (`pom.xml`)

```xml
<dependency>
    <groupId>io.github.goplasmatic</groupId>
    <artifactId>datalogic</artifactId>
    <version>5.1.0</version>
</dependency>
```

### Gradle (`build.gradle`)

```groovy
implementation 'io.github.goplasmatic:datalogic:5.1.0'
```

*Note: The Maven `groupId` is `io.github.goplasmatic`, but the Java package path is `com.goplasmatic.datalogic`.*

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

Always compile rules when executing them repeatedly. Use Java's `try-with-resources` statement to ensure native resources are disposed of correctly:

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
                    // Reuses the internal arena; does not allocate fresh memory
                    String name = session.evaluate(rule, input);
                    System.out.println(name);
                }
            }
        }
    }
}
```

## Concurrency

*   `Engine` and `Rule` instances are fully thread-safe and can be shared globally.
*   `Session` instances are **not** thread-safe and must be kept local to individual threads.

## Going deeper

- [C ABI internals: memory management & thread safety](c-abi.md) — the native-heap ownership rules every FFI binding shares
- [Engine configuration semantics](advanced/configuration.md)
- [JVM binding README](https://github.com/GoPlasmatic/datalogic-rs/tree/main/bindings/jvm#readme) — full API surface, error types, and platform table
