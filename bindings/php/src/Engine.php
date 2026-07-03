<?php

declare(strict_types=1);

/* SPDX-License-Identifier: Apache-2.0 */

namespace Goplasmatic\Datalogic;

use FFI;
use FFI\CData;
use Goplasmatic\Datalogic\Exception\DatalogicException;
use Goplasmatic\Datalogic\Internal\Native;

/**
 * A JSONLogic compile/evaluate engine. Wraps a shared
 * `Arc<datalogic_rs::Engine>` on the Rust side — safe to share across
 * threads. The native handle is released when this object is GC'd, or
 * explicitly via {@see Engine::close()}.
 *
 * <code>
 * $engine = new Engine();
 * $result = $engine->apply('{"+":[1,2]}', '{}');  // "3"
 * </code>
 */
class Engine
{
    private ?CData $handle;
    /**
     * Retain Closure references for any custom operators registered on
     * this engine so PHP doesn't GC them while the C side still holds
     * the function pointer.
     *
     * @var list<callable>
     */
    private array $retainedCallbacks = [];

    public function __construct(bool $templating = false)
    {
        $ffi = Native::ffi();
        $handle = $ffi->datalogic_engine_new($templating ? 1 : 0);
        if ($handle === null) {
            throw new \RuntimeException('datalogic_engine_new returned NULL');
        }
        $this->handle = $handle;
    }

    /**
     * @internal Used by {@see EngineBuilder::build()} to wrap a pre-existing
     * native handle and adopt the builder's pinned callbacks.
     *
     * @param list<callable> $adoptedCallbacks
     */
    public static function fromHandle(CData $handle, array $adoptedCallbacks = []): self
    {
        $engine = (new \ReflectionClass(self::class))->newInstanceWithoutConstructor();
        $engine->handle = $handle;
        $engine->retainedCallbacks = $adoptedCallbacks;
        return $engine;
    }

    /** @internal */
    public function handle(): CData
    {
        if ($this->handle === null) {
            throw new \RuntimeException('Engine has been closed');
        }
        return $this->handle;
    }

    /** The binding's version (sourced from the underlying C ABI). */
    public static function version(): string
    {
        // The one deliberately NUL-terminated static string in the v2
        // ABI — PHP FFI auto-converts the `const char*` return.
        return Native::ffi()->datalogic_version() ?? '';
    }

    /** Compile a JSONLogic rule (JSON-string) into a reusable {@see Rule}. */
    public function compile(string $ruleJson): Rule
    {
        $ffi = Native::ffi();
        $out = $ffi->new('datalogic_rule*');
        $err = Native::newErrorOut();
        $rc = $ffi->datalogic_engine_compile(
            $this->handle(),
            $ruleJson,
            strlen($ruleJson),
            FFI::addr($out),
            FFI::addr($err),
        );
        if ($rc !== Native::STATUS_OK) {
            throw DatalogicException::fromNative($rc, $err, 'compile failed');
        }
        return new Rule($out);
    }

    /**
     * One-shot: compile and evaluate in a single call, returning the
     * JSON-string result.
     */
    public function apply(string $ruleJson, string $dataJson): string
    {
        $ffi = Native::ffi();
        $buf = $ffi->new('datalogic_buf');
        $err = Native::newErrorOut();
        $rc = $ffi->datalogic_engine_apply(
            $this->handle(),
            $ruleJson,
            strlen($ruleJson),
            $dataJson,
            strlen($dataJson),
            FFI::addr($buf),
            FFI::addr($err),
        );
        if ($rc !== Native::STATUS_OK) {
            throw DatalogicException::fromNative($rc, $err, 'apply failed');
        }
        return Native::takeBuf($buf);
    }

    /** Open a hot-loop {@see Session}. NOT thread-safe — one per thread. */
    public function openSession(): Session
    {
        $s = Native::ffi()->datalogic_engine_session($this->handle());
        if ($s === null) {
            throw new \RuntimeException('datalogic_engine_session returned NULL');
        }
        return new Session($s);
    }

    /** Open a {@see TracedSession} for traced evaluation. */
    public function openTracedSession(): TracedSession
    {
        $s = Native::ffi()->datalogic_engine_traced_session($this->handle());
        if ($s === null) {
            throw new \RuntimeException('datalogic_engine_traced_session returned NULL');
        }
        return new TracedSession($s);
    }

    /** Construct a builder for engines with custom operators. */
    public static function builder(): EngineBuilder
    {
        return new EngineBuilder();
    }

    /** Release the underlying engine handle. Idempotent. */
    public function close(): void
    {
        if ($this->handle !== null) {
            Native::ffi()->datalogic_engine_free($this->handle);
            $this->handle = null;
        }
        $this->retainedCallbacks = [];
    }

    public function __destruct()
    {
        $this->close();
    }
}
