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

The full function-by-function surface, status codes, build instructions, and cbindgen notes live in the [C ABI README](https://github.com/GoPlasmatic/datalogic-rs/tree/main/bindings/c#readme).

## ABI versioning

The ABI is **v2**: `datalogic_abi_version()` returns `2`, and a wrapper refuses to load a library that reports anything else. Additions to v2 count up in `datalogic_abi_minor()`. Minor 1 shipped with 5.8.0 and added per-compile modes (`datalogic_engine_compile_mode`), `datalogic_engine_compile_checked`, `datalogic_engine_check`, the operator catalogue (`datalogic_engine_operators`), `datalogic_engine_truthy`, rule facts (`datalogic_rule_facts`), metered session evaluation, the builder's families, template key escape and strict operator names, traced evaluation in a mode, and the `datalogic_error_node_ids_json` / `datalogic_error_diagnostics_json` accessors. A wrapper that calls those entry points checks `datalogic_abi_minor() >= 1` at load.

## Binary distribution

Because these bindings rely on compiled shared/static libraries, the release pipeline compiles the `bindings/c` code for every supported operating system and architecture, then bundles the binaries into each ecosystem's standard package layout.

| Ecosystem | Packaging | Binaries Layout | Loading Mechanism |
|---|---|---|---|
| **Go** | Go Module | Static libraries in `lib/<os>_<arch>/` | cgo static linking at compile time |
| **JVM** | Maven JAR | Shared libraries at the classpath root under `<os-arch>/` | FFM (`java.lang.foreign`) at runtime |
| **.NET** | NuGet | Shared libraries under `runtimes/<rid>/native/` | P/Invoke `LibraryImport` at runtime |
| **PHP** | Composer | Shared libraries under `lib/<os>-<arch>/` | PHP FFI: preloaded `FFI::scope` (`opcache.preload`) with `FFI::cdef` fallback |

## The JSON-in/JSON-out rule

Inputs and outputs cross the boundary as **UTF-8 JSON passed as `(pointer, length)` pairs**. ABI v2 carries an explicit byte length, so there are no NUL terminators, and embedded NULs or non-ASCII bytes are safe.
The boundary does no struct marshaling. The host language serializes inputs to JSON and passes them to Rust; Rust evaluates them and returns the result as JSON bytes for the host to parse. Diagnostics, rule facts, the operator catalogue and traced runs cross the same way, in the JSON formats every binding shares.

## Memory management & safety

Because the Go, JVM, .NET, and PHP bindings interface with the Rust core over a C FFI boundary, memory management rules differ from native Go/Java/C#/PHP code.

### The danger: native memory leaks

If you instantiate an `Engine` or compile a `Rule` in a managed language, the core allocates the actual structures (compiled node trees, configuration, operator registrations) on the **native Rust heap** and returns only a raw memory pointer to your host language.

Managed garbage collectors (like the JVM, .NET CLR, Go's GC, or PHP's Zend GC) **only track the size of the wrapper object itself** (a few bytes holding the pointer address). The GC cannot see the native memory behind that pointer, which can run to megabytes.

Every host has a fallback for a handle you never close: the JVM frees it through a shared `java.lang.ref.Cleaner` once the wrapper is unreachable, Go and .NET register finalizers, and PHP releases it in the wrapper's destructor when the object goes out of scope. The JVM, Go and .NET fallbacks run only when the collector runs, and the collector cannot see the native memory, so a busy process can hold far more native memory than its heap suggests. None of these fallbacks replaces explicit cleanup.

### Best practices per language

Follow these patterns to release native memory as soon as you are done with it:

#### Go: explicit cleanup with `defer`

Every Go handle type (`Engine`, `Rule`, `Session`, `TracedSession`, `DataHandle`) registers a `runtime.SetFinalizer` fallback, and so does an `EngineBuilder` you never `Build`, but finalizers run only when the GC notices the wrapper, which may be long after the native memory stopped being useful. `defer` `.Close()` on each handle.

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

#### JVM: try-with-resources

Java and Kotlin provide the `try-with-resources` statement. Every native-handle class (`Engine`, `Rule`, `Session`, `TracedSession`, `DataHandle`) implements `AutoCloseable`, which makes this the safest pattern. `EngineBuilder` is not closeable: `build()` releases its native handle, and the `Cleaner` frees a builder you never build.

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

#### .NET: `using` statements

In C#, use the `using` keyword. If you forget, the C# wrapper's finalizer frees the handle when the collector runs; dispose each handle instead of relying on it:

```csharp
using var engine = new Engine();
using var rule = engine.Compile(ruleJSON);

using (var session = engine.OpenSession())
{
    var result = session.Evaluate(rule, data);
} // Session is disposed here
// Engine and Rule are disposed when the current method scope ends
```

#### PHP: scope-destructors & `close()`

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

### Closing handles and custom operators

`Close()` / `close()` / `Dispose()` frees a handle exactly once, even when several threads call it at the same time (Go swaps the pointer atomically, the JVM uses `AtomicReference.getAndSet`, .NET `Interlocked.Exchange`). Closing a handle while another thread is still using it is not supported in any host.

A rule, session or traced session holds its own reference on the native engine, so it keeps working after you close the `Engine`, custom operators included: each host keeps the operator callbacks alive while any handle derived from the engine is reachable.

A custom-operator callback may evaluate through the same engine, but not through the session that is running it. That session is in use until the callback returns: its evaluate calls fail with `DATALOGIC_STATUS_INVALID_ARG`, and `reset` / `free` on it do nothing. Open a second session, or evaluate through the rule, for a nested evaluation.

## Thread safety & concurrency

Each handle type has its own thread-safety rule:

| Class / Type | Thread-Safe? | Usage Pattern |
|---|---|---|
| **`Engine`** | **Yes** | Construct once globally; share across all threads/goroutines. |
| **`Rule`** | **Yes** | Compile once; share and call `Evaluate()` concurrently. |
| **`Session`** | **No** | **Never share sessions.** Keep one `Session` instance per thread. |
| **`TracedSession`** | **Yes** | Open once; evaluate concurrently. |
| **`DataHandle`** | **Yes** | Parse once; share across threads and engines (evaluation only reads it); close after the last evaluation. |
| **`EngineBuilder`** | **No** | Configure and build on one thread, then share the resulting `Engine`. |

### Why `Session` is not thread-safe

`Session` contains a `bumpalo` arena allocator, which allocates by moving a cursor forward through pre-allocated memory. If two threads evaluate logic concurrently with the same session, they overwrite each other's memory and cause crashes or data corruption.
