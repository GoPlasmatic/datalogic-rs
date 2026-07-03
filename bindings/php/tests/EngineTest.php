<?php

declare(strict_types=1);

/* SPDX-License-Identifier: Apache-2.0 */

namespace Goplasmatic\Datalogic\Tests;

use Goplasmatic\Datalogic\Engine;
use Goplasmatic\Datalogic\Exception\DatalogicException;
use Goplasmatic\Datalogic\Exception\EvaluateException;
use Goplasmatic\Datalogic\Exception\ParseException;
use PHPUnit\Framework\TestCase;

final class EngineTest extends TestCase
{
    public function test_version_is_non_empty(): void
    {
        self::assertNotSame('', Engine::version());
    }

    public function test_apply_one_shot_returns_json_result(): void
    {
        $engine = new Engine();
        self::assertSame('3', $engine->apply('{"+":[1,2]}', '{}'));
    }

    public function test_compile_then_evaluate_reuses_rule(): void
    {
        $engine = new Engine();
        $rule = $engine->compile('{"var":"x"}');
        foreach ([1, 7, 42] as $x) {
            self::assertSame((string) $x, $rule->evaluate('{"x":' . $x . '}'));
        }
    }

    public function test_session_reuses_arena_across_calls(): void
    {
        $engine = new Engine();
        $rule = $engine->compile('{"*":[{"var":"x"},2]}');
        $session = $engine->openSession();
        foreach ([3, 5, 8] as $x) {
            self::assertSame((string) ($x * 2), $session->evaluate($rule, '{"x":' . $x . '}'));
        }
        self::assertGreaterThan(0, $session->allocatedBytes());
    }

    public function test_session_reset_is_optional_and_keeps_session_usable(): void
    {
        $engine = new Engine();
        $rule = $engine->compile('{"+":[{"var":"x"},1]}');
        $session = $engine->openSession();
        self::assertSame('2', $session->evaluate($rule, '{"x":1}'));
        $session->reset();
        self::assertSame('3', $session->evaluate($rule, '{"x":2}'));
    }

    public function test_parse_error_throws_ParseException(): void
    {
        $engine = new Engine();
        $this->expectException(ParseException::class);
        $engine->compile('not-json{{');
    }

    public function test_evaluate_error_throws_with_operator_and_path(): void
    {
        $engine = new Engine();
        $rule = $engine->compile('{"throw":"boom"}');
        try {
            $rule->evaluate('{}');
            self::fail('expected EvaluateException');
        } catch (EvaluateException $ex) {
            self::assertSame('Thrown', $ex->errorType);
            self::assertNotNull($ex->pathJson);
            self::assertStringStartsWith('[', $ex->pathJson);
        }
    }

    public function test_templating_engine_constructs(): void
    {
        $engine = new Engine(templating: true);
        self::assertNotSame('', $engine->apply('{"+":[1,1]}', '{}'));
    }

    public function test_flagd_sem_ver_operator_is_available(): void
    {
        $engine = new Engine();
        self::assertSame('true', $engine->apply('{"sem_ver":["1.2.3","<","2.0.0"]}', '{}'));
    }

    public function test_traced_session_returns_result_and_steps(): void
    {
        $engine = new Engine();
        $session = $engine->openTracedSession();
        $run = $session->evaluate('{"+":[{"var":"x"},1]}', '{"x":41}');

        self::assertTrue($run->isSuccess());
        self::assertSame(42, $run->result);
        self::assertNotEmpty($run->steps);
        self::assertIsArray($run->expressionTree);
        self::assertNull($run->error);
    }

    public function test_traced_session_surfaces_runtime_error_in_payload(): void
    {
        $engine = new Engine();
        $session = $engine->openTracedSession();
        $run = $session->evaluate('{"throw":"boom"}', '{}');

        self::assertFalse($run->isSuccess());
        self::assertNotNull($run->error);
        self::assertIsArray($run->structuredError);
    }

    public function test_builder_registers_custom_operator(): void
    {
        $engine = Engine::builder()
            ->addOperator('double', function (string $argsJson): string {
                $arr = json_decode($argsJson, associative: true);
                return (string) ((int) $arr[0] * 2);
            })
            ->build();

        self::assertSame('42', $engine->apply('{"double":[21]}', '{}'));
    }

    public function test_builder_custom_operator_error_propagates(): void
    {
        $engine = Engine::builder()
            ->addOperator('boom', function (): string {
                throw new \RuntimeException('custom-failure');
            })
            ->build();

        try {
            $engine->apply('{"boom":[]}', '{}');
            self::fail('expected EvaluateException');
        } catch (EvaluateException $ex) {
            self::assertStringContainsString('custom-failure', $ex->getMessage());
        }
    }

    public function test_builder_set_config_json_strict_preset_takes_effect(): void
    {
        // Default config: null coerces to 0 and the sum evaluates.
        $lenient = new Engine();
        self::assertSame('1', $lenient->apply('{"+":[null,1]}', '{}'));

        // Strict preset: the same rule rejects the non-numeric null.
        $strict = Engine::builder()
            ->setConfigJson('{"preset":"strict"}')
            ->build();
        $this->expectException(EvaluateException::class);
        $strict->apply('{"+":[null,1]}', '{}');
    }

    public function test_builder_set_config_json_rejects_bad_input(): void
    {
        // Malformed JSON surfaces the parser's message.
        try {
            Engine::builder()->setConfigJson('not-json{{');
            self::fail('expected DatalogicException');
        } catch (DatalogicException $ex) {
            self::assertSame('ConfigurationError', $ex->errorType);
            self::assertNotSame('', $ex->getMessage());
        }

        // Unknown enum values fail loudly instead of being ignored.
        try {
            Engine::builder()->setConfigJson('{"preset":"bogus"}');
            self::fail('expected DatalogicException');
        } catch (DatalogicException $ex) {
            self::assertStringContainsString('bogus', $ex->getMessage());
        }
    }

    public function test_builder_set_config_json_chains_with_templating(): void
    {
        $engine = Engine::builder()
            ->withTemplating(true)
            ->setConfigJson('{"preset":"strict"}')
            ->build();
        self::assertSame('3', $engine->apply('{"+":[1,2]}', '{}'));

        $this->expectException(EvaluateException::class);
        $engine->apply('{"+":[null,1]}', '{}');
    }
}
