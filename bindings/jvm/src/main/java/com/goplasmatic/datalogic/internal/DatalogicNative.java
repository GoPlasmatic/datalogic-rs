/*
 * SPDX-License-Identifier: Apache-2.0
 *
 * JNA library interface for libdatalogic_c. Mirrors `bindings/c/include/datalogic.h`
 * 1:1 — keep entry points and signatures in sync when the C ABI changes.
 *
 * Native lookup:
 *   1. `jna.library.path` system property (set by Maven surefire for in-tree
 *      tests; users can set it themselves to override).
 *   2. JNA's JAR-resource auto-extract from `<jna-platform>/` at the
 *      classpath root (populated by the release workflow before
 *      `mvn package`). `Native.load` searches
 *      `<Platform.RESOURCE_PREFIX>/<libname>`, e.g. `darwin-aarch64/`.
 *   3. `java.library.path` and the OS's default loader paths.
 */

package com.goplasmatic.datalogic.internal;

import com.sun.jna.Callback;
import com.sun.jna.Library;
import com.sun.jna.Native;
import com.sun.jna.Pointer;
import com.sun.jna.PointerType;
import com.sun.jna.ptr.PointerByReference;

/** JNA library proxy for libdatalogic_c. Loaded lazily on first use. */
public interface DatalogicNative extends Library {

    /** Eagerly-loaded singleton; getting at it triggers Native.load(...). */
    DatalogicNative INSTANCE = Native.load("datalogic_c", DatalogicNative.class);

    // =============== Meta ===============

    /** Returns a borrowed UTF-8 pointer; do NOT free. */
    Pointer datalogic_version();

    /** Releases an owned UTF-8 buffer returned by an evaluate/apply entry point. */
    void datalogic_string_free(Pointer ptr);

    // =============== Engine ===============

    /** templating: 0 = off, !=0 = on. */
    Pointer datalogic_engine_new(int templating);

    void datalogic_engine_free(Pointer engine);

    /** Returns NULL on parse failure; query last-error state. */
    Pointer datalogic_engine_compile(Pointer engine, String rule_json);

    /** Returns owned UTF-8 (or NULL on failure); release via datalogic_string_free. */
    Pointer datalogic_engine_apply(Pointer engine, String rule_json, String data_json);

    Pointer datalogic_engine_session(Pointer engine);

    Pointer datalogic_engine_traced_session(Pointer engine);

    // =============== Engine builder (custom operators) ===============

    Pointer datalogic_engine_builder_new();

    void datalogic_engine_builder_free(Pointer builder);

    void datalogic_engine_builder_set_templating(Pointer builder, int enabled);

    /** Returns 0 on success, -1 on error (last-error state populated). */
    int datalogic_engine_builder_set_config_json(Pointer builder, String config_json);

    /**
     * Callback type for custom operators. JNA invokes this on whatever thread
     * the engine calls our trampoline from.
     */
    interface OperatorCallback extends Callback {
        /**
         * @param args_json borrowed UTF-8 JSON-array string of pre-evaluated args
         * @param user_data opaque pointer registered alongside the callback
         * @param error_out on error, set *error_out to a freshly-allocated UTF-8 message
         * @return freshly-allocated UTF-8 JSON-value string on success, or NULL on error
         */
        Pointer invoke(Pointer args_json, Pointer user_data, PointerByReference error_out);
    }

    /** Returns 0 on success, -1 on error. */
    int datalogic_engine_builder_add_operator(
            Pointer builder,
            String name,
            OperatorCallback callback,
            Pointer user_data);

    /** Returns NULL if the builder has already been built / is invalid. */
    Pointer datalogic_engine_builder_build(Pointer builder);

    // =============== Rule ===============

    void datalogic_rule_free(Pointer rule);

    Pointer datalogic_rule_evaluate(Pointer rule, String data_json);

    // =============== Session ===============

    void datalogic_session_free(Pointer session);

    Pointer datalogic_session_evaluate(Pointer session, Pointer rule, String data_json);

    void datalogic_session_reset(Pointer session);

    long datalogic_session_allocated_bytes(Pointer session);

    // =============== Traced session ===============

    void datalogic_traced_session_free(Pointer session);

    /** Always returns a JSON-object string (or NULL on invalid input pointers). */
    Pointer datalogic_traced_session_evaluate(Pointer session, String rule_json, String data_json);

    // =============== Last error ===============

    void datalogic_last_error_clear();

    /** Borrowed pointer — do NOT free. Valid until the next call on this thread. */
    Pointer datalogic_last_error_message();

    Pointer datalogic_last_error_type();

    Pointer datalogic_last_error_operator();

    Pointer datalogic_last_error_path_json();

    /**
     * Common owned-pointer wrapper used as a JNA struct field type when we
     * want strongly-typed handle classes. Currently unused in the API
     * (every handle is just `Pointer`), but kept here as the future
     * extension point if/when handle types diverge.
     */
    final class OwnedPointer extends PointerType {
        public OwnedPointer() { super(); }
        public OwnedPointer(Pointer p) { super(p); }
    }
}
