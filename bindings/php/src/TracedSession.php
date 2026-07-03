<?php

declare(strict_types=1);

/* SPDX-License-Identifier: Apache-2.0 */

namespace Goplasmatic\Datalogic;

use FFI;
use FFI\CData;
use Goplasmatic\Datalogic\Exception\DatalogicException;
use Goplasmatic\Datalogic\Exception\EvaluateException;
use Goplasmatic\Datalogic\Internal\Native;

/**
 * Trace-enabled handle over an {@see Engine}. Every
 * {@see TracedSession::evaluate()} call returns a {@see TracedRun}
 * carrying the result alongside execution-step and expression-tree
 * metadata. Thread-safe — share freely.
 */
final class TracedSession
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
            throw new \RuntimeException('TracedSession has been closed');
        }
        return $this->handle;
    }

    /**
     * One-shot traced evaluation. Engine errors surface inside the
     * returned {@see TracedRun} ({@see TracedRun::$error}) rather than
     * as a thrown exception — the trace data is always returned
     * alongside, even on failure.
     */
    public function evaluate(string $ruleJson, string $dataJson): TracedRun
    {
        $ffi = Native::ffi();
        $buf = $ffi->new('datalogic_buf');
        $err = Native::newErrorOut();
        $rc = $ffi->datalogic_traced_session_evaluate(
            $this->handle(),
            $ruleJson,
            strlen($ruleJson),
            $dataJson,
            strlen($dataJson),
            FFI::addr($buf),
            FFI::addr($err),
        );
        if ($rc !== Native::STATUS_OK) {
            throw DatalogicException::fromNative($rc, $err, 'traced session evaluate failed');
        }
        $payload = Native::takeBuf($buf);
        $decoded = json_decode($payload, associative: true);
        if (!is_array($decoded)) {
            throw new EvaluateException('traced session returned malformed payload');
        }
        return new TracedRun(
            result:          $decoded['result'] ?? null,
            expressionTree:  $decoded['expression_tree'] ?? null,
            steps:           is_array($decoded['steps'] ?? null) ? $decoded['steps'] : [],
            error:           is_string($decoded['error'] ?? null) ? $decoded['error'] : null,
            structuredError: $decoded['structured_error'] ?? null,
        );
    }

    public function close(): void
    {
        if ($this->handle !== null) {
            Native::ffi()->datalogic_traced_session_free($this->handle);
            $this->handle = null;
        }
    }

    public function __destruct()
    {
        $this->close();
    }
}
