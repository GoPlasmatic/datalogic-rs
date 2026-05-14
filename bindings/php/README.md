# `goplasmatic/datalogic` — PHP binding for [`datalogic-rs`](../../crates/datalogic-rs)

PHP FFI wrapper over the shared [`bindings/c`](../c) C ABI. Requires
PHP 8.4+ with `ext-ffi` enabled (the floor tracks `composer.json` — the
test suite uses PHPUnit 13, which itself requires PHP 8.4+).

## Install

```bash
composer require goplasmatic/datalogic
```

The composer package ships platform binaries under `lib/<os>-<arch>/`;
the FFI loader (`Goplasmatic\Datalogic\Internal\Native`) picks the right
one at runtime.

## Quick start

```php
use Goplasmatic\Datalogic\Engine;

$engine = new Engine();
echo $engine->apply('{"+":[1,2]}', '{}');  // "3"
```

Reusing a compiled rule:

```php
$engine = new Engine();
$rule = $engine->compile('{"var":"x"}');
foreach ([1, 2, 3] as $x) {
    echo $rule->evaluate(json_encode(['x' => $x])), "\n";
}
```

Hot-loop session (arena reuse):

```php
$session = $engine->openSession();
foreach ($inputs as $data) {
    $result = $session->evaluate($rule, $data);
}
```

Traced evaluation:

```php
$session = $engine->openTracedSession();
$run = $session->evaluate('{"+":[{"var":"x"},1]}', '{"x":41}');
echo $run->result;             // 42
echo count($run->steps);       // executed node count
```

Custom operator:

```php
$engine = Engine::builder()
    ->addOperator('double', function (string $argsJson): string {
        $args = json_decode($argsJson, true);
        return (string) ((int) $args[0] * 2);
    })
    ->build();
echo $engine->apply('{"double":[21]}', '{}');  // "42"
```

## Build & test (development)

The Native loader searches for the cdylib in this order:

1. `DATALOGIC_NATIVE_LIB` env var (absolute path).
2. `bindings/php/lib/<os>-<arch>/lib...` — populated by the release
   workflow.
3. `bindings/c/target/release/lib...` — for in-tree dev.
4. The OS's default loader paths (`LD_LIBRARY_PATH` /
   `DYLD_LIBRARY_PATH` / `PATH`).

So a fresh clone needs the C ABI built once:

```bash
cd ../c && cargo build --release
cd ../php
composer install
vendor/bin/phpunit
```

## Threading & memory

- PHP is single-threaded per request — `Engine`, `Rule`, `Session`,
  `TracedSession` are all safe in that model.
- The native handles are released by PHP's destructor when the wrapper
  object goes out of scope; explicit `close()` is also available for
  early release.
- Custom operators use PHP FFI's auto-coercion of PHP callables to C
  function pointers. The builder retains the callable for the engine's
  lifetime; releasing the engine releases the pin.

## Layout

```
bindings/php/
├── composer.json
├── phpunit.xml
├── src/
│   ├── Engine.php
│   ├── EngineBuilder.php
│   ├── Rule.php
│   ├── Session.php
│   ├── TracedSession.php
│   ├── TracedRun.php
│   ├── Exception/
│   │   ├── DatalogicException.php
│   │   ├── ParseException.php
│   │   └── EvaluateException.php
│   └── Internal/
│       └── Native.php           # FFI::cdef loader + signature list
├── lib/                         # populated at release time
│   └── <os>-<arch>/...
└── tests/
    └── EngineTest.php           # PHPUnit
```
