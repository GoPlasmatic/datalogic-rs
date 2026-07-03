<?php

declare(strict_types=1);

// compile-once-evaluate-many: compile a rule once, evaluate it against
// ~100k payloads, and print the last result plus a rough per-evaluation cost —
// then do it again over pre-parsed DataHandles (session + batch), the v2
// hot path that skips the per-call JSON parse entirely.
//
// Run from bindings/php/ (needs `composer install` once, plus the C ABI cdylib:
// `cargo build --release --manifest-path ../c/Cargo.toml`):
//   php examples/compile-once-evaluate-many.php

use Goplasmatic\Datalogic\BatchItemError;
use Goplasmatic\Datalogic\DataHandle;
use Goplasmatic\Datalogic\Engine;

require __DIR__ . '/../vendor/autoload.php';

const ITERATIONS = 100_000;

$engine = new Engine();
$rule = $engine->compile('{"*": [{"var": "price"}, {"-": [1, {"var": "discount"}]}]}');

// Tier 1: compiled rule, JSON string per call.
$last = null;
$start = hrtime(true);
for ($i = 0; $i < ITERATIONS; $i++) {
    $last = $rule->evaluate(json_encode(['price' => 100 + $i % 100, 'discount' => 0.2]));
}
$elapsedNs = hrtime(true) - $start;

echo "last result: {$last}", PHP_EOL;
echo sprintf('%d evaluations (JSON string), ~%d ns/op', ITERATIONS, intdiv($elapsedNs, ITERATIONS)), PHP_EOL;

// Tier 2: parse each distinct payload ONCE into a DataHandle, then run a
// session over the handles — zero parse work per evaluation.
$handles = [];
for ($p = 0; $p < 100; $p++) {
    $handles[] = new DataHandle(json_encode(['price' => 100 + $p, 'discount' => 0.2]));
}
$session = $engine->openSession();

$start = hrtime(true);
for ($i = 0; $i < ITERATIONS; $i++) {
    $last = $session->evaluate($rule, $handles[$i % 100]);
}
$elapsedNs = hrtime(true) - $start;
echo sprintf('%d evaluations (DataHandle), ~%d ns/op', ITERATIONS, intdiv($elapsedNs, ITERATIONS)), PHP_EOL;

// Tier 3: one rule x N handles in a single FFI crossing. Per-item failures
// come back as BatchItemError values instead of aborting the batch.
$results = $session->evaluateBatch($rule, $handles);
$failed = count(array_filter($results, fn ($r) => $r instanceof BatchItemError));
echo sprintf('batch of %d: first=%s last=%s failures=%d', count($results), $results[0], $results[99], $failed), PHP_EOL;
