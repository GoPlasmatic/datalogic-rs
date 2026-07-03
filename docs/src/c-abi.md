# C ABI: Embedding & Writing New Bindings

For language runtimes without direct Rust interoperability libraries (like `pyo3` or `napi-rs`), `datalogic-rs` exposes a stable C ABI in [`bindings/c`](https://github.com/GoPlasmatic/datalogic-rs/tree/main/bindings/c). It is how the **Go, JVM (Java/Kotlin), .NET (C#), and PHP** bindings talk to the core, and it is the starting point if you want to embed the engine in a language we don't ship yet.

```
+-------------------+
| datalogic-rs Core |
+---------+---------+
          | (Rust path-dependency)
+---------v---------+
|    bindings/c     | (C ABI, generates datalogic.h / libdatalogic_c)
+----+----+----+----+
     |    |    |    |
     |    |    |    +---> PHP FFI (goplasmatic/datalogic)
     |    |    +--------> .NET P/Invoke (Goplasmatic.Datalogic)
     |    +-------------> JVM FFM (io.github.goplasmatic:datalogic)
     +------------------> Go cgo (github.com/GoPlasmatic/datalogic-rs/bindings/go/v5)
```

The full function-by-function surface, build instructions, and cbindgen notes live in the [C ABI README](https://github.com/GoPlasmatic/datalogic-rs/tree/main/bindings/c#readme).

## Binary distribution

Because these bindings rely on compiled shared/static libraries, the release pipeline compiles the `bindings/c` code for all supported operating systems and architectures. The binaries are then bundled into the standard package layout for each ecosystem.

| Ecosystem | Packaging | Binaries Layout | Loading Mechanism |
|---|---|---|---|
| **Go** | Go Module | Static libraries in `lib/<os>_<arch>/` | cgo static linking at compile time |
| **JVM** | Maven JAR | Shared libraries at the classpath root under `<os-arch>/` | FFM (`java.lang.foreign`) at runtime |
| **.NET** | NuGet | Shared libraries under `runtimes/<rid>/native/` | P/Invoke `LibraryImport` at runtime |
| **PHP** | Composer | Shared libraries under `lib/<os>-<arch>/` | PHP `FFI::cdef` at runtime |

## The JSON-in/JSON-out rule

To keep the C ABI surface simple and performant, inputs and outputs crossing the boundary are **UTF-8 JSON strings passed as `(pointer, length)` pairs** (ABI v2 carries an explicit byte length, so there are no NUL terminators and embedded NULs or non-ASCII bytes are safe).
No complex struct marshaling is performed at the boundary. Instead, inputs are serialized to JSON in the host language, passed to Rust, evaluated, and the result is returned as JSON bytes to be parsed back by the host.

## Memory management & safety

Because the Go, JVM, .NET, and PHP bindings interface with the Rust core over a C FFI boundary, memory management rules differ significantly from native Go/Java/C#/PHP code.

### ⚠️ The danger: native memory leaks

When you instantiate an `Engine` or compile a `Rule` in a managed language, the actual structures (optimized bytecode ASTs, configuration options, operator collections) are allocated on the **native Rust heap**, and only a raw 64-bit memory pointer is returned to your host language.

Managed garbage collectors (like the JVM, .NET CLR, Go's GC, or PHP's Zend GC) **only track the size of the wrapper object itself** (which is usually a few bytes representing the pointer address). The GC has no awareness of the potentially megabytes of memory allocated on the native heap behind that pointer.

If you let these wrapper objects go out of scope without calling their destructors, **the native memory will leak permanently** until the host process terminates.

### 🛡️ Best practices per language

Follow these patterns to ensure leak-free evaluation:

#### 🟢 Go: explicit cleanup with `defer`

Go does not support object finalizers or automatic destructors for local variables. You must call `.Close()` explicitly.

```go
engine := datalogic.NewEngine()
defer engine.Close() // ALWAYS defer Close

rule, err := engine.Compile(ruleJSON)
if err != nil {
    return err
}
defer rule.Close() // ALWAYS defer Close

session := engine.Session()
defer session.Close() // ALWAYS defer Close
```

#### ☕ JVM: try-with-resources

Java and Kotlin provide the `try-with-resources` statement. All `datalogic` classes implement `AutoCloseable`, making this the cleanest and safest pattern:

```java
// Automatic closure of Engine and Rule
try (Engine engine = new Engine();
     Rule rule = engine.compile(ruleStr)) {

    // Automatic closure of Session
    try (Session session = engine.openSession()) {
        String result = session.evaluate(rule, data);
    }
} // Engine, Rule, and Session are guaranteed to be closed here
```

#### 🔷 .NET: `using` statements

In C#, use the `using` keyword. If you forget, the C# wrapper provides a finalizer fallback, but explicit disposal is highly recommended:

```csharp
using var engine = new Engine();
using var rule = engine.Compile(ruleJSON);

using (var session = engine.OpenSession())
{
    var result = session.Evaluate(rule, data);
} // Session is disposed here
// Engine and Rule are disposed when the current method scope ends
```

#### 🐘 PHP: scope-destructors & `close()`

PHP releases FFI objects when they fall out of scope. However, for CLI daemons, Swoole services, or long-running PHP-FPM requests, always close handles manually:

```php
$engine = new Engine();
$rule = $engine->compile($ruleJSON);

$session = $engine->openSession();
$result = $session->evaluate($rule, $data);

// Explicit cleanup prevents memory creep in long-running processes
$session->close();
$rule->close();
$engine->close();
```

## 🧵 Thread safety & concurrency

When sharing compiled logic across multiple threads, remember the following thread-safety boundaries:

| Class / Type | Thread-Safe? | Usage Pattern |
|---|---|---|
| **`Engine`** | **Yes** | Construct once globally; share across all threads/goroutines. |
| **`Rule`** | **Yes** | Compile once; share and call `Evaluate()` concurrently. |
| **`Session`** | ❌ **No** | **Never share sessions.** Keep one `Session` instance per thread. |
| **`TracedSession`** | **Yes** | Open once; evaluate concurrently. |

### Why `Session` is not thread-safe

`Session` contains a fast, zero-copy `bumpalo` arena allocator. It works by moving a cursor forward on a pre-allocated memory page. If two threads evaluate logic concurrently using the same session, they will overwrite each other's memory, leading to crashes or data corruption.
