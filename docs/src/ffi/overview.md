# C-FFI Bindings Overview

For language runtimes without direct Rust interoperability libraries (like `pyo3` or `napi-rs`), `datalogic-rs` exposes a stable C ABI in `bindings/c`. 

This ABI is consumed by the **Go, JVM (Java/Kotlin), .NET (C#), and PHP** bindings.

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
     |    +-------------> JVM JNA (io.github.goplasmatic:datalogic)
     +------------------> Go cgo (github.com/GoPlasmatic/datalogic-rs/bindings/go)
```

## Binary Distribution

Because these bindings rely on compiled shared/static libraries, the release pipeline compiles the `bindings/c` code for all supported operating systems and architectures. The binaries are then bundled into the standard package layout for each ecosystem.

| Ecosystem | Packaging | Binaries Layout | Loading Mechanism |
|---|---|---|---|
| **Go** | Go Module | Static libraries in `lib/<os>_<arch>/` | cgo static linking at compile time |
| **JVM** | Maven JAR | Shared libraries under `META-INF/native/<platform>/` | JNA `Native.load` at runtime |
| **.NET** | NuGet | Shared libraries under `runtimes/<rid>/native/` | P/Invoke `LibraryImport` at runtime |
| **PHP** | Composer | Shared libraries under `lib/<os>-<arch>/` | PHP `FFI::cdef` at runtime |

## The JSON-in/JSON-out Rule

To keep the C ABI surface simple and performant, all inputs and outputs crossing the boundary are **NUL-terminated UTF-8 JSON strings**. 
No complex struct marshaling is performed at the boundary. Instead, inputs are serialized to JSON in the host language, passed to Rust, evaluated, and the result is returned as a JSON string to be parsed back by the host.
