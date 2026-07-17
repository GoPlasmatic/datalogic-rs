# Installation

## Adding to Your Project

Select your target language to see package installation instructions:

<div class="codetabs">

```rust
// Cargo.toml
[dependencies]
datalogic-rs = "5.1"

# Or run in terminal:
# cargo add datalogic-rs
```

```javascript
// npm
npm install @goplasmatic/datalogic-node # for Node.js services (native FFI)
# or:
npm install @goplasmatic/datalogic-wasm # for Browsers / Bun / Workers (WASM)
```

```python
# pip
pip install datalogic-py
```

```go
// go.mod
go get github.com/GoPlasmatic/datalogic-rs/bindings/go/v5
```

```java
// Maven: pom.xml
<dependency>
    <groupId>io.github.goplasmatic</groupId>
    <artifactId>datalogic</artifactId>
    <version>5.1.0</version>
</dependency>

// Gradle: build.gradle.kts
implementation("io.github.goplasmatic:datalogic:5.1.0")
```

```csharp
// dotnet CLI
dotnet add package Goplasmatic.Datalogic
```

```php
// Composer
composer require goplasmatic/datalogic
```

</div>

> **Note for Rust users:** v5 does **not** require `serde_json` by default — the canonical
> entry points (`Engine::eval_str`, `Engine::compile(&str)`,
> `datalogic_rs::eval_str`) are string-based. Add the `serde_json` feature
> only if you need `serde_json::Value` interop or the typed
> `eval_into::<T>` paths.

## Feature Flags

v5 splits the surface into a small core plus opt-in features:

| Feature | Default | What it adds |
|---------|---------|-------------|
| `serde_json` | off | `&serde_json::Value` interop (as `EvalInput` / `IntoLogic`) and the typed `eval_into::<T>` paths on `Engine`, `Session`, and the module-level helpers. Pulls in `serde_json` as a runtime dependency. |
| `templating` | off | Templating mode — `Engine::builder().with_templating(true).build()`. |
| `datetime` | off | `datetime`, `timestamp`, `parse_date`, `format_date`, `date_diff`, `now` operators (pulls in `chrono`). |
| `trace` | off | Per-evaluation execution tracing (`engine.trace()…`). Transitively enables `serde_json`. |
| `ext-string` | off | Extended string operators. |
| `ext-array` | off | Extended array operators (e.g. `sort`). |
| `ext-control` | off | Extended control-flow operators (`exists`, `??`, `switch`/`match`, `type`). |
| `error-handling` | off | `try` / `throw` operators. |
| `ext-math` | off | Extended math operators. |
| `flagd` | off | [OpenFeature flagd-compatible](https://flagd.dev/reference/custom-operations/) `fractional` (murmurhash3 percentage bucketing) and `sem_ver` (semantic-version comparison) operators. |
| `wasm-clock` | off | JS-host clock for the `now` operator on `wasm32-unknown-unknown` (browsers, Node, Deno, Workers); combine with `datetime`. Opt-in on purpose: it forwards to `chrono/wasmbind`, whose JS imports fail to instantiate in non-JS wasm runtimes such as wasmtime, wazero, and Chicory — leave it off there (on WASI the OS clock works without it). |

Example — opt into `serde_json::Value` interop plus templating:

```toml
[dependencies]
datalogic-rs = { version = "5.1", features = ["serde_json", "templating"] }
serde_json = "1.0"
```

## Version Selection

- **v5.x** (current): canonical string-based API, opt-in `serde_json`, builder-only operator registration. v5 is a hard cliff — no `compat` shim — so plan a single cutover.
- **v4.x**: `DataLogic` engine, `serde_json::Value`-first API. Still functional but no longer the active line.
- **v3.x**: Arena-based allocation, predates the v4 simplification. Bug-fix only.

If you're upgrading from v4, see the [Migration Guide](../migration.md).

## Other languages

The Rust crate is the engine; every other language uses its own
binding. Click through to the binding's guide for install
instructions and the language-idiomatic API:

| Language                       | Package                                                                                          | Install                                                          | Deep-dive                                                                                                       |
|--------------------------------|--------------------------------------------------------------------------------------------------|------------------------------------------------------------------|-----------------------------------------------------------------------------------------------------------------|
| Node.js (native, napi-rs)      | [`@goplasmatic/datalogic-node`](https://www.npmjs.com/package/@goplasmatic/datalogic-node)       | `npm i @goplasmatic/datalogic-node`                              | [Node native README](https://github.com/GoPlasmatic/datalogic-rs/blob/main/bindings/node/README.md)        |
| JavaScript / TypeScript (WASM) | [`@goplasmatic/datalogic-wasm`](https://www.npmjs.com/package/@goplasmatic/datalogic-wasm)       | `npm i @goplasmatic/datalogic-wasm`                              | [JS / TS docs](../javascript/installation.md)        |
| Python                         | [`datalogic-py`](https://pypi.org/project/datalogic-py/)                                         | `pip install datalogic-py`                                       | [Python docs](../python/installation.md)    |
| Go                             | `datalogic-go`                                                                                   | `go get github.com/GoPlasmatic/datalogic-rs/bindings/go/v5`      | [Go docs](../go/installation.md)            |
| JVM (Java, Kotlin, Scala)      | [`io.github.goplasmatic:datalogic`](https://central.sonatype.com/artifact/io.github.goplasmatic/datalogic) | Maven Central dependency                                  | [Java / Kotlin docs](../jvm.md)          |
| .NET                           | [`Goplasmatic.Datalogic`](https://www.nuget.org/packages/Goplasmatic.Datalogic)                  | `dotnet add package Goplasmatic.Datalogic`                       | [.NET docs](../dotnet.md)    |
| PHP                            | [`goplasmatic/datalogic`](https://packagist.org/packages/goplasmatic/datalogic)                  | `composer require goplasmatic/datalogic`                         | [PHP docs](../php.md)          |
| React (visual debugger)        | [`@goplasmatic/datalogic-ui`](https://www.npmjs.com/package/@goplasmatic/datalogic-ui)           | `npm i @goplasmatic/datalogic-ui`                                | [React docs](../react-ui/installation.md)                              |

Building the WASM binding from source:

```bash
cd bindings/wasm
./build.sh
```

## Minimum Rust Version

datalogic-rs v5 uses **Rust edition 2024** — Rust **1.85** or later is
required. The crate is built with `#![forbid(unsafe_code)]`.

## Verifying Installation

Create a simple script or test file to verify everything works:

<div class="codetabs">

```rust
// main.rs
fn main() {
    let result = datalogic_rs::eval_str(r#"{"+": [1, 2]}"#, r#"{}"#).unwrap();
    println!("1 + 2 = {}", result);
    assert_eq!(result, "3");
}
// Run in terminal: cargo run
```

```javascript
// index.js
import { apply } from '@goplasmatic/datalogic-node';

const result = apply({ '+': [1, 2] }, {});
console.log(`1 + 2 = ${result}`); // 1 + 2 = 3
// browser/edge: same API via @goplasmatic/datalogic-wasm, see the WASM chapter
```

```python
# test.py
from datalogic_py import apply

result = apply({"+": [1, 2]}, {})
print(f"1 + 2 = {result}") # 1 + 2 = 3.0
```

```go
// main.go
package main

import (
    "fmt"
    datalogic "github.com/GoPlasmatic/datalogic-rs/bindings/go/v5"
)

func main() {
    result, _ := datalogic.Apply(`{"+": [1, 2]}`, `{}`)
    fmt.Printf("1 + 2 = %s\n", result) // 1 + 2 = 3
}
```

```java
// Main.java
import com.goplasmatic.datalogic.Engine;

public class Main {
    public static void main(String[] args) {
        try (Engine engine = new Engine()) {
            String result = engine.apply("{\"+\": [1, 2]}", "{}");
            System.out.println("1 + 2 = " + result); // 1 + 2 = 3
        }
    }
}
```

```csharp
// Program.cs
using Goplasmatic.Datalogic;

using var engine = new Engine();
var result = engine.Apply("""{"+": [1, 2]}""", "{}");
Console.WriteLine($"1 + 2 = {result}"); // 1 + 2 = 3
```

```php
<?php // test.php
require 'vendor/autoload.php';

use Goplasmatic\Datalogic\Engine;

$engine = new Engine();
echo "1 + 2 = " . $engine->apply('{"+": [1, 2]}', '{}'); // 1 + 2 = 3
```

</div>
