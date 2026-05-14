/* SPDX-License-Identifier: Apache-2.0 */
package com.goplasmatic.datalogic;

/**
 * Functional interface for a user-defined JSONLogic operator. The
 * argument JSON is the pre-evaluated arguments as a JSON-array string
 * (e.g. {@code "[1, 2, \"x\"]"}); return the operator's result as a
 * JSON-value string (e.g. {@code "42"}, {@code "\"a\""},
 * {@code "{\"k\":1}"}). Throw to signal an evaluation error — the
 * exception's message bubbles back to the caller as part of the
 * evaluation error.
 */
@FunctionalInterface
public interface CustomOperator {
    String invoke(String argsJson) throws Exception;
}
