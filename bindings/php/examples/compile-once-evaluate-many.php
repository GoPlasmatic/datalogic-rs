<?php

declare(strict_types=1);

// compile-once-evaluate-many: compile a rule once, evaluate it against
// ~100k payloads, and print the last result plus a rough per-evaluation cost.
//
// Run from bindings/php/ (needs `composer install` once, plus the C ABI cdylib:
// `cargo build --release --manifest-path ../c/Cargo.toml`):
//   php examples/compile-once-evaluate-many.php

use Goplasmatic\Datalogic\Engine;

require __DIR__ . '/../vendor/autoload.php';

const ITERATIONS = 100_000;

$engine = new Engine();
$rule = $engine->compile('{"*": [{"var": "price"}, {"-": [1, {"var": "discount"}]}]}');

$last = null;
$start = hrtime(true);
for ($i = 0; $i < ITERATIONS; $i++) {
    $last = $rule->evaluate(json_encode(['price' => 100 + $i % 100, 'discount' => 0.2]));
}
$elapsedNs = hrtime(true) - $start;

echo "last result: {$last}", PHP_EOL;
echo sprintf('%d evaluations, ~%d ns/op', ITERATIONS, intdiv($elapsedNs, ITERATIONS)), PHP_EOL;
