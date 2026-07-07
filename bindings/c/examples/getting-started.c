/* getting-started: one-shot JSONLogic evaluation over the datalogic C ABI
 * (v2 contract), plus the typed-result tier for boolean predicates.
 *
 * Build the cdylib + header once from the repo root:
 *   cargo build --release --manifest-path bindings/c/Cargo.toml
 * then, from bindings/c/examples/:
 *   make run           # builds + runs all three examples
 * or standalone:
 *   cc -O2 -I../include getting-started.c \
 *      -L../target/release -ldatalogic_c \
 *      -Wl,-rpath,../target/release -o getting-started && ./getting-started
 */

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "datalogic.h"

/* Print the failing call's tag + message and exit. The v2 error is an
 * out-param handle; a NULL handle means the call failed without one. */
static void die(const char *what, datalogic_error *err) {
    fprintf(stderr, "getting-started: %s failed", what);
    if (err != NULL) {
        size_t mlen = 0, tlen = 0;
        const uint8_t *msg = datalogic_error_message(err, &mlen);
        const uint8_t *tag = datalogic_error_tag(err, &tlen);
        fprintf(stderr, ": [%.*s] %.*s", (int)tlen, (const char *)tag,
                (int)mlen, (const char *)msg);
        datalogic_error_free(err);
    }
    fprintf(stderr, "\n");
    exit(1);
}

int main(void) {
    /* Refuse to run against a library built for a different ABI. */
    if (datalogic_abi_version() != DATALOGIC_ABI_VERSION) {
        fprintf(stderr, "ABI version mismatch: lib=%u header=%u\n",
                datalogic_abi_version(), (unsigned)DATALOGIC_ABI_VERSION);
        return 1;
    }

    const char *rule =
        "{\"and\": [{\">=\": [{\"var\": \"age\"}, 18]}, "
        "{\"==\": [{\"var\": \"status\"}, \"active\"]}]}";
    const char *data = "{\"age\": 25, \"status\": \"active\"}";

    datalogic_engine *engine = datalogic_engine_new(0);
    if (engine == NULL) die("engine_new", NULL);

    datalogic_error *err = NULL;

    /* One-shot: compile + evaluate in a single call. The result is an owned
     * buffer released with datalogic_buf_free. */
    datalogic_buf out;
    if (datalogic_engine_apply(engine, (const uint8_t *)rule, strlen(rule),
                               (const uint8_t *)data, strlen(data), &out,
                               &err) != DATALOGIC_STATUS_OK)
        die("engine_apply", err);
    printf("one-shot:   %.*s\n", (int)out.len, (const char *)out.ptr); /* true */
    datalogic_buf_free(out);

    /* Typed result: for predicates, skip the JSON-string round trip. Compile
     * the rule, parse the data once into a handle, and read the result
     * straight into a C boolean (0/1) via the session's typed tier. */
    datalogic_rule *compiled = NULL;
    if (datalogic_engine_compile(engine, (const uint8_t *)rule, strlen(rule),
                                 &compiled, &err) != DATALOGIC_STATUS_OK)
        die("engine_compile", err);

    datalogic_data *parsed = NULL;
    if (datalogic_data_parse((const uint8_t *)data, strlen(data), &parsed,
                             &err) != DATALOGIC_STATUS_OK)
        die("data_parse", err);

    datalogic_session *session = datalogic_engine_session(engine);
    if (session == NULL) die("engine_session", NULL);

    int32_t eligible = 0;
    if (datalogic_session_evaluate_bool(session, compiled, parsed, &eligible,
                                        &err) != DATALOGIC_STATUS_OK)
        die("session_evaluate_bool", err);
    printf("typed bool: %s\n", eligible ? "true" : "false");

    datalogic_session_free(session);
    datalogic_data_free(parsed);
    datalogic_rule_free(compiled);
    datalogic_engine_free(engine);
    return 0;
}
