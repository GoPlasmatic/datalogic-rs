/* SPDX-License-Identifier: Apache-2.0 */
package com.goplasmatic.datalogic;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;

/**
 * Per-item outcome of {@link Session#evaluateBatch(Rule, java.util.List)}
 * and {@link Session#evaluateMany(java.util.List, DataHandle)}. Item
 * failures never throw — each item carries either a result value or its
 * own error info, mirroring the C ABI's per-item status + error-object
 * contract.
 *
 * @param value         result JSON string, or {@code null} if this item
 *                      failed
 * @param errorTag      stable engine tag ({@code "Thrown"}, {@code "NaN"},
 *                      {@code "InvalidArgument"}, …), or {@code null} on
 *                      success
 * @param errorMessage  human-readable error message, or {@code null} on
 *                      success
 * @param errorOperator outermost failing operator (e.g. {@code "+"}), or
 *                      {@code null} when absent or on success
 */
public record EvalResult(String value, String errorTag, String errorMessage, String errorOperator) {

    private static final ObjectMapper MAPPER = new ObjectMapper();

    /** Whether this item evaluated successfully. */
    public boolean isSuccess() {
        return errorTag == null;
    }

    /** A successful item carrying {@code value}. */
    static EvalResult success(String value) {
        return new EvalResult(value, null, null, null);
    }

    /**
     * A failed item, decoded from the C ABI's per-item error object
     * {@code {"tag": ..., "message": ..., "operator"?: ...}}.
     */
    static EvalResult failure(String errorJson) {
        try {
            JsonNode node = MAPPER.readTree(errorJson);
            String tag = node.hasNonNull("tag") ? node.get("tag").asText() : "InternalError";
            String message = node.hasNonNull("message") ? node.get("message").asText() : errorJson;
            String operator = node.hasNonNull("operator") ? node.get("operator").asText() : null;
            return new EvalResult(null, tag, message, operator);
        } catch (Exception e) {
            return new EvalResult(null, "InternalError", errorJson, null);
        }
    }
}
