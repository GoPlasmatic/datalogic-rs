<?php

declare(strict_types=1);

/* SPDX-License-Identifier: Apache-2.0 */

namespace Goplasmatic\Datalogic\Tests;

use Goplasmatic\Datalogic\DataHandle;
use Goplasmatic\Datalogic\Engine;
use Goplasmatic\Datalogic\Exception\EvaluateException;
use Goplasmatic\Datalogic\Session;
use PHPUnit\Framework\TestCase;

final class TypedEvaluationTest extends TestCase
{
    private Engine $engine;
    private Session $session;
    private DataHandle $data;

    protected function setUp(): void
    {
        $this->engine = new Engine();
        $this->session = $this->engine->openSession();
        $this->data = new DataHandle('{"x":5,"pi":3.14,"name":"neo","empty":"","flag":true}');
    }

    public function test_evaluate_bool_both_polarities(): void
    {
        self::assertTrue($this->session->evaluateBool($this->engine->compile('{">":[{"var":"x"},2]}'), $this->data));
        self::assertFalse($this->session->evaluateBool($this->engine->compile('{"<":[{"var":"x"},2]}'), $this->data));
    }

    public function test_evaluate_bool_type_mismatch_on_number_result(): void
    {
        $rule = $this->engine->compile('{"var":"x"}');
        try {
            $this->session->evaluateBool($rule, $this->data);
            self::fail('expected EvaluateException');
        } catch (EvaluateException $ex) {
            self::assertSame('TypeMismatch', $ex->errorType);
            self::assertStringContainsString('not a boolean', $ex->getMessage());
        }
    }

    public function test_evaluate_int_exact_integer(): void
    {
        self::assertSame(15, $this->session->evaluateInt($this->engine->compile('{"*":[{"var":"x"},3]}'), $this->data));
    }

    public function test_evaluate_int_type_mismatch_on_float_result(): void
    {
        $rule = $this->engine->compile('{"var":"pi"}');
        try {
            $this->session->evaluateInt($rule, $this->data);
            self::fail('expected EvaluateException');
        } catch (EvaluateException $ex) {
            self::assertSame('TypeMismatch', $ex->errorType);
        }
    }

    public function test_evaluate_float_accepts_any_number(): void
    {
        self::assertSame(3.14, $this->session->evaluateFloat($this->engine->compile('{"var":"pi"}'), $this->data));
        // Integers are numbers too.
        self::assertSame(5.0, $this->session->evaluateFloat($this->engine->compile('{"var":"x"}'), $this->data));
    }

    public function test_evaluate_float_type_mismatch_on_string_result(): void
    {
        $rule = $this->engine->compile('{"var":"name"}');
        try {
            $this->session->evaluateFloat($rule, $this->data);
            self::fail('expected EvaluateException');
        } catch (EvaluateException $ex) {
            self::assertSame('TypeMismatch', $ex->errorType);
        }
    }

    public function test_evaluate_truthy_never_type_mismatches(): void
    {
        // Truthiness collapses ANY result — strings, numbers, bools.
        self::assertTrue($this->session->evaluateTruthy($this->engine->compile('{"var":"name"}'), $this->data));
        self::assertFalse($this->session->evaluateTruthy($this->engine->compile('{"var":"empty"}'), $this->data));
        self::assertTrue($this->session->evaluateTruthy($this->engine->compile('{"var":"x"}'), $this->data));
        self::assertTrue($this->session->evaluateTruthy($this->engine->compile('{"var":"flag"}'), $this->data));
    }
}
