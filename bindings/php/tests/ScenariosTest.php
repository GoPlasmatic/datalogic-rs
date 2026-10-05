<?php

declare(strict_types=1);

/* SPDX-License-Identifier: Apache-2.0 */

namespace Goplasmatic\Datalogic\Tests;

use Goplasmatic\Datalogic\Engine;
use Goplasmatic\Datalogic\Exception\DatalogicException;
use Goplasmatic\Datalogic\Internal\Native;
use PHPUnit\Framework\Attributes\DataProvider;
use PHPUnit\Framework\TestCase;

/**
 * The cross-binding scenarios in bindings/scenarios/api.json, through the
 * PHP API. Every binding runs the same file (see bindings/BINDINGS.md).
 */
final class ScenariosTest extends TestCase
{
    /**
     * Each case twice: decoded to arrays for comparing results, and to
     * objects for re-encoding inputs (an empty `{}` stays an object).
     *
     * @return iterable<string, array{array<string, mixed>, object}>
     */
    public static function scenarios(): iterable
    {
        $text = (string) file_get_contents(__DIR__ . '/../../scenarios/api.json');
        $arrays = json_decode($text, true);
        $objects = json_decode($text, false);
        foreach ($arrays as $i => $case) {
            if (is_array($case)) {
                yield $case['call'] . ': ' . $case['description'] => [$case, $objects[$i]];
            }
        }
    }

    /** @param array<string, mixed> $case */
    private static function engineFor(array $case): Engine
    {
        $opts = $case['engine'] ?? [];
        $b = Engine::builder();
        if (($opts['templating'] ?? false) === true) {
            $b->withTemplating(true);
        }
        if (isset($opts['template_key_escape'])) {
            $b->withTemplateKeyEscape($opts['template_key_escape']);
        }
        if (isset($opts['config'])) {
            $b->setConfigJson(json_encode($opts['config']));
        }
        return $b->build();
    }

    private static function enc(mixed $v): string
    {
        return json_encode($v, JSON_PRESERVE_ZERO_FRACTION);
    }

    /**
     * @param array<string, mixed> $case
     * @return array{0: mixed, 1: ?string}
     */
    private static function runCase(array $case, object $raw): array
    {
        $e = self::engineFor($case);
        $rule = self::enc($raw->rule ?? null);
        $data = self::enc($raw->data ?? null);
        $mode = match ($case['mode'] ?? 'engine') {
            'strict' => Native::MODE_STRICT,
            'template' => Native::MODE_TEMPLATE,
            default => Native::MODE_ENGINE,
        };
        try {
            switch ($case['call']) {
                case 'check':
                    return [array_map(
                        static fn (array $d): array => [$d['code'], $d['pointer']],
                        json_decode($e->check($rule, $mode), true),
                    ), null];
                case 'truthy':
                    return [$e->truthy(self::enc($raw->value)), null];
                case 'facts':
                    return [json_decode($e->compile($rule)->facts(), true), null];
                case 'metered':
                    $m = $e->openSession()->evaluateMetered($e->compile($rule), $data, $case['budget']);
                    return [json_decode($m['value'], true), null];
                default:
                    $r = match ($case['call']) {
                        'compile_template' => $e->compileTemplate($rule),
                        'compile_strict' => $e->compileStrict($rule),
                        'compile_checked' => $e->compileChecked($rule),
                        default => $e->compile($rule),
                    };
                    return [json_decode($r->evaluate($data), true), null];
            }
        } catch (DatalogicException $ex) {
            return [null, $ex->errorType];
        }
    }

    /** @param array<string, mixed> $case */
    #[DataProvider('scenarios')]
    public function testScenario(array $case, object $raw): void
    {
        [$got, $error] = self::runCase($case, $raw);
        if (array_key_exists('error', $case)) {
            self::assertSame($case['error'], $error);
            return;
        }
        self::assertNull($error, 'unexpected error');
        if (array_key_exists('diagnostics', $case)) {
            self::assertSame($case['diagnostics'], $got);
        } elseif (array_key_exists('facts', $case)) {
            foreach ($case['facts'] as $k => $v) {
                self::assertSame($v, $got[$k], $k);
            }
        } else {
            self::assertEquals($case['result'], $got);
        }
    }
}
