# PHP (FFI)

The PHP binding `goplasmatic/datalogic` uses PHP's native FFI extension (`ext-ffi`) to interact with the shared C ABI. It requires **PHP 8.4 or newer**.

## Installation

Add the Composer dependency to your project:

```bash
composer require goplasmatic/datalogic
```

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

The Composer package ships with precompiled shared libraries under `lib/<os>-<arch>/`. The loader detects the current platform and loads its library.

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

Compile rules that you evaluate repeatedly. Compiling parses the rule once into optimized bytecode on the Rust side:

```php
<?php

use Goplasmatic\Datalogic\Engine;

$engine = new Engine();
$rule = $engine->compile('{"if": [{ ">": [{"var": "score"}, 50] }, "pass", "fail"]}');

echo $rule->evaluate(json_encode(['score' => 75])), "\n"; // "pass"
echo $rule->evaluate(json_encode(['score' => 30])), "\n"; // "fail"

// Explicitly close resources to free native handles early
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
    // Reuses the session's internal memory arena
    $name = $session->evaluate($rule, $input);
    echo $name, "\n";
}

$session->close();
$rule->close();
$engine->close();
```

## Memory Management

PHP releases FFI-allocated memory when wrapper objects go out of scope and the PHP engine collects them. In long-running environments (such as PHP-FPM, Swoole, RoadRunner, or CLI daemons), garbage collection delays can let heap usage build up.

To guarantee immediate cleanup, call `$object->close()` explicitly on the `Engine`, `Rule`, or `Session` wrappers.

## Going deeper

- [C ABI internals: memory management & thread safety](c-abi.md): the native-heap ownership rules every FFI binding shares
- [Engine configuration semantics](advanced/configuration.md)
- [Package README on Packagist](https://github.com/GoPlasmatic/datalogic-rs/tree/main/bindings/php#readme): full API surface, error types, and platform table
