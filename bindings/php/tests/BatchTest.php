<?php

declare(strict_types=1);

/* SPDX-License-Identifier: Apache-2.0 */

namespace Goplasmatic\Datalogic\Tests;

use Goplasmatic\Datalogic\BatchItemError;
use Goplasmatic\Datalogic\DataHandle;
use Goplasmatic\Datalogic\Engine;
use Goplasmatic\Datalogic\Internal\Native;
use PHPUnit\Framework\TestCase;

final class BatchTest extends TestCase
{
    public function test_evaluate_batch_mixed_success_and_failure_preserves_order(): void
    {
        $engine = new Engine();
        $rule = $engine->compile(
            '{"if":[{"==":[{"var":"kind"},"bad"]},{"throw":"item-failed"},{"var":"x"}]}',
        );
        $session = $engine->openSession();

        $results = $session->evaluateBatch($rule, [
            new DataHandle('{"kind":"ok","x":1}'),
            new DataHandle('{"kind":"bad"}'),
            new DataHandle('{"kind":"ok","x":3}'),
        ]);

        self::assertCount(3, $results);
        self::assertSame('1', $results[0]);
        self::assertSame('3', $results[2]);

        $err = $results[1];
        self::assertInstanceOf(BatchItemError::class, $err);
        self::assertSame(Native::STATUS_EVAL, $err->status);
        self::assertSame('Thrown', $err->tag);
        self::assertStringContainsString('item-failed', $err->message);
        self::assertNotNull($err->operator);
        // The error JSON shape round-trips.
        $decoded = json_decode($err->toJson(), associative: true);
        self::assertSame($err->tag, $decoded['tag']);
        self::assertSame($err->message, $decoded['message']);
    }

    public function test_evaluate_batch_item_failure_does_not_abort_the_rest(): void
    {
        $engine = new Engine();
        $rule = $engine->compile('{"*":[{"var":"x"},10]}');
        $session = $engine->openSession();

        $datas = [];
        for ($i = 0; $i < 100; $i++) {
            $datas[] = new DataHandle(json_encode(['x' => $i]));
        }
        $results = $session->evaluateBatch($rule, $datas);
        self::assertCount(100, $results);
        foreach ($results as $i => $r) {
            self::assertSame((string) ($i * 10), $r);
        }
    }

    public function test_evaluate_batch_empty_input_returns_empty_array(): void
    {
        $engine = new Engine();
        $rule = $engine->compile('{"var":"x"}');
        self::assertSame([], $engine->openSession()->evaluateBatch($rule, []));
    }

    public function test_evaluate_batch_rejects_non_data_handle_items(): void
    {
        $engine = new Engine();
        $rule = $engine->compile('{"var":"x"}');
        $session = $engine->openSession();
        $this->expectException(\InvalidArgumentException::class);
        /** @phpstan-ignore-next-line deliberately wrong element type */
        $session->evaluateBatch($rule, [new DataHandle('{}'), '{"x":1}']);
    }

    public function test_evaluate_many_rule_set_against_one_payload(): void
    {
        $engine = new Engine();
        $session = $engine->openSession();
        $data = new DataHandle('{"x":5}');

        $results = $session->evaluateMany([
            $engine->compile('{">":[{"var":"x"},2]}'),
            $engine->compile('{"throw":"rule-two-failed"}'),
            $engine->compile('{"+":[{"var":"x"},10]}'),
        ], $data);

        self::assertCount(3, $results);
        self::assertSame('true', $results[0]);
        self::assertSame('15', $results[2]);

        $err = $results[1];
        self::assertInstanceOf(BatchItemError::class, $err);
        self::assertSame(Native::STATUS_EVAL, $err->status);
        self::assertSame('Thrown', $err->tag);
        self::assertStringContainsString('rule-two-failed', $err->message);
    }

    public function test_evaluate_many_empty_input_returns_empty_array(): void
    {
        $engine = new Engine();
        self::assertSame([], $engine->openSession()->evaluateMany([], new DataHandle('{}')));
    }

    public function test_evaluate_many_rejects_non_rule_items(): void
    {
        $engine = new Engine();
        $session = $engine->openSession();
        $this->expectException(\InvalidArgumentException::class);
        /** @phpstan-ignore-next-line deliberately wrong element type */
        $session->evaluateMany(['{"var":"x"}'], new DataHandle('{}'));
    }

    public function test_session_stays_usable_after_batch(): void
    {
        $engine = new Engine();
        $rule = $engine->compile('{"+":[{"var":"x"},1]}');
        $session = $engine->openSession();
        $session->evaluateBatch($rule, [new DataHandle('{"x":1}'), new DataHandle('{"x":2}')]);
        self::assertSame('9', $session->evaluate($rule, '{"x":8}'));
    }
}
