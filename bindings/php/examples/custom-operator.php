<?php

declare(strict_types=1);

// custom-operator: register a PHP `double` operator and call it from a rule.
// Custom operators receive their pre-evaluated arguments as a JSON-array
// string and return a JSON-value string. Built-in names always win.
//
// Run from bindings/php/ (needs `composer install` once, plus the C ABI cdylib:
// `cargo build --release --manifest-path ../c/Cargo.toml`):
//   php examples/custom-operator.php

use Goplasmatic\Datalogic\Engine;

require __DIR__ . '/../vendor/autoload.php';

$engine = Engine::builder()
    ->addOperator('double', function (string $argsJson): string {
        $args = json_decode($argsJson, true);
        return json_encode($args[0] * 2);
    })
    ->build();

echo $engine->apply('{"double": [21]}', '{}'), PHP_EOL; // 42
