<?php

declare(strict_types=1);

/* SPDX-License-Identifier: Apache-2.0 */

namespace Goplasmatic\Datalogic\Tests;

use Goplasmatic\Datalogic\Engine;
use Goplasmatic\Datalogic\Exception\DatalogicException;
use Goplasmatic\Datalogic\Exception\EvaluateException;
use PHPUnit\Framework\TestCase;

/**
 * The operation budget reaches this binding through the config wire
 * format alone — there is no per-call budget entry point across the C
 * ABI — so these pin that the key is accepted, that it actually bounds
 * evaluation, and that the failure carries its own error type.
 */
final class BudgetTest extends TestCase
{
    private const RULE = '{"map":[{"var":"xs"},{"*":[{"var":""},2]}]}';

    /** `{"xs":[0,1,...,n-1]}` */
    private static function items(int $n): string
    {
        return '{"xs":[' . implode(',', range(0, $n - 1)) . ']}';
    }

    private static function engineWithBudget(string $config): Engine
    {
        return Engine::builder()->setConfigJson($config)->build();
    }

    public function test_ops_budget_bounds_evaluation(): void
    {
        // 100 is comfortably above the three-item run and comfortably
        // below the 200-item one.
        $engine = self::engineWithBudget('{"ops_budget":100}');
        self::assertSame('[0,2,4]', $engine->apply(self::RULE, self::items(3)));

        try {
            $engine->apply(self::RULE, self::items(200));
            self::fail('expected the 200-item payload to exceed a budget of 100');
        } catch (EvaluateException $ex) {
            self::assertSame('BudgetExceeded', $ex->errorType);
            self::assertStringContainsString('budget', $ex->getMessage());
        }
    }

    public function test_try_cannot_recover_from_an_exhausted_budget(): void
    {
        $engine = self::engineWithBudget('{"ops_budget":10}');
        try {
            $engine->apply('{"try":[' . self::RULE . ',"fallback"]}', self::items(200));
            self::fail('try must not recover from an exhausted budget');
        } catch (EvaluateException $ex) {
            self::assertSame('BudgetExceeded', $ex->errorType);
        }
    }

    public function test_a_null_budget_is_unbounded(): void
    {
        $engine = self::engineWithBudget('{"ops_budget":null}');
        self::assertStringStartsWith('[0,2,4,', $engine->apply(self::RULE, self::items(200)));
    }

    public function test_an_invalid_budget_is_a_configuration_error(): void
    {
        foreach (['0', '-1', '"many"'] as $bad) {
            try {
                Engine::builder()->setConfigJson('{"ops_budget":' . $bad . '}');
                self::fail("budget {$bad} should be rejected");
            } catch (DatalogicException $ex) {
                self::assertSame('ConfigurationError', $ex->errorType);
            }
        }
    }

    public function test_tensor_operators_are_priced_by_the_elements_they_move(): void
    {
        // `zeros` allocates 256 elements from a three-node rule: the node
        // count alone would price this at nothing.
        $engine = self::engineWithBudget('{"ops_budget":100}');
        try {
            $engine->apply('{"zeros":[[16,16],"f32"]}', '{}');
            self::fail('a 256-element tensor should not fit a budget of 100');
        } catch (EvaluateException $ex) {
            self::assertSame('BudgetExceeded', $ex->errorType);
        }
    }

    /**
     * The tensor family crosses this binding as JSON like any other
     * value — the tagged form — so no FFI change was needed for it.
     */
    public function test_tensor_round_trips_as_tagged_json(): void
    {
        $engine = new Engine();
        $emitted = $engine->apply('{"tensor":[[1,2,3],"u8"]}', '{}');
        self::assertSame('{"tensor":{"dtype":"u8","shape":[3],"data":"AQID"}}', $emitted);
        // And the emitted form is accepted back as a rule.
        self::assertSame('[1,2,3]', $engine->apply('{"to_list":[' . $emitted . ']}', '{}'));
    }
}
