<?php

declare(strict_types=1);

/* SPDX-License-Identifier: Apache-2.0 */

namespace Goplasmatic\Datalogic\Tests;

use Goplasmatic\Datalogic\DataHandle;
use Goplasmatic\Datalogic\Engine;
use Goplasmatic\Datalogic\Exception\ParseException;
use PHPUnit\Framework\TestCase;

final class DataHandleTest extends TestCase
{
    public function test_rule_evaluate_accepts_data_handle(): void
    {
        $engine = new Engine();
        $rule = $engine->compile('{"*":[{"var":"x"},2]}');
        $data = new DataHandle('{"x":21}');
        self::assertSame('42', $rule->evaluate($data));
        // Not consumed by evaluation — reusable.
        self::assertSame('42', $rule->evaluate($data));
        // The string overload keeps working side by side.
        self::assertSame('42', $rule->evaluate('{"x":21}'));
    }

    public function test_session_evaluate_accepts_data_handle(): void
    {
        $engine = new Engine();
        $rule = $engine->compile('{"+":[{"var":"a"},{"var":"b"}]}');
        $session = $engine->openSession();
        $data = new DataHandle('{"a":40,"b":2}');
        self::assertSame('42', $session->evaluate($rule, $data));
        self::assertSame('42', $session->evaluate($rule, '{"a":40,"b":2}'));
    }

    public function test_data_handle_is_engine_independent(): void
    {
        $data = new DataHandle('{"x":7}');
        $a = new Engine();
        $b = new Engine(templating: true);
        self::assertSame('8', $a->compile('{"+":[{"var":"x"},1]}')->evaluate($data));
        self::assertSame('6', $b->compile('{"-":[{"var":"x"},1]}')->evaluate($data));
    }

    public function test_allocated_bytes_is_positive_and_zero_after_close(): void
    {
        $data = new DataHandle('{"x":[1,2,3,4,5]}');
        self::assertGreaterThan(0, $data->allocatedBytes());
        $data->close();
        self::assertSame(0, $data->allocatedBytes());
        $data->close(); // idempotent
    }

    public function test_using_closed_handle_throws(): void
    {
        $engine = new Engine();
        $rule = $engine->compile('{"var":"x"}');
        $data = new DataHandle('{"x":1}');
        $data->close();
        $this->expectException(\RuntimeException::class);
        $this->expectExceptionMessage('DataHandle has been closed');
        $rule->evaluate($data);
    }

    public function test_malformed_json_throws_ParseException(): void
    {
        try {
            new DataHandle('{"x":');
            self::fail('expected ParseException');
        } catch (ParseException $ex) {
            self::assertSame('ParseError', $ex->errorType);
            self::assertNotSame('', $ex->getMessage());
        }
    }
}
