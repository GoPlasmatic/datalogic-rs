<?php
// Boundary-benchmark runner for the PHP binding (`runtime: "php"`).
//
// Exercises the binding's public API on the ABI-v2 tiers. The
// handle-path modes are runtime-guarded with class/method_exists so the
// runner degrades to the string modes on an older binding. Surface used:
//
//   - new Goplasmatic\Datalogic\DataHandle(string $json), close()
//   - Session::evaluate(Rule $rule, string|DataHandle $data): string —
//     one union-typed method covers both the string and handle paths
//   - Session::evaluateMany(array $rules, DataHandle $data): array —
//     returns list<string|BatchItemError> (string on item success)
//
// Modes: session-evaluate, session-evaluate-data (guarded),
// session-evaluate-many-100 (guarded; ns_op per evaluation: call/100),
// rule-evaluate, encode-eval-decode-roundtrip, engine-apply-oneshot.
//
// Timing discipline (BINDINGS-OVERHEAD.md appendix): warmup 5,000
// iterations (run with JIT: php -d opcache.enable_cli=1 -d opcache.jit=tracing
// -d opcache.jit_buffer_size=64M), pilot to ~250 ms per sample, median
// of 5, results consumed into a sink.
//
// Usage:
//   php -d opcache.enable_cli=1 -d opcache.jit=tracing \
//       -d opcache.jit_buffer_size=64M \
//       runner-php.php <workloads-dir> [--modes=a,b] [--workloads=x,y]
// Native lib resolution: DATALOGIC_NATIVE_LIB env var, or the binding's
// built-in fallback to ../c/target/release.

declare(strict_types=1);

// Composer autoload when present; otherwise a minimal PSR-4 fallback to
// the in-tree binding source (the binding has no required deps).
$composer = __DIR__ . '/../../../bindings/php/vendor/autoload.php';
if (is_file($composer)) {
    require $composer;
} else {
    spl_autoload_register(function (string $class): void {
        $prefix = 'Goplasmatic\\Datalogic\\';
        if (str_starts_with($class, $prefix)) {
            $rel = str_replace('\\', '/', substr($class, strlen($prefix)));
            $path = __DIR__ . '/../../../bindings/php/src/' . $rel . '.php';
            if (is_file($path)) {
                require $path;
            }
        }
    });
}

use Goplasmatic\Datalogic\Engine;

const RUNTIME = 'php';
const WARMUP = 5000; // JIT runtime tier
const TARGET_SAMPLE_NS = 250e6;
const PILOT_MIN_NS = 10e6;
const SAMPLES = 5;
const MANY_N = 100;

$globalSink = 0;

/** @param callable(int):int $batch Runs n iterations, returns a sink. */
function measure(callable $batch): float
{
    global $globalSink;
    $globalSink += $batch(WARMUP);

    $n = 32;
    while (true) {
        $t0 = hrtime(true);
        $globalSink += $batch($n);
        $elapsed = hrtime(true) - $t0;
        if ($elapsed >= PILOT_MIN_NS) {
            $perOp = $elapsed / $n;
            break;
        }
        $n *= 2;
    }

    $iters = max(1, (int) round(TARGET_SAMPLE_NS / $perOp));
    $samples = [];
    for ($s = 0; $s < SAMPLES; $s++) {
        $t0 = hrtime(true);
        $globalSink += $batch($iters);
        $samples[] = (hrtime(true) - $t0) / $iters;
    }
    sort($samples);
    return $samples[intdiv(SAMPLES, 2)];
}

function emit(string $mode, string $workload, float $nsOp): void
{
    printf(
        "{\"runtime\": \"%s\", \"mode\": \"%s\", \"workload\": \"%s\", \"ns_op\": %.3f}\n",
        RUNTIME,
        $mode,
        $workload,
        $nsOp
    );
}

function verifyStr(string $mode, string $workload, string $got, string $expected): void
{
    if ($got !== $expected) {
        fwrite(STDERR, "runner-php: verification failed for mode=$mode workload=$workload\n"
            . "  expected: $expected\n  got:      $got\n");
        exit(1);
    }
}

// ---- CLI ----
$dir = null;
$modeFilter = null;
$workloadFilter = null;
foreach (array_slice($argv, 1) as $arg) {
    if (str_starts_with($arg, '--modes=')) {
        $modeFilter = explode(',', substr($arg, strlen('--modes=')));
    } elseif (str_starts_with($arg, '--workloads=')) {
        $workloadFilter = explode(',', substr($arg, strlen('--workloads=')));
    } else {
        $dir = $arg;
    }
}
if ($dir === null) {
    fwrite(STDERR, "usage: php runner-php.php <workloads-dir> [--modes=a,b] [--workloads=x,y]\n");
    exit(1);
}

$hasDataHandle = class_exists('Goplasmatic\\Datalogic\\DataHandle');

$engine = new Engine();

foreach (['simple', 'eligibility', 'array100'] as $name) {
    if ($workloadFilter !== null && !in_array($name, $workloadFilter, true)) {
        continue;
    }
    $ruleJson = file_get_contents("$dir/$name.rule.json");
    $dataJson = file_get_contents("$dir/$name.data.json");
    $expected = file_get_contents("$dir/$name.expected.json");
    if ($ruleJson === false || $dataJson === false || $expected === false) {
        fwrite(STDERR, "runner-php: cannot read workload $name from $dir\n");
        exit(1);
    }

    $rule = $engine->compile($ruleJson);
    $session = $engine->openSession();
    // Single hot array identity for the encode/decode round trip.
    $dataArr = json_decode($dataJson, true);

    $modes = [];

    $modes['session-evaluate'] = [
        'verify' => fn () => verifyStr('session-evaluate', $name,
            $session->evaluate($rule, $dataJson), $expected),
        'batch' => function (int $n) use ($session, $rule, $dataJson): int {
            $sink = 0;
            for ($i = 0; $i < $n; $i++) {
                $sink += strlen($session->evaluate($rule, $dataJson));
            }
            return $sink;
        },
        'perCallEvals' => 1.0,
    ];

    if ($hasDataHandle) {
        // v2: parse-once data handle (constructor-style API).
        $dataHandle = new \Goplasmatic\Datalogic\DataHandle($dataJson);

        $modes['session-evaluate-data'] = [
            // Same union-typed Session::evaluate, handle-shaped input.
            'verify' => fn () => verifyStr('session-evaluate-data', $name,
                $session->evaluate($rule, $dataHandle), $expected),
            'batch' => function (int $n) use ($session, $rule, $dataHandle): int {
                $sink = 0;
                for ($i = 0; $i < $n; $i++) {
                    $sink += strlen($session->evaluate($rule, $dataHandle));
                }
                return $sink;
            },
            'perCallEvals' => 1.0,
        ];

        if (method_exists($session, 'evaluateMany')) {
            // 100 identical rules, compiled separately (a rule-set of
            // identical rules — separate compiles so the batch doesn't
            // flatter one hot compiled tree).
            $manyRules = [];
            for ($i = 0; $i < MANY_N; $i++) {
                $manyRules[] = $engine->compile($ruleJson);
            }
            $modes['session-evaluate-many-100'] = [
                'verify' => function () use ($session, $manyRules, $dataHandle, $name, $expected): void {
                    foreach ($session->evaluateMany($manyRules, $dataHandle) as $r) {
                        if (!is_string($r)) {
                            fwrite(STDERR, "runner-php: many item failed: {$r->tag}: {$r->message}\n");
                            exit(1);
                        }
                        verifyStr('session-evaluate-many-100', $name, $r, $expected);
                    }
                },
                'batch' => function (int $n) use ($session, $manyRules, $dataHandle): int {
                    $sink = 0;
                    for ($i = 0; $i < $n; $i++) {
                        $results = $session->evaluateMany($manyRules, $dataHandle);
                        $sink += strlen($results[0]) + strlen($results[MANY_N - 1]);
                    }
                    return $sink;
                },
                'perCallEvals' => (float) MANY_N,
            ];
        }
    } else {
        fwrite(STDERR, "runner-php: DataHandle not present in this binding build; "
            . "skipping session-evaluate-data / session-evaluate-many-100\n");
    }

    $modes['rule-evaluate'] = [
        'verify' => fn () => verifyStr('rule-evaluate', $name,
            $rule->evaluate($dataJson), $expected),
        'batch' => function (int $n) use ($rule, $dataJson): int {
            $sink = 0;
            for ($i = 0; $i < $n; $i++) {
                $sink += strlen($rule->evaluate($dataJson));
            }
            return $sink;
        },
        'perCallEvals' => 1.0,
    ];

    // From-array caller shape: json_encode in, evaluate, json_decode out
    // (the documented "encode-eval-decode-roundtrip" tier).
    $modes['encode-eval-decode-roundtrip'] = [
        'verify' => function () use ($rule, $dataArr, $name, $expected): void {
            $res = json_decode($rule->evaluate(json_encode($dataArr)));
            verifyStr('encode-eval-decode-roundtrip', $name, json_encode($res), $expected);
        },
        'batch' => function (int $n) use ($rule, $dataArr): int {
            $sink = 0;
            for ($i = 0; $i < $n; $i++) {
                $res = json_decode($rule->evaluate(json_encode($dataArr)));
                $sink += $res === null ? 0 : 1;
            }
            return $sink;
        },
        'perCallEvals' => 1.0,
    ];

    $modes['engine-apply-oneshot'] = [
        'verify' => fn () => verifyStr('engine-apply-oneshot', $name,
            $engine->apply($ruleJson, $dataJson), $expected),
        'batch' => function (int $n) use ($engine, $ruleJson, $dataJson): int {
            $sink = 0;
            for ($i = 0; $i < $n; $i++) {
                $sink += strlen($engine->apply($ruleJson, $dataJson));
            }
            return $sink;
        },
        'perCallEvals' => 1.0,
    ];

    foreach ($modes as $mode => $spec) {
        if ($modeFilter !== null && !in_array($mode, $modeFilter, true)) {
            continue;
        }
        $spec['verify']();
        emit($mode, $name, measure($spec['batch']) / $spec['perCallEvals']);
    }
}

fwrite(STDERR, 'runner-php: sink=' . $globalSink . "\n");
