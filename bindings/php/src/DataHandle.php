<?php

declare(strict_types=1);

/* SPDX-License-Identifier: Apache-2.0 */

namespace Goplasmatic\Datalogic;

use FFI;
use FFI\CData;
use Goplasmatic\Datalogic\Exception\DatalogicException;
use Goplasmatic\Datalogic\Internal\Native;

/**
 * A parsed, resident JSON document — parse once, evaluate many.
 *
 * Immutable and engine-independent: one handle can feed rules compiled
 * by different engines, any number of times (evaluation never consumes
 * it). This is the hot path for repeated evaluations against the same
 * payload — the per-call JSON parse disappears entirely.
 *
 * <code>
 * $data = new DataHandle('{"user":{"age":42}}');
 * $rule->evaluate($data);
 * $session->evaluateBool($rule, $data);
 * </code>
 *
 * The native handle is released when this object is GC'd, or explicitly
 * via {@see DataHandle::close()}.
 */
final class DataHandle
{
    private ?CData $handle;

    /**
     * Parse a JSON document into a resident handle.
     *
     * @throws \Goplasmatic\Datalogic\Exception\ParseException on malformed JSON
     */
    public function __construct(string $json)
    {
        $ffi = Native::ffi();
        $out = $ffi->new('datalogic_data*');
        $err = Native::newErrorOut();
        $rc = $ffi->datalogic_data_parse($json, strlen($json), FFI::addr($out), FFI::addr($err));
        if ($rc !== Native::STATUS_OK) {
            throw DatalogicException::fromNative($rc, $err, 'data parse failed');
        }
        $this->handle = $out;
    }

    /** @internal */
    public function handle(): CData
    {
        if ($this->handle === null) {
            throw new \RuntimeException('DataHandle has been closed');
        }
        return $this->handle;
    }

    /** Bytes held by the handle's backing arena (input copy + tree). */
    public function allocatedBytes(): int
    {
        return $this->handle === null
            ? 0
            : Native::ffi()->datalogic_data_allocated_bytes($this->handle);
    }

    /** Release the underlying data handle. Idempotent. */
    public function close(): void
    {
        if ($this->handle !== null) {
            Native::ffi()->datalogic_data_free($this->handle);
            $this->handle = null;
        }
    }

    public function __destruct()
    {
        $this->close();
    }
}
