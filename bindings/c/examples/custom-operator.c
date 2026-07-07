/* custom-operator: register a `double` operator and call it from a rule.
 * Custom operators receive their pre-evaluated arguments as a UTF-8 JSON
 * array (not NUL-terminated) and write the outcome through the
 * datalogic_op_result_set_* functions. A non-zero return becomes an
 * evaluation error for the caller. Built-in names always win.
 *
 * Build the cdylib + header once from the repo root:
 *   cargo build --release --manifest-path bindings/c/Cargo.toml
 * then, from bindings/c/examples/:
 *   make run
 */

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "datalogic.h"

static void die(const char *what, datalogic_error *err) {
    fprintf(stderr, "custom-operator: %s failed", what);
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

/* `double` operator: multiply the first numeric argument by two.
 *
 * `args_json` is the JSON array of pre-evaluated arguments, e.g. "[21]".
 * The bytes are borrowed and not NUL-terminated, so parse within
 * `args_len`. An empty array is reported as an operator error. */
static int32_t double_op(const uint8_t *args_json, size_t args_len,
                         void *user_data, datalogic_op_result *out) {
    (void)user_data;

    const char *p = (const char *)args_json;
    const char *end = p + args_len;
    /* Step over the opening bracket and any leading whitespace/commas. */
    while (p < end &&
           (*p == '[' || *p == ' ' || *p == '\t' || *p == '\n' || *p == '\r'))
        p++;
    if (p >= end || *p == ']') {
        const char *msg = "double expects one numeric argument";
        datalogic_op_result_set_error(out, (const uint8_t *)msg, strlen(msg));
        return 1;
    }

    char *num_end = NULL;
    double v = strtod(p, &num_end); /* stops at the ']' terminator */
    if (num_end == p) {
        const char *msg = "double expects a numeric argument";
        datalogic_op_result_set_error(out, (const uint8_t *)msg, strlen(msg));
        return 1;
    }

    char result[64];
    int n = snprintf(result, sizeof(result), "%g", v * 2.0);
    datalogic_op_result_set_json(out, (const uint8_t *)result, (size_t)n);
    return 0;
}

/* Evaluate a rule and print the JSON result. */
static void apply_print(const datalogic_engine *engine, const char *label,
                        const char *rule, const char *data) {
    datalogic_error *err = NULL;
    datalogic_buf out;
    if (datalogic_engine_apply(engine, (const uint8_t *)rule, strlen(rule),
                               (const uint8_t *)data, strlen(data), &out,
                               &err) != DATALOGIC_STATUS_OK)
        die("engine_apply", err);
    printf("%-10s %.*s\n", label, (int)out.len, (const char *)out.ptr);
    datalogic_buf_free(out);
}

/* Evaluate a rule that is expected to fail, and print the surfaced error. */
static void apply_expect_error(const datalogic_engine *engine, const char *label,
                               const char *rule, const char *data) {
    datalogic_error *err = NULL;
    datalogic_buf out;
    datalogic_status st =
        datalogic_engine_apply(engine, (const uint8_t *)rule, strlen(rule),
                               (const uint8_t *)data, strlen(data), &out, &err);
    if (st == DATALOGIC_STATUS_OK) {
        fprintf(stderr, "custom-operator: expected an error but got a result\n");
        datalogic_buf_free(out);
        exit(1);
    }
    size_t mlen = 0, tlen = 0;
    const uint8_t *msg = datalogic_error_message(err, &mlen);
    const uint8_t *tag = datalogic_error_tag(err, &tlen);
    printf("%-10s [%.*s] %.*s\n", label, (int)tlen, (const char *)tag,
           (int)mlen, (const char *)msg);
    datalogic_error_free(err);
}

int main(void) {
    if (datalogic_abi_version() != DATALOGIC_ABI_VERSION) {
        fprintf(stderr, "ABI version mismatch: lib=%u header=%u\n",
                datalogic_abi_version(), (unsigned)DATALOGIC_ABI_VERSION);
        return 1;
    }

    datalogic_engine_builder *builder = datalogic_engine_builder_new();
    if (builder == NULL) die("builder_new", NULL);

    datalogic_error *err = NULL;
    const char *name = "double";
    if (datalogic_engine_builder_add_operator(builder, (const uint8_t *)name,
                                              strlen(name), double_op, NULL,
                                              &err) != DATALOGIC_STATUS_OK)
        die("add_operator", err);

    datalogic_engine *engine = datalogic_engine_builder_build(builder);
    if (engine == NULL) die("builder_build", NULL);
    datalogic_engine_builder_free(builder); /* still required after build */

    /* Direct call. */
    apply_print(engine, "double:", "{\"double\": [21]}", "{}"); /* 42 */

    /* Custom operators compose with built-ins (here, map). */
    apply_print(engine, "mapped:",
                "{\"map\": [{\"var\": \"xs\"}, {\"double\": [{\"var\": \"\"}]}]}",
                "{\"xs\": [1, 2, 3]}"); /* [2,4,6] */

    /* The operator's error path surfaces as a regular evaluation error. */
    apply_expect_error(engine, "error:", "{\"double\": []}", "{}");

    datalogic_engine_free(engine);
    return 0;
}
