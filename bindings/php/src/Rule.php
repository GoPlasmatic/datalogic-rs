<?php

declare(strict_types=1);

/* SPDX-License-Identifier: Apache-2.0 */

namespace Goplasmatic\Datalogic;

use FFI\CData;
use Goplasmatic\Datalogic\Exception\DatalogicException;
use Goplasmatic\Datalogic\Internal\Native;

/**
 * A compiled JSONLogic rule. Safe to share across requests — each
 * {@see Rule::evaluate()} uses its own short-lived arena. For tight
 * loops, open a {@see Session} instead.
 */
final class Rule
{
    private ?CData $handle;

    /** @internal */
    public function __construct(CData $handle)
    {
        $this->handle = $handle;
    }

    /** @internal */
    public function handle(): CData
    {
        if ($this->handle === null) {
            throw new \RuntimeException('Rule has been closed');
        }
        return $this->handle;
    }

    /** Evaluate against `$dataJson`; returns the JSON-string result. */
    public function evaluate(string $dataJson): string
    {
        $ptr = Native::ffi()->datalogic_rule_evaluate($this->handle(), $dataJson);
        if ($ptr === null) {
            throw DatalogicException::fromLastError('rule evaluate failed');
        }
        return Native::takeString($ptr) ?? '';
    }

    /** Release the underlying rule handle. Idempotent. */
    public function close(): void
    {
        if ($this->handle !== null) {
            Native::ffi()->datalogic_rule_free($this->handle);
            $this->handle = null;
        }
    }

    public function __destruct()
    {
        $this->close();
    }
}
