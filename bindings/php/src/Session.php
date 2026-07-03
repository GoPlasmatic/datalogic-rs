<?php

declare(strict_types=1);

/* SPDX-License-Identifier: Apache-2.0 */

namespace Goplasmatic\Datalogic;

use FFI;
use FFI\CData;
use Goplasmatic\Datalogic\Exception\DatalogicException;
use Goplasmatic\Datalogic\Internal\Native;

/**
 * Hot-loop session bound to a single {@see Engine}. Reuses one
 * `bumpalo::Bump` across evaluations and resets it at the start of every
 * call so peak memory stays bounded. NOT thread-safe — open one per
 * thread (PHP is typically single-threaded per request, so this is the
 * common case).
 *
 * Results returned by the C side are borrowed from a session-owned
 * buffer; every method here copies them into PHP strings before
 * returning, so callers never see the borrow.
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
     * Evaluate `$rule` against a JSON string or a pre-parsed
     * {@see DataHandle} using this session's reusable arena. The
     * `DataHandle` overload is the hot path: zero parse work per call.
     */
    public function evaluate(Rule $rule, string|DataHandle $data): string
    {
        $ffi = Native::ffi();
        $outPtr = $ffi->new('const uint8_t*');
        $outLen = $ffi->new('size_t');
        $err = Native::newErrorOut();
        if ($data instanceof DataHandle) {
            $rc = $ffi->datalogic_session_evaluate_data(
                $this->handle(),
                $rule->handle(),
                $data->handle(),
                FFI::addr($outPtr),
                FFI::addr($outLen),
                FFI::addr($err),
            );
        } else {
            $rc = $ffi->datalogic_session_evaluate(
                $this->handle(),
                $rule->handle(),
                $data,
                strlen($data),
                FFI::addr($outPtr),
                FFI::addr($outLen),
                FFI::addr($err),
            );
        }
        if ($rc !== Native::STATUS_OK) {
            throw DatalogicException::fromNative($rc, $err, 'session evaluate failed');
        }
        // Borrowed until the next call touching this session — copy now.
        return Native::copyBytes($outPtr, $outLen->cdata) ?? '';
    }

    /**
     * Evaluate and read the result as a strict JSON boolean. Any other
     * result type throws (errorType `"TypeMismatch"`); for JSONLogic
     * truthiness coercion use {@see Session::evaluateTruthy()}.
     */
    public function evaluateBool(Rule $rule, DataHandle $data): bool
    {
        $ffi = Native::ffi();
        $out = $ffi->new('int32_t');
        $err = Native::newErrorOut();
        $rc = $ffi->datalogic_session_evaluate_bool(
            $this->handle(),
            $rule->handle(),
            $data->handle(),
            FFI::addr($out),
            FFI::addr($err),
        );
        if ($rc !== Native::STATUS_OK) {
            throw DatalogicException::fromNative($rc, $err, 'evaluateBool failed');
        }
        return $out->cdata !== 0;
    }

    /**
     * Evaluate and read the result as an integer. Throws (errorType
     * `"TypeMismatch"`) when the result is not an exact integer number.
     */
    public function evaluateInt(Rule $rule, DataHandle $data): int
    {
        $ffi = Native::ffi();
        $out = $ffi->new('int64_t');
        $err = Native::newErrorOut();
        $rc = $ffi->datalogic_session_evaluate_i64(
            $this->handle(),
            $rule->handle(),
            $data->handle(),
            FFI::addr($out),
            FFI::addr($err),
        );
        if ($rc !== Native::STATUS_OK) {
            throw DatalogicException::fromNative($rc, $err, 'evaluateInt failed');
        }
        return $out->cdata;
    }

    /**
     * Evaluate and read the result as a float. Accepts any JSON number;
     * throws (errorType `"TypeMismatch"`) otherwise.
     */
    public function evaluateFloat(Rule $rule, DataHandle $data): float
    {
        $ffi = Native::ffi();
        $out = $ffi->new('double');
        $err = Native::newErrorOut();
        $rc = $ffi->datalogic_session_evaluate_f64(
            $this->handle(),
            $rule->handle(),
            $data->handle(),
            FFI::addr($out),
            FFI::addr($err),
        );
        if ($rc !== Native::STATUS_OK) {
            throw DatalogicException::fromNative($rc, $err, 'evaluateFloat failed');
        }
        return $out->cdata;
    }

    /**
     * Evaluate and collapse the result to a bool via the engine's
     * configured truthiness rules (the same coercion `if`/`and`/`or`
     * apply). Never type-mismatches — any result truthy-converts.
     */
    public function evaluateTruthy(Rule $rule, DataHandle $data): bool
    {
        $ffi = Native::ffi();
        $out = $ffi->new('int32_t');
        $err = Native::newErrorOut();
        $rc = $ffi->datalogic_session_evaluate_truthy(
            $this->handle(),
            $rule->handle(),
            $data->handle(),
            FFI::addr($out),
            FFI::addr($err),
        );
        if ($rc !== Native::STATUS_OK) {
            throw DatalogicException::fromNative($rc, $err, 'evaluateTruthy failed');
        }
        return $out->cdata !== 0;
    }

    /**
     * One rule × many payloads. Returns one entry per input, in order:
     * the JSON-string result on success, a {@see BatchItemError} on
     * per-item failure. Item failures never throw and never abort the
     * remaining items; only argument-level problems (closed handles,
     * a rule from a different engine, …) throw.
     *
     * @param list<DataHandle> $datas
     * @return list<string|BatchItemError>
     */
    public function evaluateBatch(Rule $rule, array $datas): array
    {
        $datas = array_values($datas);
        $n = count($datas);
        if ($n === 0) {
            return [];
        }
        $ffi = Native::ffi();
        $handles = $ffi->new("const datalogic_data*[{$n}]");
        foreach ($datas as $i => $d) {
            if (!$d instanceof DataHandle) {
                throw new \InvalidArgumentException(sprintf(
                    'evaluateBatch expects a list of DataHandle, got %s at index %d',
                    get_debug_type($d),
                    $i,
                ));
            }
            $handles[$i] = $d->handle();
        }
        $results = $ffi->new("datalogic_slice[{$n}]");
        $statuses = $ffi->new("datalogic_status[{$n}]");
        $err = Native::newErrorOut();
        $rc = $ffi->datalogic_session_evaluate_batch(
            $this->handle(),
            $rule->handle(),
            $ffi->cast('const datalogic_data* const*', FFI::addr($handles[0])),
            $n,
            FFI::addr($results[0]),
            FFI::addr($statuses[0]),
            FFI::addr($err),
        );
        if ($rc !== Native::STATUS_OK) {
            throw DatalogicException::fromNative($rc, $err, 'evaluateBatch failed');
        }
        return self::collectItems($n, $results, $statuses);
    }

    /**
     * Many rules × one payload — the rule-set / feature-flag shape.
     * Same per-item semantics and result shape as
     * {@see Session::evaluateBatch()}.
     *
     * @param list<Rule> $rules
     * @return list<string|BatchItemError>
     */
    public function evaluateMany(array $rules, DataHandle $data): array
    {
        $rules = array_values($rules);
        $n = count($rules);
        if ($n === 0) {
            return [];
        }
        $ffi = Native::ffi();
        $handles = $ffi->new("const datalogic_rule*[{$n}]");
        foreach ($rules as $i => $r) {
            if (!$r instanceof Rule) {
                throw new \InvalidArgumentException(sprintf(
                    'evaluateMany expects a list of Rule, got %s at index %d',
                    get_debug_type($r),
                    $i,
                ));
            }
            $handles[$i] = $r->handle();
        }
        $results = $ffi->new("datalogic_slice[{$n}]");
        $statuses = $ffi->new("datalogic_status[{$n}]");
        $err = Native::newErrorOut();
        $rc = $ffi->datalogic_session_evaluate_many(
            $this->handle(),
            $ffi->cast('const datalogic_rule* const*', FFI::addr($handles[0])),
            $n,
            $data->handle(),
            FFI::addr($results[0]),
            FFI::addr($statuses[0]),
            FFI::addr($err),
        );
        if ($rc !== Native::STATUS_OK) {
            throw DatalogicException::fromNative($rc, $err, 'evaluateMany failed');
        }
        return self::collectItems($n, $results, $statuses);
    }

    /**
     * Copy the borrowed per-item slices into PHP values immediately
     * (they die on the next session call). Success → JSON string;
     * failure → the item's `{"tag","message","operator"?}` object
     * decoded into a {@see BatchItemError}.
     *
     * @return list<string|BatchItemError>
     */
    private static function collectItems(int $n, CData $results, CData $statuses): array
    {
        $items = [];
        for ($i = 0; $i < $n; $i++) {
            $status = $statuses[$i];
            $json = Native::copyBytes($results[$i]->ptr, $results[$i]->len) ?? '';
            if ($status === Native::STATUS_OK) {
                $items[] = $json;
                continue;
            }
            $decoded = json_decode($json, associative: true);
            $items[] = new BatchItemError(
                status: $status,
                tag: is_string($decoded['tag'] ?? null) ? $decoded['tag'] : 'InternalError',
                message: is_string($decoded['message'] ?? null) ? $decoded['message'] : $json,
                operator: is_string($decoded['operator'] ?? null) ? $decoded['operator'] : null,
            );
        }
        return $items;
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
