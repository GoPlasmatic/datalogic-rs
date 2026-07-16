# goplasmatic/datalogic

[![Packagist](https://img.shields.io/packagist/v/goplasmatic/datalogic)](https://packagist.org/packages/goplasmatic/datalogic)
[![CI](https://github.com/GoPlasmatic/datalogic-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/GoPlasmatic/datalogic-rs/actions/workflows/ci.yml)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

Part of [datalogic-rs](https://github.com/GoPlasmatic/datalogic-rs) — one engine, every runtime.

PHP bindings for [datalogic-rs](https://github.com/GoPlasmatic/datalogic-rs),
the JSONLogic rules engine with one Rust core and official bindings for
Rust, Node.js, the browser (WASM), Python, Go, Java, .NET, and PHP. Same
rules, same semantics: every binding runs the same core and passes the
same 1,553-case conformance battery (54 suites). Compile once, evaluate
many, natively in PHP.

For the cross-runtime overview and the API-tier model every binding
implements, see the
[repo README](https://github.com/GoPlasmatic/datalogic-rs#readme).

> **New in v5.** This package is new: there is no v4 PHP artifact. If
> you are coming from the v4 Rust crate or the v4
> `@goplasmatic/datalogic` WASM package, the engine's v4 → v5 changes
> are catalogued in
> [MIGRATION.md](https://github.com/GoPlasmatic/datalogic-rs/blob/main/MIGRATION.md).

## Install

```bash
composer require goplasmatic/datalogic
```

Requires PHP 8.4+ with `ext-ffi` enabled. The binding is a PHP FFI
wrapper over the engine's C ABI; the Composer package ships the native
library under `lib/<os>-<arch>/` for every supported platform, and the
FFI loader picks the right one at runtime. No Rust toolchain needed.

| Platform | Architectures   |
|----------|-----------------|
| Linux    | x86_64, aarch64 |
| macOS    | x86_64, arm64   |
| Windows  | x86_64, arm64   |

## Quick start

```php
use Goplasmatic\Datalogic\Engine;

$engine = new Engine();
echo $engine->apply('{"+":[1,2]}', '{}');  // "3"
```

Rules, data, and results cross the boundary as JSON strings; use
`json_encode` / `json_decode` at the edges.

## Compile once, evaluate many

Compile the rule once when you'll evaluate it against many data inputs:

```php
$engine = new Engine();
$rule = $engine->compile('{"var":"x"}');
foreach ([1, 2, 3] as $x) {
    echo $rule->evaluate(json_encode(['x' => $x])), "\n";
}
```

`Engine` and compiled `Rule` objects carry no per-call state: build and
compile once per process and reuse them across requests. Sessions
(below) hold a mutable arena, so give each evaluation loop its own.

## Data handles (parse once, evaluate many)

When one payload feeds many evaluations, parse it once into a
`DataHandle` and pass the handle wherever a JSON string is accepted —
the per-call JSON parse disappears entirely:

```php
use Goplasmatic\Datalogic\DataHandle;

$data = new DataHandle('{"user":{"age":42,"plan":"pro"}}');
$rule->evaluate($data);              // same result as the string overload
$session->evaluate($rule, $data);    // hot path: zero parse work per call
```

Handles are immutable and engine-independent — one handle can feed
rules compiled by different engines, any number of times (evaluation
never consumes it). The native memory is released when the object is
GC'd, or eagerly via `close()`; `allocatedBytes()` reports the handle's
resident size.

## Typed evaluations

Sessions can return native PHP scalars instead of JSON strings —
handy for predicates (feature flags, routing) where decoding JSON per
call is pure overhead. All four take a compiled `Rule` and a
`DataHandle`:

```php
$session->evaluateBool($rule, $data);    // bool   — strict: JSON true/false only
$session->evaluateInt($rule, $data);     // int    — exact integers only
$session->evaluateFloat($rule, $data);   // float  — any JSON number
$session->evaluateTruthy($rule, $data);  // bool   — JSONLogic truthiness, never mismatches
```

The strict variants throw `EvaluateException` with
`$errorType === "TypeMismatch"` when the result is of any other type;
`evaluateTruthy` collapses any result through the engine's configured
truthiness rules (the same coercion `if`/`and`/`or` apply).

## Batch evaluation

Evaluate one rule against many payloads (`evaluateBatch`) or many
rules against one payload (`evaluateMany`, the rule-set / feature-flag
shape) in a single native call. Results come back in input order; a
failed item puts a `BatchItemError` in its slot instead of aborting
the other N-1 — item failures never throw:

```php
use Goplasmatic\Datalogic\BatchItemError;

$results = $session->evaluateBatch($rule, $handles);   // list<DataHandle> in
$results = $session->evaluateMany($rules, $data);      // list<Rule> in

foreach ($results as $i => $r) {
    if ($r instanceof BatchItemError) {
        error_log("item {$i} failed: {$r->tag}: {$r->message}");
        continue;
    }
    // $r is the item's JSON-string result
}
```

`BatchItemError` exposes `$status` (the raw C-ABI status code), `$tag`
(stable engine tag, e.g. `"Thrown"`, `"NaN"`), `$message`, and
`$operator` (outermost failing operator, when known).

## Sessions (hot loops)

A `Session` reuses one arena across evaluations and resets it at the
start of every call, so peak memory stays bounded:

```php
$session = $engine->openSession();
foreach ($inputs as $data) {
    $result = $session->evaluate($rule, $data);
}
```

Native handles are released by PHP's destructor when the wrapper object
goes out of scope; every wrapper type also exposes an explicit
`close()` for early release.

## API surface

The binding mirrors the Rust engine's
[API tier model](https://github.com/GoPlasmatic/datalogic-rs#one-api-shape-every-binding).
Every method takes and returns JSON strings.

| Tier            | Entry point                                                    | Use when                                              |
|-----------------|----------------------------------------------------------------|-------------------------------------------------------|
| One-shot        | `$engine->apply($rule, $data)`                                 | Ad-hoc evaluation, one rule + one data shape          |
| Engine + config | `new Engine($templating)` / `Engine::builder()…->build()`      | Templating mode, custom operators, evaluation config  |
| Compile once    | `$engine->compile($rule)` → `$rule->evaluate($data)`           | Same rule evaluated against many data inputs          |
| Parse once      | `new DataHandle($json)` → pass instead of a JSON string        | Same payload evaluated by many rules/calls            |
| Session         | `$engine->openSession()` → `$session->evaluate($rule, $data)`  | Hot loops: amortise arena reset across iterations     |
| Typed           | `$session->evaluateBool/Int/Float/Truthy($rule, $handle)`      | Predicates: native scalars, no JSON decode per call   |
| Batch           | `$session->evaluateBatch($rule, $handles)` / `evaluateMany($rules, $handle)` | Many evaluations per FFI crossing, per-item errors |
| Traced          | `$engine->openTracedSession()` → `$session->evaluate($rule, $data)` | Step-by-step debugging; feeds the React debugger |

`Rule::evaluate` and `Session::evaluate` accept either a JSON string or
a `DataHandle`.

## Custom operators

Register PHP-implemented operators through the builder. Each callback
receives the operator's pre-evaluated arguments as a JSON-array string
and returns a JSON-value string; throwing signals an evaluation error
whose message bubbles back to the caller.

```php
$engine = Engine::builder()
    ->addOperator('double', function (string $argsJson): string {
        $args = json_decode($argsJson, true);
        return (string) ((int) $args[0] * 2);
    })
    ->build();
echo $engine->apply('{"double":[21]}', '{}');  // "42"
```

**Built-ins win**: a custom registration of a built-in name (`+`, `if`,
`var`, ...) never dispatches at evaluation time; the built-in always
runs.

## Engine configuration

`Engine::builder()->setConfigJson($json)` sets the evaluation semantics
from a JSON object string: an optional `preset` plus per-field
overrides. Unknown keys or values throw `EvaluateException` (error type
`ConfigurationError`), so typos fail loudly:

```php
$lenient = Engine::builder()
    ->setConfigJson('{"division_by_zero":"return_null"}')
    ->build();
echo $lenient->apply('{"/":[1.5,0]}', '{}');  // "null"

$strict = Engine::builder()
    ->setConfigJson('{"preset":"strict"}')
    ->build();
$strict->apply('{"+":["",1]}', '{}');         // throws: strict rejects non-numeric coercion
```

| Key | Values |
|-----|--------|
| `preset` | `"default"`, `"safe_arithmetic"`, `"strict"` |
| `arithmetic_nan_handling` | `"throw_error"`, `"ignore_value"`, `"coerce_to_zero"`, `"return_null"` |
| `division_by_zero` | `"return_saturated"`, `"throw_error"`, `"return_null"`, `"return_infinity"` |
| `loose_equality_errors` | `bool` |
| `truthy_evaluator` | `"javascript"`, `"python"`, `"strict_boolean"` |
| `numeric_coercion` | object of bools: `empty_string_to_zero`, `null_to_zero`, `bool_to_number`, `reject_non_numeric` |
| `max_recursion_depth` | integer >= 1 |

The `preset` applies first; the remaining keys override individual
fields on top of it. Every binding shares this JSON schema and parses it
with the same core code, so a config that works here works in the
Python, Node, and WASM bindings too. The full semantics of each knob are
documented on the Rust crate's
[`EvaluationConfig`](https://docs.rs/datalogic-rs/latest/datalogic_rs/struct.EvaluationConfig.html).

## Error handling

Everything the binding throws extends
`Goplasmatic\Datalogic\Exception\DatalogicException` (a
`RuntimeException`):

| Exception           | When                                                      |
|---------------------|-----------------------------------------------------------|
| `ParseException`    | Malformed rule or data JSON, or an unsupported operator   |
| `EvaluateException` | Operator failure at runtime, or a rejected engine config  |

The structured fields ride on the base class as public readonly
properties: `$errorType` is the stable engine tag (e.g. `"ParseError"`,
`"Thrown"`, `"NaN"`), `$operatorName` the outermost failing operator
(e.g. `"+"`), and `$pathJson` the root-to-leaf error path as a JSON
array; each is `null` when not applicable.

```php
use Goplasmatic\Datalogic\Exception\EvaluateException;

try {
    $engine->apply('{"+":["x",1]}', '{}');  // arithmetic on a non-numeric string
} catch (EvaluateException $e) {
    echo $e->errorType;     // runtime error tag, e.g. "Thrown", "NaN"
    echo $e->operatorName;  // "+"
    echo $e->pathJson;      // JSON-array path through the compiled tree
}
```

Under the hood the binding targets the engine's C ABI **v2**: every
fallible native call returns a status code plus an owned error handle,
and the binding asserts `datalogic_abi_version() == 2` the first time
the library is loaded — a stale native library fails loudly at startup
(`RuntimeException`), never mid-request.

## Threading

| Type         | Pattern                                         |
|--------------|-------------------------------------------------|
| `Engine`     | Build once per process; reuse across requests   |
| `Rule`       | Compile once per process; reuse across requests |
| `DataHandle` | Immutable; share freely across engines/rules    |
| `Session`    | One per evaluation loop; do not share           |

PHP is single-threaded per request, so `Engine`, `Rule`, `Session`, and
`TracedSession` are all safe in that model.

Custom operators use PHP FFI's auto-coercion of PHP callables to C
function pointers. The builder retains the callable for the engine's
lifetime; releasing the engine releases the pin.

## Tracing

```php
$session = $engine->openTracedSession();
$run = $session->evaluate('{"+":[{"var":"x"},1]}', '{"x":41}');
echo $run->result;             // 42
echo count($run->steps);       // executed node count
```

Same trace envelope as every other binding; the
[React debugger](https://github.com/GoPlasmatic/datalogic-rs/tree/main/ui)
consumes it directly. `TracedRun` exposes `$result`, `$expressionTree`,
`$steps`, `$error`, and `$structuredError` (plus `isSuccess()`); runtime
failures surface inside the run rather than as exceptions. Tracing
disables the optimizer so every operator appears in the trace: use it
for debugging, not hot paths.

## Performance

<!-- canonical-bench v5.0 -->
Geomean across 50 operator benchmark suites (Apple M2 Pro, median of 3 runs; pairwise shared-suite ratios per the [methodology](https://github.com/GoPlasmatic/datalogic-rs/blob/main/tools/benchmark/BENCHMARK.md)): the native Rust core evaluates at **8.9 ns/op**, 7.9× faster than json-logic-engine (compiled, the fastest JS engine), 30.6× faster than jsonlogic-rs (the closest Rust alternative), and 104.2× faster than the json-logic-js reference implementation. The WASM build under Node measures 901.1 ns geomean (101× native); on Node servers, prefer `@goplasmatic/datalogic-node`.

The PHP FFI boundary adds a small per-call marshalling cost on top of
the core numbers.

## Preloading (opcache.preload + FFI)

By default the binding lazily calls `FFI::cdef` on first use, which
works out of the box on the CLI. Production FPM/web SAPIs should use
[FFI preloading](https://www.php.net/manual/en/ffi.configuration.php)
instead: PHP's default `ffi.enable=preload` forbids runtime `FFI::cdef`
outside the CLI, and preloading also moves all header parsing to server
start. The package ships a ready-made preload script:

```ini
; php.ini
opcache.preload=/path/to/vendor/goplasmatic/datalogic/preload.php
opcache.preload_user=www-data
ffi.enable=preload
```

`preload.php` resolves the native library exactly like the runtime
loader does (see [Building from source](#building-from-source)),
rewrites the `FFI_LIB` line of the bundled header
(`src/datalogic-ffi.h`) to that absolute path, and registers the
persistent FFI scope `"datalogic"` via `FFI::load`. At request time the
binding finds the scope through `FFI::scope("datalogic")` and skips
`FFI::cdef` entirely; when no scope is preloaded it falls back to
`FFI::cdef` transparently. To pin a specific library build, set the
`DATALOGIC_NATIVE_LIB` environment variable before the server starts.

If your application already has a preload script, `require` the
package's `preload.php` from it (it is idempotent), or call
`\Goplasmatic\Datalogic\Internal\Native::preload()` directly.
Power users who manage their own headers can copy `src/datalogic-ffi.h`,
hard-code `FFI_LIB` to their library path, and `FFI::load` it
themselves — the committed default (`libdatalogic_c.so`) resolves via
the OS loader path. The header is also the single source of the cdef
declarations (Native.php reads it with the `#define` lines stripped),
so the two load paths cannot drift apart.

## Building from source

The binding lives in
[`bindings/php/`](https://github.com/GoPlasmatic/datalogic-rs/tree/main/bindings/php).
The FFI loader searches for the cdylib in order: the
`DATALOGIC_NATIVE_LIB` env var, the package's `lib/<os>-<arch>/` layout,
the in-tree C ABI target dir, then the OS's default loader paths. So a
fresh clone needs the C ABI built once:

```bash
git clone https://github.com/GoPlasmatic/datalogic-rs
cd datalogic-rs/bindings/c && cargo build --release
cd ../php
composer install
vendor/bin/phpunit
```

## Learn more

- [datalogic-rs repository](https://github.com/GoPlasmatic/datalogic-rs#readme)
- [Rust crate deep-dive](https://github.com/GoPlasmatic/datalogic-rs/tree/main/crates/datalogic-rs#readme)
- [PHP docs chapter](https://goplasmatic.github.io/datalogic-rs/php.html)
- [Online playground](https://goplasmatic.github.io/datalogic-rs/playground/)
- [JSONLogic specification](https://jsonlogic.com)
- [C ABI internals](https://github.com/GoPlasmatic/datalogic-rs/tree/main/bindings/c#readme)

## License

Apache-2.0. See the
[main repository](https://github.com/GoPlasmatic/datalogic-rs) for
source and contribution guidelines.
