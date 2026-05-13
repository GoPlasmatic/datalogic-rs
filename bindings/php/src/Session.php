<?php

declare(strict_types=1);

/* SPDX-License-Identifier: Apache-2.0 */

namespace Goplasmatic\Datalogic;

use FFI\CData;
use Goplasmatic\Datalogic\Exception\DatalogicException;
use Goplasmatic\Datalogic\Internal\Native;

/**
 * Hot-loop session bound to a single {@see Engine}. Reuses one
 * `bumpalo::Bump` across evaluations and resets it at the start of every
 * call so peak memory stays bounded. NOT thread-safe — open one per
 * thread (PHP is typically single-threaded per request, so this is the
 * common case).
 */
final class Session
{
    private ?CData $handle;

    /** @internal */
    public function __construct(CData $handle)
    {
        $this->handle = $handle;
    }

    private function handle(): CData
    {
        if ($this->handle === null) {
            throw new \RuntimeException('Session has been closed');
        }
        return $this->handle;
    }

    /**
     * Evaluate `$rule` against `$dataJson` using this session's reusable
     * arena.
     */
    public function evaluate(Rule $rule, string $dataJson): string
    {
        $ptr = Native::ffi()->datalogic_session_evaluate(
            $this->handle(),
            $rule->handle(),
            $dataJson,
        );
        if ($ptr === null) {
            throw DatalogicException::fromLastError('session evaluate failed');
        }
        return Native::takeString($ptr) ?? '';
    }

    /**
     * Manually reset the session's arena. Optional — every
     * {@see Session::evaluate()} already resets at the start of the call.
     */
    public function reset(): void
    {
        Native::ffi()->datalogic_session_reset($this->handle());
    }

    /**
     * Bytes currently held by the session's arena (sum across all
     * chunks).
     */
    public function allocatedBytes(): int
    {
        return Native::ffi()->datalogic_session_allocated_bytes($this->handle());
    }

    public function close(): void
    {
        if ($this->handle !== null) {
            Native::ffi()->datalogic_session_free($this->handle);
            $this->handle = null;
        }
    }

    public function __destruct()
    {
        $this->close();
    }
}
