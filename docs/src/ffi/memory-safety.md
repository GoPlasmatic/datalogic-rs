# Memory Management & Safety

Because the Go, JVM, .NET, and PHP bindings interface with the Rust core over a C FFI boundary, memory management rules differ significantly from native Go/Java/C#/PHP code.

---

## ⚠️ The Danger: Native Memory Leaks

When you instantiate an `Engine` or compile a `Rule` in a managed language, the actual structures (optimized bytecode ASTs, configuration options, operator collections) are allocated on the **native Rust heap**, and only a raw 64-bit memory pointer is returned to your host language.

Managed garbage collectors (like the JVM, .NET CLR, Go's GC, or PHP's Zend GC) **only track the size of the wrapper object itself** (which is usually a few bytes representing the pointer address). The GC has no awareness of the potentially megabytes of memory allocated on the native heap behind that pointer.

If you let these wrapper objects go out of scope without calling their destructors, **the native memory will leak permanently** until the host process terminates.

---

## 🛡️ Best Practices per Language

Follow these patterns to ensure leak-free evaluation:

### 🟢 Go: Explicit Cleanup with `defer`

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

### ☕ JVM: Try-With-Resources

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

### 🔷 .NET: `using` Statements

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

### 🐘 PHP: Scope-Destructors & `close()`

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

---

## 🧵 Thread Safety & Concurrency

When sharing compiled logic across multiple threads, remember the following thread-safety boundaries:

| Class / Type | Thread-Safe? | Usage Pattern |
|---|---|---|
| **`Engine`** | **Yes** | Construct once globally; share across all threads/goroutines. |
| **`Rule`** | **Yes** | Compile once; share and call `Evaluate()` concurrently. |
| **`Session`** | ❌ **No** | **Never share sessions.** Keep one `Session` instance per thread. |
| **`TracedSession`** | ❌ **No** | Keep local to individual threads. |

### Why `Session` is Not Thread-Safe
`Session` contains a fast, zero-copy `bumpalo` arena allocator. It works by moving a cursor forward on a pre-allocated memory page. If two threads evaluate logic concurrently using the same session, they will overwrite each other's memory, leading to crashes or data corruption.
