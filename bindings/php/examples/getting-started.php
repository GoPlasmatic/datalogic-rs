<?php

declare(strict_types=1);

// getting-started: one-shot JSONLogic evaluation with the datalogic PHP binding.
//
// Run from bindings/php/ (needs `composer install` once, plus the C ABI cdylib:
// `cargo build --release --manifest-path ../c/Cargo.toml`):
//   php examples/getting-started.php

use Goplasmatic\Datalogic\Engine;

require __DIR__ . '/../vendor/autoload.php';

$rule = '{"and": [{">=": [{"var": "age"}, 18]}, {"==": [{"var": "status"}, "active"]}]}';
$data = '{"age": 25, "status": "active"}';

$engine = new Engine();
echo $engine->apply($rule, $data), PHP_EOL; // true
