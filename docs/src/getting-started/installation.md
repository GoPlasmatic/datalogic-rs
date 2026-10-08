# Installation

## Adding to Your Project

Pick your language. Each package carries the core crate's version number (5.8.1 for this release):

<div class="codetabs">

```rust
// Cargo.toml
[dependencies]
datalogic-rs = "5"

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
    <version>5.8.1</version>
</dependency>

// Gradle: build.gradle.kts
implementation("io.github.goplasmatic:datalogic:5.8.1")
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

The Rust crate builds without `serde_json` by default: the main entry
points (`Engine::eval_str`, `Engine::compile(&str)`,
`datalogic_rs::eval_str`) take and return JSON text. Add the `serde_json`
feature if you need `serde_json::Value` interop or the typed
`eval_into::<T>` paths.

## Feature Flags

The crate is a small core plus opt-in features. With `default = []` you get
the 33 JSONLogic core operators and no optional dependencies:

| Feature | Default | What it adds |
|---------|---------|-------------|
| `serde_json` | off | `&serde_json::Value` interop (as `EvalInput` / `IntoLogic`) and the typed `eval_into::<T>` paths on `Engine`, `Session`, and the module-level helpers. Pulls in `serde_json` as a runtime dependency. |
| `templating` | off | Templating mode: engine-wide with `Engine::builder().with_templating(true).build()`, or for one rule with `Engine::compile_template`. |
| `datetime` | off | `datetime`, `timestamp`, `parse_date`, `format_date`, `date_diff`, `now` operators, including the optional trailing IANA-zone argument on `format_date` / `parse_date` (pulls in `chrono` and `chrono-tz`). |
| `trace` | off | Per-evaluation execution tracing (`engine.trace()…`). Transitively enables `serde_json`. |
| `ext-string` | off | Extended string operators (`length`, `starts_with`, `ends_with`, `upper`, `lower`, `trim`, `split`). |
| `ext-array` | off | Extended array operators (`sort`, `slice`, `group_by`, `distinct`). |
| `ext-object` | off | Object take-apart operators (`keys`, `values`, `entries`). |
| `ext-control` | off | Extended control-flow operators (`exists`, `??`, `switch`/`match`, `type`). |
| `error-handling` | off | `try` / `throw` operators. |
| `ext-math` | off | Extended math operators (`abs`, `ceil`, `floor`). |
| `flagd` | off | [OpenFeature flagd-compatible](https://flagd.dev/reference/custom-operations/) `fractional` (murmurhash3 percentage bucketing) and `sem_ver` (semantic-version comparison) operators. |
| `wasm-clock` | off | JS-host clock for the `now` operator on `wasm32-unknown-unknown` (browsers, Node, Deno, Workers); combine with `datetime`. It forwards to `chrono/wasmbind`, whose JS imports fail to instantiate in non-JS wasm runtimes such as wasmtime, wazero and Chicory, so leave it off there (on WASI the OS clock works without it). |
| `tensor` | off | The `Tensor` value (dtype, shape and a row-major byte buffer) and 20 marshalling-only operators over it, for turning JSON into a model's inputs and its outputs back into JSON. Constructors refuse more than 2^28 elements. No new dependency. |
| `tensor-half` | off | Lifts the `f16` / `bf16` restriction on the element-wise tensor operators. Implies `tensor`; pulls in `half`. |
| `budget` | off | A per-evaluation operation counter with a hard abort (`EvaluationConfig::ops_budget`, `Engine::evaluate_metered`, `Session::eval_metered`, `ErrorKind::BudgetExceeded`). |
| `all-operators` | off | Every operator family at once: `datetime`, `error-handling`, the five `ext-*` families, `flagd` and `tensor`. A family added in a later release joins it. It does not include `serde_json`, `templating`, `trace`, `budget`, `tensor-half` or `wasm-clock`. |

Example: opt into `serde_json::Value` interop plus templating:

```toml
[dependencies]
datalogic-rs = { version = "5", features = ["serde_json", "templating"] }
serde_json = "1.0"
```

Example: every operator, plus `serde_json` interop and templating (the
official bindings use this set, plus `trace` and `budget`):

```toml
[dependencies]
datalogic-rs = { version = "5", features = ["all-operators", "serde_json", "templating"] }
```

A Cargo feature decides what is compiled in. To give one engine fewer
operators than the build has, list the families it keeps with
`Engine::builder().with_families(..)` (`families` in the bindings); see
[Configuration](../advanced/configuration.md#operator-families).

## Version Selection

- **v5.x** (current, 5.8.1): string-based API, opt-in `serde_json`, builder-only operator registration. v4 code has no compatibility shim in v5, so plan a single cutover.
- **v4.x**: `DataLogic` engine with a `serde_json::Value`-first API. No longer the active line.
- **v3.x**: the arena-based engine that v4 replaced.

If you're upgrading from v4, or from an earlier 5.x release, see the [Migration Guide](../migration.md).

## Other languages

The Rust crate is the engine; each other language has its own binding
over it. The binding's guide covers install details and the API in that
language:

| Language                       | Package                                                                                          | Install                                                          | Deep-dive                                                                                                       |
|--------------------------------|--------------------------------------------------------------------------------------------------|------------------------------------------------------------------|-----------------------------------------------------------------------------------------------------------------|
| Node.js (native, napi-rs)      | [`@goplasmatic/datalogic-node`](https://www.npmjs.com/package/@goplasmatic/datalogic-node)       | `npm i @goplasmatic/datalogic-node`                              | [Node.js docs](../nodejs/overview.md)        |
| JavaScript / TypeScript (WASM) | [`@goplasmatic/datalogic-wasm`](https://www.npmjs.com/package/@goplasmatic/datalogic-wasm)       | `npm i @goplasmatic/datalogic-wasm`                              | [JS / TS docs](../javascript/installation.md)        |
| Python                         | [`datalogic-py`](https://pypi.org/project/datalogic-py/)                                         | `pip install datalogic-py`                                       | [Python docs](../python/installation.md)    |
| Go                             | `datalogic-go`                                                                                   | `go get github.com/GoPlasmatic/datalogic-rs/bindings/go/v5`      | [Go docs](../go/installation.md)            |
| JVM (Java, Kotlin, Scala)      | [`io.github.goplasmatic:datalogic`](https://central.sonatype.com/artifact/io.github.goplasmatic/datalogic) | `io.github.goplasmatic:datalogic:5.8.1` (Maven Central, JDK 22+) | [Java / Kotlin docs](../jvm.md)          |
| .NET                           | [`Goplasmatic.Datalogic`](https://www.nuget.org/packages/Goplasmatic.Datalogic)                  | `dotnet add package Goplasmatic.Datalogic`                       | [.NET docs](../dotnet.md)    |
| PHP                            | [`goplasmatic/datalogic`](https://packagist.org/packages/goplasmatic/datalogic)                  | `composer require goplasmatic/datalogic`                         | [PHP docs](../php.md)          |
| C ABI (any FFI host)           | `datalogic-c` (in-tree, not published)                                                           | build `bindings/c` from source                                   | [C ABI docs](../c-abi.md)                                                                                       |
| React (visual debugger)        | [`@goplasmatic/datalogic-ui`](https://www.npmjs.com/package/@goplasmatic/datalogic-ui)           | `npm i @goplasmatic/datalogic-ui`                                | [React docs](../react-ui/installation.md)                              |

Building the WASM binding from source:

```bash
cd bindings/wasm
./build.sh
```

## Minimum Rust Version

datalogic-rs requires Rust **1.98** or later (the `rust-version` in its
`Cargo.toml`). The floor comes from the `datavalue-rs` 0.3 dependency,
which declares 1.98; the crate's own code uses edition 2024 and needs
less. The crate sets `#![forbid(unsafe_code)]`.

## Verifying Installation

Run a one-line evaluation to confirm the package loads:

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
print(f"1 + 2 = {result}") # 1 + 2 = 3
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
