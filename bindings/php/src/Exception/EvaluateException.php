<?php

declare(strict_types=1);

/* SPDX-License-Identifier: Apache-2.0 */

namespace Goplasmatic\Datalogic\Exception;

/** Thrown when rule evaluation fails (Thrown, NaN, runtime, …). */
final class EvaluateException extends DatalogicException {}
