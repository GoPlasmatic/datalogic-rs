/* SPDX-License-Identifier: Apache-2.0 */
package com.goplasmatic.datalogic;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.node.MissingNode;

/**
 * Result of a traced evaluation. Mirrors the cross-binding wire JSON
 * shape: {@code {result, expression_tree, steps, error?, structured_error?}}.
 */
public final class TracedRun {
    private final JsonNode result;
    private final JsonNode expressionTree;
    private final JsonNode steps;
    private final String error;
    private final JsonNode structuredError;

    TracedRun(JsonNode result, JsonNode expressionTree, JsonNode steps,
              String error, JsonNode structuredError) {
        this.result = result == null ? MissingNode.getInstance() : result;
        this.expressionTree = expressionTree == null ? MissingNode.getInstance() : expressionTree;
        this.steps = steps == null ? MissingNode.getInstance() : steps;
        this.error = error;
        this.structuredError = structuredError == null ? MissingNode.getInstance() : structuredError;
    }

    /** Evaluation result, or a Null node if the run errored. */
    public JsonNode result() { return result; }

    /** Compile-time expression tree for flow-diagram rendering. */
    public JsonNode expressionTree() { return expressionTree; }

    /** Per-node execution log captured during the run (always a JSON array). */
    public JsonNode steps() { return steps; }

    /** Engine error message, or {@code null} on success. */
    public String error() { return error; }

    /** Structured error object, or a Missing node on success. */
    public JsonNode structuredError() { return structuredError; }

    /** Whether the run succeeded. */
    public boolean isSuccess() { return error == null; }
}
