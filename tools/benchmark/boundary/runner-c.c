/* Boundary-benchmark runner for the C ABI (`runtime: "c-abi"`), v2 contract.
 *
 * Modes:
 *   session-evaluate           JSON text in, borrowed bytes out (hot string path)
 *   session-evaluate-data      parsed-data handle in, borrowed bytes out (hot handle path)
 *   session-evaluate-many-100  100 identical rules x 1 data handle via
 *                              datalogic_session_evaluate_many; ns_op is
 *                              reported PER EVALUATION (call time / 100)
 *   rule-evaluate              JSON text in, owned datalogic_buf out (+ buf_free)
 *   engine-apply-oneshot       compile + evaluate per call, owned buf out
 *
 * Timing discipline (BINDINGS-OVERHEAD.md appendix): warmup 2,000
 * iterations (native tier), pilot pass sizing N so one timed sample lands
 * near ~250 ms, median of 5 samples, results consumed into a volatile
 * sink. Emits JSON lines to stdout:
 *   {"runtime": "c-abi", "mode": "...", "workload": "...", "ns_op": <float>}
 *
 * Build (see run.sh):
 *   cc -O2 runner-c.c -I ../../../bindings/c/include \
 *      -L ../../../bindings/c/target/release -ldatalogic_c -o runner-c
 *
 * Usage: runner-c <workloads-dir> [--modes=a,b] [--workloads=x,y]
 */

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#include "datalogic.h"

#define WARMUP 2000ULL
#define TARGET_SAMPLE_NS 250e6
#define SAMPLES 5
#define PILOT_MIN_NS 10000000ULL
#define MANY_N 100

/* Consumed results land here; volatile so the compiler can't elide the
 * evaluations that produced them. */
static volatile uint64_t g_sink;

static uint64_t now_ns(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (uint64_t)ts.tv_sec * 1000000000ULL + (uint64_t)ts.tv_nsec;
}

static void die(const char *what, datalogic_error *err) {
    fprintf(stderr, "runner-c: %s failed", what);
    if (err != NULL) {
        size_t mlen = 0, tlen = 0;
        const uint8_t *msg = datalogic_error_message(err, &mlen);
        const uint8_t *tag = datalogic_error_tag(err, &tlen);
        fprintf(stderr, ": [%.*s] %.*s", (int)tlen, (const char *)tag, (int)mlen,
                (const char *)msg);
        datalogic_error_free(err);
    }
    fprintf(stderr, "\n");
    exit(1);
}

static char *read_file(const char *dir, const char *name, const char *suffix,
                       size_t *len_out) {
    char path[4096];
    snprintf(path, sizeof(path), "%s/%s.%s.json", dir, name, suffix);
    FILE *f = fopen(path, "rb");
    if (f == NULL) {
        fprintf(stderr, "runner-c: cannot open %s\n", path);
        exit(1);
    }
    fseek(f, 0, SEEK_END);
    long sz = ftell(f);
    fseek(f, 0, SEEK_SET);
    char *buf = malloc((size_t)sz + 1);
    if (buf == NULL || fread(buf, 1, (size_t)sz, f) != (size_t)sz) {
        fprintf(stderr, "runner-c: short read on %s\n", path);
        exit(1);
    }
    buf[sz] = '\0';
    fclose(f);
    *len_out = (size_t)sz;
    return buf;
}

typedef struct {
    const char *name;
    char *rule;
    size_t rule_len;
    char *data;
    size_t data_len;
    char *expected;
    size_t expected_len;
} workload;

/* Everything a mode's batch function needs, prepared once per workload. */
typedef struct {
    const workload *w;
    datalogic_engine *engine;
    datalogic_rule *rule;
    datalogic_session *session;
    datalogic_data *data_handle;
    /* session-evaluate-many-100: 100 separately-compiled handles of the
     * same rule JSON (a rule-set of identical rules — separate compiles
     * so the batch doesn't flatter a single hot compiled tree). */
    const datalogic_rule *many_rules[MANY_N];
    datalogic_slice many_results[MANY_N];
    datalogic_status many_statuses[MANY_N];
} mode_ctx;

typedef uint64_t (*batch_fn)(mode_ctx *ctx, uint64_t n);

/* ------------------------- batch functions ------------------------- */
/* Hot loops pass err = NULL: error capture is opt-in in the v2 contract
 * and skipping it is the documented fast path. Verification (below) runs
 * each call once WITH capture before any timing. */

static uint64_t batch_session_evaluate(mode_ctx *c, uint64_t n) {
    uint64_t sink = 0;
    const uint8_t *out_ptr;
    size_t out_len;
    for (uint64_t i = 0; i < n; i++) {
        if (datalogic_session_evaluate(c->session, c->rule,
                                       (const uint8_t *)c->w->data, c->w->data_len,
                                       &out_ptr, &out_len, NULL) != DATALOGIC_STATUS_OK)
            die("session_evaluate (timed)", NULL);
        sink += out_len + (uint64_t)out_ptr[0];
    }
    return sink;
}

static uint64_t batch_session_evaluate_data(mode_ctx *c, uint64_t n) {
    uint64_t sink = 0;
    const uint8_t *out_ptr;
    size_t out_len;
    for (uint64_t i = 0; i < n; i++) {
        if (datalogic_session_evaluate_data(c->session, c->rule, c->data_handle,
                                            &out_ptr, &out_len,
                                            NULL) != DATALOGIC_STATUS_OK)
            die("session_evaluate_data (timed)", NULL);
        sink += out_len + (uint64_t)out_ptr[0];
    }
    return sink;
}

static uint64_t batch_session_evaluate_many(mode_ctx *c, uint64_t n) {
    uint64_t sink = 0;
    for (uint64_t i = 0; i < n; i++) {
        if (datalogic_session_evaluate_many(c->session, c->many_rules, MANY_N,
                                            c->data_handle, c->many_results,
                                            c->many_statuses,
                                            NULL) != DATALOGIC_STATUS_OK)
            die("session_evaluate_many (timed)", NULL);
        sink += c->many_results[0].len + c->many_results[MANY_N - 1].len;
    }
    return sink;
}

static uint64_t batch_rule_evaluate(mode_ctx *c, uint64_t n) {
    uint64_t sink = 0;
    for (uint64_t i = 0; i < n; i++) {
        datalogic_buf buf;
        if (datalogic_rule_evaluate(c->rule, (const uint8_t *)c->w->data,
                                    c->w->data_len, &buf, NULL) != DATALOGIC_STATUS_OK)
            die("rule_evaluate (timed)", NULL);
        sink += buf.len + (uint64_t)buf.ptr[0];
        datalogic_buf_free(buf);
    }
    return sink;
}

static uint64_t batch_engine_apply(mode_ctx *c, uint64_t n) {
    uint64_t sink = 0;
    for (uint64_t i = 0; i < n; i++) {
        datalogic_buf buf;
        if (datalogic_engine_apply(c->engine, (const uint8_t *)c->w->rule,
                                   c->w->rule_len, (const uint8_t *)c->w->data,
                                   c->w->data_len, &buf, NULL) != DATALOGIC_STATUS_OK)
            die("engine_apply (timed)", NULL);
        sink += buf.len + (uint64_t)buf.ptr[0];
        datalogic_buf_free(buf);
    }
    return sink;
}

/* --------------------------- verification --------------------------- */

static void check_bytes(const char *mode, const char *workload_name,
                        const uint8_t *got, size_t got_len, const workload *w) {
    if (got_len != w->expected_len || memcmp(got, w->expected, got_len) != 0) {
        fprintf(stderr,
                "runner-c: verification failed for mode=%s workload=%s\n"
                "  expected: %.*s\n  got:      %.*s\n",
                mode, workload_name, (int)w->expected_len, w->expected, (int)got_len,
                (const char *)got);
        exit(1);
    }
}

/* Run each mode once with error capture on and byte-compare the result
 * against the checked-in expectation before any timing. */
static void verify_mode(mode_ctx *c, const char *mode) {
    datalogic_error *err = NULL;
    if (strcmp(mode, "session-evaluate") == 0) {
        const uint8_t *p;
        size_t l;
        if (datalogic_session_evaluate(c->session, c->rule, (const uint8_t *)c->w->data,
                                       c->w->data_len, &p, &l,
                                       &err) != DATALOGIC_STATUS_OK)
            die("session_evaluate (verify)", err);
        check_bytes(mode, c->w->name, p, l, c->w);
    } else if (strcmp(mode, "session-evaluate-data") == 0) {
        const uint8_t *p;
        size_t l;
        if (datalogic_session_evaluate_data(c->session, c->rule, c->data_handle, &p, &l,
                                            &err) != DATALOGIC_STATUS_OK)
            die("session_evaluate_data (verify)", err);
        check_bytes(mode, c->w->name, p, l, c->w);
    } else if (strcmp(mode, "session-evaluate-many-100") == 0) {
        if (datalogic_session_evaluate_many(c->session, c->many_rules, MANY_N,
                                            c->data_handle, c->many_results,
                                            c->many_statuses,
                                            &err) != DATALOGIC_STATUS_OK)
            die("session_evaluate_many (verify)", err);
        for (int i = 0; i < MANY_N; i++) {
            if (c->many_statuses[i] != DATALOGIC_STATUS_OK) {
                fprintf(stderr, "runner-c: many item %d status %d (workload=%s)\n", i,
                        (int)c->many_statuses[i], c->w->name);
                exit(1);
            }
            check_bytes(mode, c->w->name, c->many_results[i].ptr, c->many_results[i].len,
                        c->w);
        }
    } else if (strcmp(mode, "rule-evaluate") == 0) {
        datalogic_buf buf;
        if (datalogic_rule_evaluate(c->rule, (const uint8_t *)c->w->data, c->w->data_len,
                                    &buf, &err) != DATALOGIC_STATUS_OK)
            die("rule_evaluate (verify)", err);
        check_bytes(mode, c->w->name, buf.ptr, buf.len, c->w);
        datalogic_buf_free(buf);
    } else if (strcmp(mode, "engine-apply-oneshot") == 0) {
        datalogic_buf buf;
        if (datalogic_engine_apply(c->engine, (const uint8_t *)c->w->rule, c->w->rule_len,
                                   (const uint8_t *)c->w->data, c->w->data_len, &buf,
                                   &err) != DATALOGIC_STATUS_OK)
            die("engine_apply (verify)", err);
        check_bytes(mode, c->w->name, buf.ptr, buf.len, c->w);
        datalogic_buf_free(buf);
    }
}

/* ------------------------------ timing ------------------------------ */

static double measure(batch_fn fn, mode_ctx *ctx) {
    g_sink += fn(ctx, WARMUP);

    uint64_t n = 32;
    double per_op;
    for (;;) {
        uint64_t t0 = now_ns();
        g_sink += fn(ctx, n);
        uint64_t elapsed = now_ns() - t0;
        if (elapsed >= PILOT_MIN_NS) {
            per_op = (double)elapsed / (double)n;
            break;
        }
        n *= 2;
    }

    uint64_t iters = (uint64_t)(TARGET_SAMPLE_NS / per_op);
    if (iters < 1) iters = 1;

    double samples[SAMPLES];
    for (int s = 0; s < SAMPLES; s++) {
        uint64_t t0 = now_ns();
        g_sink += fn(ctx, iters);
        samples[s] = (double)(now_ns() - t0) / (double)iters;
    }
    /* insertion-sort 5 values, take the median */
    for (int i = 1; i < SAMPLES; i++) {
        double v = samples[i];
        int j = i - 1;
        while (j >= 0 && samples[j] > v) {
            samples[j + 1] = samples[j];
            j--;
        }
        samples[j + 1] = v;
    }
    return samples[SAMPLES / 2];
}

typedef struct {
    const char *name;
    batch_fn fn;
    /* ns_op divisor: 1 for per-call modes, MANY_N for the batch mode
     * (one call performs MANY_N evaluations). */
    double per_call_evals;
} mode_spec;

static const mode_spec MODE_SPECS[] = {
    {"session-evaluate", batch_session_evaluate, 1.0},
    {"session-evaluate-data", batch_session_evaluate_data, 1.0},
    {"session-evaluate-many-100", batch_session_evaluate_many, (double)MANY_N},
    {"rule-evaluate", batch_rule_evaluate, 1.0},
    {"engine-apply-oneshot", batch_engine_apply, 1.0},
};

static int selected(const char *csv, const char *name) {
    if (csv == NULL) return 1;
    size_t nlen = strlen(name);
    const char *p = csv;
    while (*p) {
        const char *comma = strchr(p, ',');
        size_t seg = comma ? (size_t)(comma - p) : strlen(p);
        if (seg == nlen && strncmp(p, name, nlen) == 0) return 1;
        p += seg;
        if (*p == ',') p++;
    }
    return 0;
}

int main(int argc, char **argv) {
    const char *dir = NULL;
    const char *modes_csv = NULL;
    const char *workloads_csv = NULL;
    for (int i = 1; i < argc; i++) {
        if (strncmp(argv[i], "--modes=", 8) == 0)
            modes_csv = argv[i] + 8;
        else if (strncmp(argv[i], "--workloads=", 12) == 0)
            workloads_csv = argv[i] + 12;
        else
            dir = argv[i];
    }
    if (dir == NULL) {
        fprintf(stderr, "usage: runner-c <workloads-dir> [--modes=a,b] [--workloads=x,y]\n");
        return 1;
    }

    if (datalogic_abi_version() != DATALOGIC_ABI_VERSION) {
        fprintf(stderr, "runner-c: ABI version mismatch: lib=%u header=%u\n",
                datalogic_abi_version(), (unsigned)DATALOGIC_ABI_VERSION);
        return 1;
    }

    static const char *NAMES[] = {"simple", "eligibility", "array100"};
    datalogic_engine *engine = datalogic_engine_new(0);
    if (engine == NULL) die("engine_new", NULL);

    for (size_t wi = 0; wi < 3; wi++) {
        if (!selected(workloads_csv, NAMES[wi])) continue;

        workload w;
        w.name = NAMES[wi];
        w.rule = read_file(dir, w.name, "rule", &w.rule_len);
        w.data = read_file(dir, w.name, "data", &w.data_len);
        w.expected = read_file(dir, w.name, "expected", &w.expected_len);

        mode_ctx ctx;
        memset(&ctx, 0, sizeof(ctx));
        ctx.w = &w;
        ctx.engine = engine;

        datalogic_error *err = NULL;
        if (datalogic_engine_compile(engine, (const uint8_t *)w.rule, w.rule_len,
                                     &ctx.rule, &err) != DATALOGIC_STATUS_OK)
            die("engine_compile", err);
        ctx.session = datalogic_engine_session(engine);
        if (ctx.session == NULL) die("engine_session", NULL);
        if (datalogic_data_parse((const uint8_t *)w.data, w.data_len, &ctx.data_handle,
                                 &err) != DATALOGIC_STATUS_OK)
            die("data_parse", err);
        datalogic_rule *many_owned[MANY_N];
        for (int i = 0; i < MANY_N; i++) {
            if (datalogic_engine_compile(engine, (const uint8_t *)w.rule, w.rule_len,
                                         &many_owned[i], &err) != DATALOGIC_STATUS_OK)
                die("engine_compile (many)", err);
            ctx.many_rules[i] = many_owned[i];
        }

        for (size_t mi = 0; mi < sizeof(MODE_SPECS) / sizeof(MODE_SPECS[0]); mi++) {
            const mode_spec *m = &MODE_SPECS[mi];
            if (!selected(modes_csv, m->name)) continue;
            verify_mode(&ctx, m->name);
            double ns_per_call = measure(m->fn, &ctx);
            printf("{\"runtime\": \"c-abi\", \"mode\": \"%s\", \"workload\": \"%s\", "
                   "\"ns_op\": %.3f}\n",
                   m->name, w.name, ns_per_call / m->per_call_evals);
            fflush(stdout);
        }

        for (int i = 0; i < MANY_N; i++) datalogic_rule_free(many_owned[i]);
        datalogic_data_free(ctx.data_handle);
        datalogic_session_free(ctx.session);
        datalogic_rule_free(ctx.rule);
        free(w.rule);
        free(w.data);
        free(w.expected);
    }

    datalogic_engine_free(engine);
    /* Keep the sink observable so the whole program can't be folded. */
    fprintf(stderr, "runner-c: sink=%llu\n", (unsigned long long)g_sink);
    return 0;
}
