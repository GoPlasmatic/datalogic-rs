<?php

declare(strict_types=1);

/* SPDX-License-Identifier: Apache-2.0 */

namespace Goplasmatic\Datalogic\Tests;

use Goplasmatic\Datalogic\Engine;
use Goplasmatic\Datalogic\Exception\DatalogicException;
use Goplasmatic\Datalogic\Exception\EvaluateException;
use Goplasmatic\Datalogic\Exception\ParseException;
use Goplasmatic\Datalogic\Internal\Native;
use PHPUnit\Framework\TestCase;

/** The ABI v2.1 surface: check, modes, operators, facts, truthy, metering, builder options. */
final class IntrospectionTest extends TestCase
{
    public function testCheckReportsProblemsWithPointers(): void
    {
        $e = new Engine();
        $diags = json_decode($e->check('{"if": [true, {"vr": "x"}, {"map": [1]}]}'), true);
        self::assertSame('UnknownOperator', $diags[0]['code']);
        self::assertSame('/if/1', $diags[0]['pointer']);
        self::assertSame('ArgumentCount', $diags[1]['code']);
        self::assertSame('[]', $e->check('{"a": {"var": "x"}, "b": 1}', Native::MODE_TEMPLATE));
    }

    public function testCompileCheckedCarriesDiagnostics(): void
    {
        $e = new Engine();
        try {
            $e->compileChecked('{"if": [{"bogus": 1}, {"map": [1]}]}');
            self::fail('expected a CompileError');
        } catch (ParseException $ex) {
            self::assertSame('CompileError', $ex->errorType);
            self::assertCount(2, json_decode($ex->diagnosticsJson, true));
        }
        self::assertSame('3', $e->compileChecked('{"+": [1, {"var": "x"}]}')->evaluate('{"x": 2}'));
    }

    public function testCompileModesAndFacts(): void
    {
        $e = new Engine();
        $tpl = '{"user": {"var": "name"}, "n": 1}';
        $rule = $e->compileTemplate($tpl);
        self::assertSame('{"user":"ana","n":1}', $rule->evaluate('{"name": "ana"}'));
        self::assertSame([['name']], json_decode($rule->facts(), true)['reads']);
        $this->expectException(DatalogicException::class);
        $e->compileStrict($tpl);
    }

    public function testOperatorsAndTruthy(): void
    {
        $e = new Engine();
        $docs = json_decode((string) file_get_contents(__DIR__ . '/../../../docs/src/operators/operators.json'), true);
        self::assertSame($docs, json_decode($e->operators(), true));
        self::assertFalse($e->truthy('{}'));
        self::assertTrue($e->truthy('{"a": 1}'));
    }

    public function testMeteredSession(): void
    {
        $e = new Engine();
        $rule = $e->compile('{"map": [{"var": "xs"}, {"+": [{"var": ""}, 1]}]}');
        $s = $e->openSession();
        $m = $s->evaluateMetered($rule, '{"xs": [1, 2, 3]}');
        self::assertSame('[2,3,4]', $m['value']);
        self::assertGreaterThan(0, $m['ops']);
        try {
            $s->evaluateMetered($rule, '{"xs": [1, 2, 3]}', 2);
            self::fail('expected BudgetExceeded');
        } catch (EvaluateException $ex) {
            self::assertSame('BudgetExceeded', $ex->errorType);
        }
    }

    public function testBuilderEscapeAndStrictNames(): void
    {
        $one = static fn (string $args): string => '1';
        try {
            Engine::builder()->withStrictOperatorNames(true)->addOperator('length', $one);
            self::fail('expected ConfigurationError');
        } catch (DatalogicException $ex) {
            self::assertSame('ConfigurationError', $ex->errorType);
        }
        $e = Engine::builder()->withTemplating(true)->withTemplateKeyEscape('$')
            ->withStrictOperatorNames(true)->addOperator('uno', $one)->build();
        self::assertSame('{"type":1,"k":2}', $e->apply('{"$type": {"uno": []}, "k": 2}', 'null'));
    }

    public function testNegativeBudgetIsRefused(): void
    {
        $e = new Engine();
        $rule = $e->compile('{"+": [1, 2]}');
        $this->expectException(\InvalidArgumentException::class);
        $e->openSession()->evaluateMetered($rule, 'null', -1);
    }

    public function testEscapeOutsideAscii(): void
    {
        // Two-, three- and four-byte UTF-8 escapes, decoded without mbstring.
        foreach (['§', '€', '😀'] as $escape) {
            $e = Engine::builder()->withTemplating(true)->withTemplateKeyEscape($escape)->build();
            self::assertSame('{"type":1,"k":2}', $e->apply('{"' . $escape . 'type": 1, "k": 2}', 'null'));
        }
        foreach (['', 'ab', "\xC3"] as $bad) {
            try {
                Engine::builder()->withTemplateKeyEscape($bad);
                self::fail('expected InvalidArgumentException for ' . bin2hex($bad));
            } catch (\InvalidArgumentException) {
                self::addToAssertionCount(1);
            }
        }
    }

    public function testErrorsCarryNodeIds(): void
    {
        try {
            (new Engine())->apply('{"+": ["a", 1]}', 'null');
            self::fail('expected an evaluation error');
        } catch (EvaluateException $ex) {
            self::assertNotNull($ex->nodeIdsJson);
        }
    }
}
