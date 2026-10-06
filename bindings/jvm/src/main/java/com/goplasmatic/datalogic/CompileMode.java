/* SPDX-License-Identifier: Apache-2.0 */
package com.goplasmatic.datalogic;

import com.goplasmatic.datalogic.internal.DatalogicNative;

/** How {@link Engine#compileMode}, {@link Engine#check} and {@link TracedSession#evaluate(String, String, CompileMode)} read a rule. */
public enum CompileMode {
    /** The engine's own mode, as {@link Engine#compile(String)} reads it. */
    ENGINE(DatalogicNative.MODE_ENGINE),
    /** Outside templating mode: a multi-key object or an unknown operator is an error. */
    STRICT(DatalogicNative.MODE_STRICT),
    /** In templating mode: a multi-key object is an output template, an unknown key a field. */
    TEMPLATE(DatalogicNative.MODE_TEMPLATE);

    private final int code;

    CompileMode(int code) {
        this.code = code;
    }

    int code() {
        return code;
    }
}
