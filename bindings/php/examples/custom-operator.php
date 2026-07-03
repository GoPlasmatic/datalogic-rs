<?php

declare(strict_types=1);

// custom-operator: register a PHP `double` operator and call it from a rule.
// Custom operators receive their pre-evaluated arguments as a JSON-array
// string and return a JSON-value string. Built-in names always win.
// Throwing from the callback surfaces as an EvaluateException with the
// engine's structured error detail (tag / operator).
//
// Run from bindings/php/ (needs `composer install` once, plus the C ABI cdylib:
// `cargo build --release --manifest-path ../c/Cargo.toml`):
//   php examples/custom-operator.php

use Goplasmatic\Datalogic\Engine;
use Goplasmatic\Datalogic\Exception\EvaluateException;

require __DIR__ . '/../vendor/autoload.php';

$engine = Engine::builder()
    ->addOperator('double', function (string $argsJson): string {
        $args = json_decode($argsJson, true);
        if (!is_numeric($args[0] ?? null)) {
            throw new InvalidArgumentException('double expects a number');
        }
        return json_encode($args[0] * 2);
    })
    ->build();

echo $engine->apply('{"double": [21]}', '{}'), PHP_EOL; // 42

// A throwing callback maps onto the binding's structured errors.
try {
    $engine->apply('{"double": ["not-a-number"]}', '{}');
} catch (EvaluateException $e) {
    echo "error: {$e->getMessage()} (tag: {$e->errorType})", PHP_EOL;
}
