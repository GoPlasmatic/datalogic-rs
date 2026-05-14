<?php

declare(strict_types=1);

/* SPDX-License-Identifier: Apache-2.0 */

namespace Goplasmatic\Datalogic\Exception;

use Goplasmatic\Datalogic\Internal\Native;
use RuntimeException;

/**
 * Base class for every exception thrown by this binding. Mirrors the
 * thread-local last-error block exposed by the C ABI
 * (`datalogic_last_error_*`).
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
     * Construct the right subclass by reading the C ABI's thread-local
     * last-error block.
     */
    public static function fromLastError(string $fallback): self
    {
        $ffi = Native::ffi();
        $msgPtr  = $ffi->datalogic_last_error_message();
        $typePtr = $ffi->datalogic_last_error_type();
        $opPtr   = $ffi->datalogic_last_error_operator();
        $pathPtr = $ffi->datalogic_last_error_path_json();

        $msg  = Native::borrowString($msgPtr) ?? $fallback;
        $type = Native::borrowString($typePtr);
        $op   = Native::borrowString($opPtr);
        $path = Native::borrowString($pathPtr);

        return match ($type) {
            'ParseError' => new ParseException($msg, $type, $op, $path),
            default       => new EvaluateException($msg, $type, $op, $path),
        };
    }
}
