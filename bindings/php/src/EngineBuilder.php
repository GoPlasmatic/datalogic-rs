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
     * Pin every PHP callable AND its FFI closure CData so PHP doesn't
     * GC them while the resulting engine still holds the function
     * pointer.
     *
     * @var list<mixed>
     */
    private array $pinned = [];

    public function __construct()
    {
        $ffi = Native::ffi();
        $h = $ffi->datalogic_engine_builder_new();
        if ($h === null) {
            throw DatalogicException::fromLastError('builder_new failed');
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
        $rc = Native::ffi()->datalogic_engine_builder_set_config_json($this->handle, $json);
        if ($rc !== 0) {
            throw DatalogicException::fromLastError('set_config_json failed');
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
        // when the parameter type is a function-pointer typedef. Use
        // `mixed` parameter types because:
        //   - `const char*` auto-coerces to PHP string (PHP 8.1+)
        //   - `void*` passes through as CData<void*> (may be null-pointer)
        //   - `char**` passes through as CData<char**>
        // Typing parameters strictly would throw TypeError from inside
        // the callback, which PHP forbids ("throwing from FFI callbacks
        // is not allowed").
        $trampoline = function (mixed $args, mixed $userData, mixed $errorOutPtr) use ($op, $ffi): ?CData {
            $argsString = is_string($args) ? $args : FFI::string($args);
            try {
                $result = $op($argsString);
                if (!is_string($result)) {
                    return self::writeError($ffi, $errorOutPtr, 'custom operator returned non-string');
                }
                return self::allocCString($ffi, $result);
            } catch (\Throwable $t) {
                return self::writeError($ffi, $errorOutPtr, $t->getMessage() ?: $t::class);
            }
        };

        $this->pinned[] = $trampoline;

        $rc = $ffi->datalogic_engine_builder_add_operator(
            $this->handle,
            $name,
            $trampoline,
            null,
        );
        if ($rc !== 0) {
            throw DatalogicException::fromLastError('add_operator failed');
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
            throw DatalogicException::fromLastError('builder build failed');
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

    /**
     * Allocate a libc-managed UTF-8 NUL-terminated buffer and return a
     * `char*` pointer to it. The Rust side calls libc `free()` on this
     * pointer, so we MUST allocate with the matching allocator. PHP
     * FFI's `new($type, owned: false, persistent: true)` uses libc
     * `malloc` rather than Zend's emalloc — that's the critical bit;
     * with `persistent: false` the allocation goes through Zend and a
     * Rust `free()` segfaults on heap-allocator mismatch.
     */
    private static function allocCString(FFI $ffi, string $s): CData
    {
        $len = strlen($s);
        $buf = $ffi->new("char[" . ($len + 1) . "]", owned: false, persistent: true);
        FFI::memcpy($buf, $s, $len);
        $buf[$len] = "\0";
        return $ffi->cast('char*', FFI::addr($buf[0]));
    }

    private static function writeError(FFI $ffi, CData $errorOutPtr, string $msg): ?CData
    {
        $alloc = self::allocCString($ffi, $msg);
        $errorOutPtr[0] = $alloc;
        return null;
    }
}
