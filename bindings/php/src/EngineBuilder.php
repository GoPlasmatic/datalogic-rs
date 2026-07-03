<?php

declare(strict_types=1);

/* SPDX-License-Identifier: Apache-2.0 */

namespace Goplasmatic\Datalogic;

use FFI;
use FFI\CData;
use Goplasmatic\Datalogic\Exception\DatalogicException;
use Goplasmatic\Datalogic\Internal\Native;

/**
 * Builder for engines with custom JSONLogic operators implemented in
 * PHP. Mirrors the cross-binding contract: registering a name that
 * collides with a built-in (`+`, `if`, `var`, …) silently dispatches
 * to the built-in at evaluation time — built-ins always win.
 */
final class EngineBuilder
{
    private ?CData $handle;
    private bool $consumed = false;
    /**
     * Pin every trampoline Closure so PHP doesn't GC it while the
     * resulting engine still holds the C function pointer.
     *
     * @var list<callable>
     */
    private array $pinned = [];

    public function __construct()
    {
        $ffi = Native::ffi();
        $h = $ffi->datalogic_engine_builder_new();
        if ($h === null) {
            throw new \RuntimeException('datalogic_engine_builder_new returned NULL');
        }
        $this->handle = $h;
    }

    /** Toggle templating mode on the resulting engine. */
    public function withTemplating(bool $enabled): self
    {
        $this->ensureFresh();
        Native::ffi()->datalogic_engine_builder_set_templating($this->handle, $enabled ? 1 : 0);
        return $this;
    }

    /**
     * Set the engine's evaluation configuration from a JSON object
     * string, parsed by the core crate's shared config parser (the same
     * wire format every binding uses). All keys are optional; an
     * optional `"preset"` (`"default"` | `"safe_arithmetic"` |
     * `"strict"`) selects the starting point and the remaining keys
     * (`arithmetic_nan_handling`, `division_by_zero`,
     * `loose_equality_errors`, `truthy_evaluator`, `numeric_coercion`
     * as an object of bools, `max_recursion_depth`) override individual
     * fields on top of it. Unknown keys and values are rejected (error
     * type `"ConfigurationError"`) so typos fail loudly instead of
     * being silently ignored. Each call replaces the builder's entire
     * evaluation config; templating and registered operators are
     * unaffected.
     *
     * @throws DatalogicException if the config JSON is malformed or
     *         contains unknown keys or values
     */
    public function setConfigJson(string $json): self
    {
        $this->ensureFresh();
        $err = Native::newErrorOut();
        $rc = Native::ffi()->datalogic_engine_builder_set_config_json(
            $this->handle,
            $json,
            strlen($json),
            FFI::addr($err),
        );
        if ($rc !== Native::STATUS_OK) {
            throw DatalogicException::fromNative($rc, $err, 'set_config_json failed');
        }
        return $this;
    }

    /**
     * Register a custom JSONLogic operator.
     *
     * The callback receives the operator's pre-evaluated arguments as a
     * JSON-array string and must return a JSON-value string. Throw to
     * signal an evaluation error — the exception's message bubbles back
     * to the caller.
     *
     * @param callable(string): string $op
     */
    public function addOperator(string $name, callable $op): self
    {
        if ($name === '') {
            throw new \InvalidArgumentException('operator name is empty');
        }
        $this->ensureFresh();
        $ffi = Native::ffi();

        // PHP FFI auto-coerces a PHP callable to a C function pointer
        // when the parameter type is a function-pointer typedef
        // (`datalogic_op_fn`). Parameters are typed `mixed` because a
        // TypeError thrown from inside an FFI callback is forbidden
        // ("throwing from FFI callbacks is not allowed"):
        //   - $argsPtr  arrives as CData<const uint8_t*> — the args are
        //     NOT NUL-terminated, so the cdef deliberately avoids
        //     `const char*` (which PHP would auto-convert by scanning
        //     for a NUL) and we copy with the explicit length instead;
        //   - $argsLen  arrives as int;
        //   - $userData arrives as ?CData<void*> (unused, always NULL);
        //   - $out      arrives as CData<datalogic_op_result*>, only
        //     valid during this invocation.
        // The outcome crosses the boundary through the v2 setters —
        // `datalogic_op_result_set_json` / `_set_error` copy the bytes
        // immediately, so no allocator crosses the FFI boundary.
        $trampoline = function (mixed $argsPtr, mixed $argsLen, mixed $userData, mixed $out) use ($op, $ffi): int {
            try {
                $argsJson = ($argsPtr !== null && $argsLen > 0)
                    ? FFI::string($argsPtr, $argsLen)
                    : '[]';
                $result = $op($argsJson);
                if (!is_string($result)) {
                    $msg = 'custom operator returned non-string';
                    $ffi->datalogic_op_result_set_error($out, $msg, strlen($msg));
                    return 1;
                }
                $ffi->datalogic_op_result_set_json($out, $result, strlen($result));
                return 0;
            } catch (\Throwable $t) {
                $msg = $t->getMessage() !== '' ? $t->getMessage() : $t::class;
                $ffi->datalogic_op_result_set_error($out, $msg, strlen($msg));
                return 1;
            }
        };

        $this->pinned[] = $trampoline;

        $err = Native::newErrorOut();
        $rc = $ffi->datalogic_engine_builder_add_operator(
            $this->handle,
            $name,
            strlen($name),
            $trampoline,
            null,
            FFI::addr($err),
        );
        if ($rc !== Native::STATUS_OK) {
            throw DatalogicException::fromNative($rc, $err, 'add_operator failed');
        }
        return $this;
    }

    /**
     * Consume the builder and produce an {@see Engine}. Subsequent calls
     * throw.
     */
    public function build(): Engine
    {
        $this->ensureFresh();
        $ffi = Native::ffi();
        $enginePtr = $ffi->datalogic_engine_builder_build($this->handle);
        $ffi->datalogic_engine_builder_free($this->handle);
        $this->handle = null;
        $this->consumed = true;
        if ($enginePtr === null) {
            throw new \RuntimeException('engine builder build failed');
        }
        return Engine::fromHandle($enginePtr, $this->pinned);
    }

    private function ensureFresh(): void
    {
        if ($this->consumed) {
            throw new \RuntimeException('EngineBuilder has already been built');
        }
        if ($this->handle === null) {
            throw new \RuntimeException('EngineBuilder is invalid');
        }
    }
}
