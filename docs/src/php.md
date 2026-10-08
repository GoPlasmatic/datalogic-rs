# PHP (FFI)

The PHP binding `goplasmatic/datalogic` calls the shared C ABI through PHP's FFI extension (`ext-ffi`). It requires **PHP 8.4 or newer**.

## Installation

Add the Composer dependency to your project:

```bash
composer require goplasmatic/datalogic
```

Composer records the constraint `^5.8`.

Enable PHP's FFI extension in your `php.ini` configuration. The binding supports two setups:

```ini
extension=ffi
; Simplest setup (CLI tools, or any SAPI): allow runtime FFI::cdef
ffi.enable=true
```

For PHP-FPM and other web SAPIs, PHP's default `ffi.enable=preload` forbids runtime `FFI::cdef` outside the CLI, so preload the package's FFI scope instead (it also moves header parsing to server start):

```ini
extension=ffi
opcache.preload=/path/to/vendor/goplasmatic/datalogic/preload.php
opcache.preload_user=www-data
ffi.enable=preload
```

The binding looks for the preloaded `FFI::scope("datalogic")` first and falls back to `FFI::cdef` when no scope is registered. Details are in the [Preloading section of the PHP README](https://github.com/GoPlasmatic/datalogic-rs/tree/main/bindings/php#preloading-opcachepreload--ffi).

The Composer package ships precompiled shared libraries under `lib/<os>-<arch>/` for Linux, macOS and Windows on x86_64 and arm64, and the loader picks the one for the current platform. Set the `DATALOGIC_NATIVE_LIB` environment variable to an absolute path to load a library you built yourself.

## Quick Start

### One-Shot Evaluation

```php
<?php

use Goplasmatic\Datalogic\Engine;

$engine = new Engine();
$result = $engine->apply('{"+": [1, 2, 3]}', '{}');
echo $result; // "6"
```

### Reusable Compiled Rules

Compile rules that you evaluate repeatedly. Compiling parses and optimizes the rule once on the Rust side:

```php
<?php

use Goplasmatic\Datalogic\Engine;

$engine = new Engine();
$rule = $engine->compile('{"if": [{ ">": [{"var": "score"}, 50] }, "pass", "fail"]}');

echo $rule->evaluate(json_encode(['score' => 75])), "\n"; // "pass"
echo $rule->evaluate(json_encode(['score' => 30])), "\n"; // "fail"

// Close the handles to free native memory now instead of at garbage collection
$rule->close();
$engine->close();
```

### Arena Recycling with `Session`

To recycle memory allocations in hot loops, open a `Session`:

```php
<?php

use Goplasmatic\Datalogic\Engine;

$engine = new Engine();
$rule = $engine->compile('{"var": "user.name"}');

$session = $engine->openSession();
foreach ($dataset as $input) {
    // Reuses the session's arena, reset at the start of each call
    $name = $session->evaluate($rule, $input);
    echo $name, "\n";
}

$session->close();
$rule->close();
$engine->close();
```

## Checking and Inspecting Rules

The engine can report problems in a rule before it runs, and describe a compiled rule:

```php
<?php

use Goplasmatic\Datalogic\Engine;
use Goplasmatic\Datalogic\Exception\ParseException;

$engine = new Engine();

echo $engine->check('{"if": [true, {"vr": "x"}]}');
// [{"code":"UnknownOperator","severity":"error",
//   "message":"unknown operator `vr`; did you mean `var`?","pointer":"/if/1","operator":"vr"}]

try {
    $rule = $engine->compileChecked($ruleJson);
    echo $rule->facts();  // {"reads":[["user","age"]], "operators":[...], "deterministic":true, ...}
} catch (ParseException $e) {
    echo $e->diagnosticsJson;  // errorType "CompileError": every problem, each with a JSON Pointer
}
```

| Method | Returns |
|--------|---------|
| `$engine->check($rule, $mode = Native::MODE_ENGINE)` | Every problem the engine can see, as a JSON array of `{code, severity, message, pointer, operator}`; it does not throw for a bad rule |
| `$engine->compileChecked($rule)` | A `Rule`, or `ParseException` with `$errorType === "CompileError"` if `check` finds an error |
| `$engine->compileTemplate($rule)` / `compileStrict($rule)` | A `Rule` compiled with templating on or off, whatever the engine's own mode |
| `$rule->facts()` | The data paths the rule reads, the operators it calls, and whether it is deterministic, as JSON |
| `$engine->operators()` | The operator catalogue, in the schema of [`operators.json`](https://github.com/GoPlasmatic/datalogic-rs/blob/main/docs/src/operators/operators.json) |
| `$engine->truthy($valueJson)` | Whether a JSON value is truthy under the engine's configured truthiness |

`Engine::builder()` also takes `withFamilies('ExtString', ...)`, which keeps the engine to the JSONLogic core plus the operator families you name, and `withStrictOperatorNames(true)`, which makes `addOperator` throw for a name a built-in operator answers to. The engine config accepts `"missing_var": "error"`, which turns a `var` read that finds nothing into a `VariableNotFound` error.

## Memory Management

PHP releases FFI-allocated memory when wrapper objects go out of scope and the PHP engine collects them. In long-running environments (such as PHP-FPM, Swoole, RoadRunner, or CLI daemons), garbage collection delays can let heap usage build up.

To free native memory at a known point, call `$object->close()` on the `Engine`, `Rule`, `Session`, `TracedSession` or `DataHandle` wrapper. A `Rule`, `Session` or `TracedSession` keeps its engine (and the engine's custom operators) alive, so closing the engine does not break them. Each native handle has one owner: wrapping a handle that a wrapper already owns throws `InvalidArgumentException`.

## Going deeper

- [C ABI internals: memory management & thread safety](c-abi.md): the native-heap ownership rules every FFI binding shares
- [Engine configuration semantics](advanced/configuration.md)
- [Rule analysis](advanced/rule-analysis.md): diagnostic codes, the facts fields and the operator catalogue
- [Package README on Packagist](https://github.com/GoPlasmatic/datalogic-rs/tree/main/bindings/php#readme): full API surface, error types, and platform table
