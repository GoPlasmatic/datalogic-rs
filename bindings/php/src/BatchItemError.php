<?php

declare(strict_types=1);

/* SPDX-License-Identifier: Apache-2.0 */

namespace Goplasmatic\Datalogic;

/**
 * Per-item failure inside {@see Session::evaluateBatch()} /
 * {@see Session::evaluateMany()}. Item failures never throw — the
 * batch result array holds a `BatchItemError` in the failed item's
 * slot (and JSON-string results in every successful slot), so one bad
 * payload can't abort the other N-1.
 *
 * `$status` is the item's raw `datalogic_status`
 * ({@see \Goplasmatic\Datalogic\Internal\Native}::STATUS_*), `$tag`
 * the stable engine error tag (e.g. `"NaN"`, `"Thrown"`), `$operator`
 * the outermost failing operator when known.
 */
final class BatchItemError
{
    public function __construct(
        public readonly int $status,
        public readonly string $tag,
        public readonly string $message,
        public readonly ?string $operator = null,
    ) {}

    /** Rebuild the item's `{"tag","message","operator"?}` error JSON. */
    public function toJson(): string
    {
        $obj = ['tag' => $this->tag, 'message' => $this->message];
        if ($this->operator !== null) {
            $obj['operator'] = $this->operator;
        }
        return json_encode($obj, JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE) ?: '{}';
    }
}
