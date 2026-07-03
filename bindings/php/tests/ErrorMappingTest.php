<?php

declare(strict_types=1);

/* SPDX-License-Identifier: Apache-2.0 */

namespace Goplasmatic\Datalogic\Tests;

use Goplasmatic\Datalogic\Engine;
use Goplasmatic\Datalogic\Exception\EvaluateException;
use Goplasmatic\Datalogic\Exception\ParseException;
use PHPUnit\Framework\TestCase;

/**
 * The v2 error-handle fields (`datalogic_error_message` / `_tag` /
 * `_operator` / `_path_json`) must land on the exception's public
 * readonly properties exactly like the v1 thread-local block did.
 */
final class ErrorMappingTest extends TestCase
{
    public function test_parse_error_maps_tag_onto_ParseException(): void
    {
        $engine = new Engine();
        try {
            $engine->compile('not-json{{');
            self::fail('expected ParseException');
        } catch (ParseException $ex) {
            self::assertSame('ParseError', $ex->errorType);
            self::assertStringContainsString('Parse error', $ex->getMessage());
            self::assertNull($ex->operatorName);
        }
    }

    public function test_eval_error_maps_tag_operator_and_path(): void
    {
        $engine = new Engine();
        $rule = $engine->compile('{"+":["x",1]}');
        try {
            $rule->evaluate('{}');
            self::fail('expected EvaluateException');
        } catch (EvaluateException $ex) {
            self::assertSame('Thrown', $ex->errorType);
            self::assertSame('+', $ex->operatorName);
            self::assertNotNull($ex->pathJson);
            $path = json_decode($ex->pathJson, associative: true);
            self::assertIsArray($path);
            self::assertNotEmpty($path);
            self::assertArrayHasKey('json_pointer', $path[0]);
        }
    }

    public function test_session_error_carries_same_structured_fields(): void
    {
        $engine = new Engine();
        $rule = $engine->compile('{"throw":"boom"}');
        $session = $engine->openSession();
        try {
            $session->evaluate($rule, '{}');
            self::fail('expected EvaluateException');
        } catch (EvaluateException $ex) {
            self::assertSame('Thrown', $ex->errorType);
            self::assertSame('throw', $ex->operatorName);
            self::assertStringContainsString('boom', $ex->getMessage());
        }
    }

    public function test_cross_engine_rule_maps_to_InvalidArgument(): void
    {
        $a = new Engine();
        $b = new Engine();
        $rule = $b->compile('{"+":[1,1]}');
        $session = $a->openSession();
        try {
            $session->evaluate($rule, '{}');
            self::fail('expected EvaluateException');
        } catch (EvaluateException $ex) {
            self::assertSame('InvalidArgument', $ex->errorType);
            self::assertStringContainsString('different engine', $ex->getMessage());
        }
    }

    public function test_configuration_error_maps_tag(): void
    {
        try {
            Engine::builder()->setConfigJson('{"preset":"bogus"}');
            self::fail('expected EvaluateException');
        } catch (EvaluateException $ex) {
            self::assertSame('ConfigurationError', $ex->errorType);
            self::assertStringContainsString('bogus', $ex->getMessage());
        }
    }
}
