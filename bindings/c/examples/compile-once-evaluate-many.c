/* compile-once-evaluate-many: compile a rule once, evaluate it against many
 * payloads, and print the last result plus a rough per-evaluation cost.
 * First over JSON strings (re-parsed per call), then over pre-parsed data
 * handles (the hot path: zero parse work per call), and finally as one
 * batch call that returns every result in order.
 *
 * Build the cdylib + header once from the repo root:
 *   cargo build --release --manifest-path bindings/c/Cargo.toml
 * then, from bindings/c/examples/:
 *   make run
 */

/* Expose clock_gettime / CLOCK_MONOTONIC from <time.h> under strict -std=c11
 * (glibc hides POSIX symbols unless a feature-test macro is set). */
#define _POSIX_C_SOURCE 200809L

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#include "datalogic.h"

#define ITERATIONS 100000
#define PAYLOADS 100

static void die(const char *what, datalogic_error *err) {
    fprintf(stderr, "compile-once-evaluate-many: %s failed", what);
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

static uint64_t now_ns(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (uint64_t)ts.tv_sec * 1000000000ULL + (uint64_t)ts.tv_nsec;
}

int main(void) {
    if (datalogic_abi_version() != DATALOGIC_ABI_VERSION) {
        fprintf(stderr, "ABI version mismatch: lib=%u header=%u\n",
                datalogic_abi_version(), (unsigned)DATALOGIC_ABI_VERSION);
        return 1;
    }

    const char *rule_json =
        "{\"*\": [{\"var\": \"price\"}, {\"-\": [1, {\"var\": \"discount\"}]}]}";

    datalogic_engine *engine = datalogic_engine_new(0);
    if (engine == NULL) die("engine_new", NULL);

    datalogic_error *err = NULL;
    datalogic_rule *rule = NULL;
    if (datalogic_engine_compile(engine, (const uint8_t *)rule_json,
                                 strlen(rule_json), &rule,
                                 &err) != DATALOGIC_STATUS_OK)
        die("engine_compile", err);

    datalogic_session *session = datalogic_engine_session(engine);
    if (session == NULL) die("engine_session", NULL);

    /* The session result is borrowed and valid only until the next call on
     * this session, so copy the current result into `last` each iteration. */
    char last[64];
    size_t last_len = 0;
    const uint8_t *out_ptr = NULL;
    size_t out_len = 0;

    /* Tier 1 -- compiled rule, JSON-string data (re-parsed per call). */
    uint64_t start = now_ns();
    for (int i = 0; i < ITERATIONS; i++) {
        char data[64];
        int dn = snprintf(data, sizeof(data),
                          "{\"price\": %d, \"discount\": 0.2}", 100 + i % PAYLOADS);
        if (datalogic_session_evaluate(session, rule, (const uint8_t *)data,
                                       (size_t)dn, &out_ptr, &out_len,
                                       &err) != DATALOGIC_STATUS_OK)
            die("session_evaluate", err);
        last_len = out_len < sizeof(last) ? out_len : sizeof(last) - 1;
        memcpy(last, out_ptr, last_len);
    }
    uint64_t elapsed = now_ns() - start;
    printf("string data:  last result %.*s, %d evaluations, ~%llu ns/op\n",
           (int)last_len, last, ITERATIONS,
           (unsigned long long)(elapsed / ITERATIONS));

    /* Tier 2 -- session + pre-parsed data handles: parse each distinct
     * payload once, then every evaluation skips JSON parsing entirely. */
    datalogic_data *handles[PAYLOADS];
    for (int i = 0; i < PAYLOADS; i++) {
        char data[64];
        int dn = snprintf(data, sizeof(data),
                          "{\"price\": %d, \"discount\": 0.2}", 100 + i);
        if (datalogic_data_parse((const uint8_t *)data, (size_t)dn, &handles[i],
                                 &err) != DATALOGIC_STATUS_OK)
            die("data_parse", err);
    }

    start = now_ns();
    for (int i = 0; i < ITERATIONS; i++) {
        if (datalogic_session_evaluate_data(session, rule, handles[i % PAYLOADS],
                                            &out_ptr, &out_len,
                                            &err) != DATALOGIC_STATUS_OK)
            die("session_evaluate_data", err);
        last_len = out_len < sizeof(last) ? out_len : sizeof(last) - 1;
        memcpy(last, out_ptr, last_len);
    }
    elapsed = now_ns() - start;
    printf("data handles: last result %.*s, %d evaluations, ~%llu ns/op\n",
           (int)last_len, last, ITERATIONS,
           (unsigned long long)(elapsed / ITERATIONS));

    /* Tier 3 -- one native call for the whole set: per-item results (and
     * per-item statuses) come back in order, borrowing the session buffer. */
    datalogic_slice results[PAYLOADS];
    datalogic_status statuses[PAYLOADS];
    if (datalogic_session_evaluate_batch(session, rule,
                                         (const datalogic_data *const *)handles,
                                         PAYLOADS, results, statuses,
                                         &err) != DATALOGIC_STATUS_OK)
        die("session_evaluate_batch", err);
    printf("batch:        %d results in one call, first %.*s, last %.*s\n",
           PAYLOADS, (int)results[0].len, (const char *)results[0].ptr,
           (int)results[PAYLOADS - 1].len, (const char *)results[PAYLOADS - 1].ptr);

    for (int i = 0; i < PAYLOADS; i++) datalogic_data_free(handles[i]);
    datalogic_session_free(session);
    datalogic_rule_free(rule);
    datalogic_engine_free(engine);
    return 0;
}
