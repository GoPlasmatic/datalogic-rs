<?php

declare(strict_types=1);

// getting-started: one-shot JSONLogic evaluation with the datalogic PHP binding,
// plus the parse-once DataHandle and a typed session evaluation.
//
// Run from bindings/php/ (needs `composer install` once, plus the C ABI cdylib:
// `cargo build --release --manifest-path ../c/Cargo.toml`):
//   php examples/getting-started.php

use Goplasmatic\Datalogic\DataHandle;
use Goplasmatic\Datalogic\Engine;

require __DIR__ . '/../vendor/autoload.php';

$rule = '{"and": [{">=": [{"var": "age"}, 18]}, {"==": [{"var": "status"}, "active"]}]}';
$data = '{"age": 25, "status": "active"}';

// One-shot: compile + evaluate in a single call.
$engine = new Engine();
echo $engine->apply($rule, $data), PHP_EOL; // true

// Parse the payload once, evaluate as often as you like — and ask for
// a typed PHP bool instead of a JSON string.
$compiled = $engine->compile($rule);
$handle = new DataHandle($data);
$session = $engine->openSession();
var_dump($session->evaluateBool($compiled, $handle)); // bool(true)
