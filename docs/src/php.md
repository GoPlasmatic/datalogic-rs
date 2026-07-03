# PHP (FFI)

The PHP binding `goplasmatic/datalogic` uses PHP's native FFI extension (`ext-ffi`) to interact with the shared C ABI. It requires **PHP 8.4 or newer**.

## Installation

Add the Composer dependency to your project:

```bash
composer require goplasmatic/datalogic
```

Ensure that PHP's FFI extension is enabled in your `php.ini` configuration:

```ini
extension=ffi
# For command line tools and web servers, allow FFI
ffi.enable=true
```

The Composer package ships with precompiled shared libraries under `lib/<os>-<arch>/`. The loader automatically detects and loads the library for the current platform.

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

Always compile rules when executing them repeatedly. This parses the rule into optimized bytecode on the Rust side:

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

In PHP, FFI-allocated memory is released when wrapper objects go out of scope and are collected by the PHP engine. However, in long-running environments (such as PHP-FPM, Swoole, RoadRunner, or CLI daemons), garbage collection delays can accumulate heap usage. 

To guarantee immediate cleanup, call `$object->close()` explicitly on the `Engine`, `Rule`, or `Session` wrappers.

## Going deeper

- [C ABI internals: memory management & thread safety](c-abi.md) — the native-heap ownership rules every FFI binding shares
- [Engine configuration semantics](advanced/configuration.md)
- [Package README on Packagist](https://github.com/GoPlasmatic/datalogic-rs/tree/main/bindings/php#readme) — full API surface, error types, and platform table
