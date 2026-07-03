<?php

declare(strict_types=1);

/* SPDX-License-Identifier: Apache-2.0 */

namespace Goplasmatic\Datalogic\Exception;

use FFI;
use FFI\CData;
use Goplasmatic\Datalogic\Internal\Native;
use RuntimeException;

/**
 * Base class for every exception thrown by this binding. Carries the
 * structured error detail exposed by the C ABI's error handles
 * (`datalogic_error_message` / `_tag` / `_operator` / `_path_json`).
 */
class DatalogicException extends RuntimeException
{
    public function __construct(
        string $message,
        public readonly ?string $errorType = null,
        public readonly ?string $operatorName = null,
        public readonly ?string $pathJson = null,
    ) {
        parent::__construct($message);
    }

    /**
     * Construct the right subclass from a failed v2 call: `$status` is
     * the `datalogic_status` the call returned, `$errOut` the
     * `datalogic_error *` out-param slot it may have filled. Reads the
     * borrowed accessors and ALWAYS releases the handle.
     *
     * @internal
     */
    public static function fromNative(int $status, CData $errOut, string $fallback): self
    {
        $ffi = Native::ffi();
        if (FFI::isNull($errOut)) {
            // No detail captured (should not happen — every fallible
            // call in this binding passes an out-param).
            return $status === Native::STATUS_PARSE
                ? new ParseException($fallback)
                : new EvaluateException($fallback);
        }

        $len = $ffi->new('size_t');
        $addr = FFI::addr($len);
        $msg  = Native::copyBytes($ffi->datalogic_error_message($errOut, $addr), $len->cdata);
        $tag  = Native::copyBytes($ffi->datalogic_error_tag($errOut, $addr), $len->cdata);
        $op   = Native::copyBytes($ffi->datalogic_error_operator($errOut, $addr), $len->cdata);
        $path = Native::copyBytes($ffi->datalogic_error_path_json($errOut, $addr), $len->cdata);
        $ffi->datalogic_error_free($errOut);

        $message = ($msg === null || $msg === '') ? $fallback : $msg;
        return $status === Native::STATUS_PARSE
            ? new ParseException($message, $tag, $op, $path)
            : new EvaluateException($message, $tag, $op, $path);
    }
}
