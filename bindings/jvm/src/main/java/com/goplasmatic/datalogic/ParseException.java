/* SPDX-License-Identifier: Apache-2.0 */
package com.goplasmatic.datalogic;

/** Thrown when a JSONLogic rule fails to parse. */
public final class ParseException extends DatalogicException {
    ParseException(String message, String errorType, String operatorName, String pathJson) {
        super(message, errorType, operatorName, pathJson);
    }
}
