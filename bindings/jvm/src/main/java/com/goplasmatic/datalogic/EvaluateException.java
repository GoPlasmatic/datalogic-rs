/* SPDX-License-Identifier: Apache-2.0 */
package com.goplasmatic.datalogic;

/** Thrown when rule evaluation fails (Thrown, NaN, runtime, …). */
public final class EvaluateException extends DatalogicException {
    EvaluateException(String message, String errorType, String operatorName, String pathJson) {
        super(message, errorType, operatorName, pathJson);
    }
}
