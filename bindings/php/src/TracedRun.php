<?php

declare(strict_types=1);

/* SPDX-License-Identifier: Apache-2.0 */

namespace Goplasmatic\Datalogic;

/**
 * Result of a traced evaluation. Mirrors the cross-binding wire JSON
 * shape: `{result, expression_tree, steps, error?, structured_error?, pointers?}`.
 */
final class TracedRun
{
    /**
     * @param mixed         $result          parsed result (any JSON value), or null on failure
     * @param mixed         $expressionTree  compile-time expression tree
     * @param list<mixed>   $steps           per-node execution log
     * @param string|null   $error           engine error message, or null on success
     * @param mixed         $structuredError structured error object, or null on success
     * @param array<string, string> $pointers for every node id the run can
     *        name (as a string key), the RFC 6901 JSON Pointer into the rule
     *        of the value that node was compiled from; empty when the rule did
     *        not compile
     */
    public function __construct(
        public readonly mixed $result,
        public readonly mixed $expressionTree,
        public readonly array $steps,
        public readonly ?string $error,
        public readonly mixed $structuredError,
        public readonly array $pointers = [],
    ) {}

    public function isSuccess(): bool
    {
        return $this->error === null;
    }
}
