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

Because these bindings rely on compiled shared/static libraries, the release pipeline compiles the `bindings/c` code for every supported operating system and architecture, then bundles the binaries into each ecosystem's standard package layout.

| Ecosystem | Packaging | Binaries Layout | Loading Mechanism |
|---|---|---|---|
| **Go** | Go Module | Static libraries in `lib/<os>_<arch>/` | cgo static linking at compile time |
| **JVM** | Maven JAR | Shared libraries at the classpath root under `<os-arch>/` | FFM (`java.lang.foreign`) at runtime |
| **.NET** | NuGet | Shared libraries under `runtimes/<rid>/native/` | P/Invoke `LibraryImport` at runtime |
| **PHP** | Composer | Shared libraries under `lib/<os>-<arch>/` | PHP FFI: preloaded `FFI::scope` (`opcache.preload`) with `FFI::cdef` fallback |

## The JSON-in/JSON-out rule

To keep the C ABI surface simple and performant, inputs and outputs crossing the boundary are **UTF-8 JSON strings passed as `(pointer, length)` pairs** (ABI v2 carries an explicit byte length, so there are no NUL terminators and embedded NULs or non-ASCII bytes are safe).
The boundary does no complex struct marshaling. The host language serializes inputs to JSON and passes them to Rust; Rust evaluates them and returns the result as JSON bytes for the host to parse.

## Memory management & safety

Because the Go, JVM, .NET, and PHP bindings interface with the Rust core over a C FFI boundary, memory management rules differ from native Go/Java/C#/PHP code.

### ⚠️ The danger: native memory leaks

When you instantiate an `Engine` or compile a `Rule` in a managed language, the core allocates the actual structures (optimized bytecode ASTs, configuration options, operator collections) on the **native Rust heap** and returns only a raw 64-bit memory pointer to your host language.

Managed garbage collectors (like the JVM, .NET CLR, Go's GC, or PHP's Zend GC) **only track the size of the wrapper object itself** (usually a few bytes holding the pointer address). The GC cannot see the native memory behind that pointer, which can run to megabytes.

If you let these wrapper objects go out of scope without closing them, the collector's normal accounting does not reclaim the native memory. The outcome depends on the language: JVM handles that are never closed **leak permanently** until the host process terminates (the binding registers no `Cleaner`); the Go and .NET wrappers register best-effort finalizers, so they eventually recover the memory, but only when the GC happens to run; PHP releases the handle in the wrapper's destructor as soon as the object goes out of scope. None of these fallbacks replaces explicit cleanup.

### 🛡️ Best practices per language

Follow these patterns to ensure leak-free evaluation:

#### 🟢 Go: explicit cleanup with `defer`

Every Go handle type (`Engine`, `Rule`, `Session`, `TracedSession`, `DataHandle`) registers a `runtime.SetFinalizer` fallback, but finalizers are non-deterministic and best-effort: they run only when the GC notices the wrapper, which may be long after the native memory stopped being useful. Always `defer` `.Close()` explicitly.

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

Java and Kotlin provide the `try-with-resources` statement. Every native-handle class (`Engine`, `Rule`, `Session`, `TracedSession`, `DataHandle`) implements `AutoCloseable`, which makes this the safest pattern (`EngineBuilder` is not closeable; it releases its native handle when `build()` runs):

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

In C#, use the `using` keyword. If you forget, the C# wrapper provides a finalizer fallback, but dispose explicitly rather than rely on it:

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

PHP releases FFI objects when they fall out of scope. For CLI daemons, Swoole services, or long-running PHP-FPM requests, close handles manually:

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

When you share compiled logic across threads, respect these thread-safety boundaries:

| Class / Type | Thread-Safe? | Usage Pattern |
|---|---|---|
| **`Engine`** | **Yes** | Construct once globally; share across all threads/goroutines. |
| **`Rule`** | **Yes** | Compile once; share and call `Evaluate()` concurrently. |
| **`Session`** | ❌ **No** | **Never share sessions.** Keep one `Session` instance per thread. |
| **`TracedSession`** | **Yes** | Open once; evaluate concurrently. |
| **`DataHandle`** | **Yes** | Parse once; share across threads and engines (evaluation only reads it); close after the last evaluation. |

### Why `Session` is not thread-safe

`Session` contains a zero-copy `bumpalo` arena allocator, which allocates by moving a cursor forward on a pre-allocated memory page. If two threads evaluate logic concurrently with the same session, they overwrite each other's memory and cause crashes or data corruption.
