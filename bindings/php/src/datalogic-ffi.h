#define FFI_SCOPE "datalogic"
#define FFI_LIB "libdatalogic_c.so"

/*
 * SPDX-License-Identifier: Apache-2.0
 *
 * PHP-FFI declarations for libdatalogic_c — C ABI v2.
 *
 * This file is BOTH the `FFI::load` / opcache.preload header and the
 * source of the `FFI::cdef` declarations (Native.php reads it and
 * strips the `#define` lines), so the two load paths cannot drift
 * apart. Keep it in sync with `bindings/c/include/datalogic.h`.
 *
 * Deliberate deviations from datalogic.h — all ABI-identical:
 *
 *  - `datalogic_status` is declared as `typedef int32_t` instead of a
 *    C enum. The enum's values (OK=0, INVALID_ARG=1, PARSE=2, EVAL=3,
 *    TYPE_MISMATCH=4, INTERNAL=5) are mirrored as Native::STATUS_*.
 *  - Byte-INPUT parameters are declared `const char *` instead of
 *    `const uint8_t *`: PHP FFI passes a PHP string zero-copy to a
 *    `char*` parameter but refuses the coercion for `uint8_t*`. The
 *    length is always passed explicitly — nothing reads for a NUL.
 *  - Byte-OUTPUT pointers (error accessors, borrowed session results,
 *    slice/buf fields, callback args) stay `uint8_t *`: a `const
 *    char *` RETURN value would be auto-converted to a PHP string by
 *    scanning for a NUL terminator, and v2 bytes are not
 *    NUL-terminated. `uint8_t*` arrives as CData and is copied with
 *    FFI::string($ptr, $len).
 *
 * PHP FFI cannot process #include — only the two #define lines above
 * are preprocessor directives, and only fixed-width types PHP FFI
 * knows natively (uint8_t, int32_t, int64_t, uint32_t, size_t,
 * double, void) are used.
 *
 * FFI_LIB above is a default for hand-rolled preload scripts on
 * Linux; `preload.php` / Native::preload() rewrite that line to the
 * resolved absolute library path before calling FFI::load, so the
 * committed value is only a fallback for users who load this header
 * verbatim with the library on the OS loader path.
 */

typedef struct datalogic_engine datalogic_engine;
typedef struct datalogic_engine_builder datalogic_engine_builder;
typedef struct datalogic_rule datalogic_rule;
typedef struct datalogic_data datalogic_data;
typedef struct datalogic_session datalogic_session;
typedef struct datalogic_traced_session datalogic_traced_session;
typedef struct datalogic_error datalogic_error;
typedef struct datalogic_op_result datalogic_op_result;

/* Enum in datalogic.h; int-backed there, int32_t here (ABI-identical). */
typedef int32_t datalogic_status;

/* Owned byte buffer; release via datalogic_buf_free (by value). */
typedef struct {
    uint8_t *ptr;
    size_t len;
    size_t cap;
} datalogic_buf;

/* Borrowed byte range (session-owned; copy before the next session call). */
typedef struct {
    const uint8_t *ptr;
    size_t len;
} datalogic_slice;

/* Custom-operator callback: args are borrowed, NOT NUL-terminated. */
typedef int32_t (*datalogic_op_fn)(const uint8_t *args_json,
                                   size_t args_len,
                                   void *user_data,
                                   datalogic_op_result *out);

/* --- Meta --- */
uint32_t datalogic_abi_version(void);
const char *datalogic_version(void);
void datalogic_buf_free(datalogic_buf buf);

/* --- Custom-operator result setters (valid only inside the callback) --- */
void datalogic_op_result_set_json(datalogic_op_result *out, const char *json, size_t len);
void datalogic_op_result_set_error(datalogic_op_result *out, const char *msg, size_t len);

/* --- Engine builder --- */
datalogic_engine_builder *datalogic_engine_builder_new(void);
void datalogic_engine_builder_free(datalogic_engine_builder *builder);
void datalogic_engine_builder_set_templating(datalogic_engine_builder *builder, int32_t enabled);
datalogic_status datalogic_engine_builder_set_config_json(datalogic_engine_builder *builder,
                                                          const char *config_json,
                                                          size_t config_len,
                                                          datalogic_error **err);
datalogic_status datalogic_engine_builder_add_operator(datalogic_engine_builder *builder,
                                                       const char *name,
                                                       size_t name_len,
                                                       datalogic_op_fn callback,
                                                       void *user_data,
                                                       datalogic_error **err);
datalogic_engine *datalogic_engine_builder_build(datalogic_engine_builder *builder);

/* --- Data handles (parse once, evaluate many) --- */
datalogic_status datalogic_data_parse(const char *json,
                                      size_t len,
                                      datalogic_data **out,
                                      datalogic_error **err);
void datalogic_data_free(datalogic_data *data);
size_t datalogic_data_allocated_bytes(const datalogic_data *data);

/* --- Engine --- */
datalogic_engine *datalogic_engine_new(int32_t templating);
void datalogic_engine_free(datalogic_engine *engine);
datalogic_status datalogic_engine_compile(const datalogic_engine *engine,
                                          const char *rule_json,
                                          size_t rule_len,
                                          datalogic_rule **out_rule,
                                          datalogic_error **err);
datalogic_status datalogic_engine_apply(const datalogic_engine *engine,
                                        const char *rule_json,
                                        size_t rule_len,
                                        const char *data_json,
                                        size_t data_len,
                                        datalogic_buf *out,
                                        datalogic_error **err);
datalogic_session *datalogic_engine_session(const datalogic_engine *engine);

/* --- Errors (owned handles; accessors borrow until _free) --- */
void datalogic_error_free(datalogic_error *err);
datalogic_status datalogic_error_status(const datalogic_error *err);
const uint8_t *datalogic_error_message(const datalogic_error *err, size_t *len_out);
const uint8_t *datalogic_error_tag(const datalogic_error *err, size_t *len_out);
const uint8_t *datalogic_error_operator(const datalogic_error *err, size_t *len_out);
const uint8_t *datalogic_error_path_json(const datalogic_error *err, size_t *len_out);

/* --- Rules --- */
void datalogic_rule_free(datalogic_rule *rule);
datalogic_status datalogic_rule_evaluate(const datalogic_rule *rule,
                                         const char *data_json,
                                         size_t data_len,
                                         datalogic_buf *out,
                                         datalogic_error **err);
datalogic_status datalogic_rule_evaluate_data(const datalogic_rule *rule,
                                              const datalogic_data *data,
                                              datalogic_buf *out,
                                              datalogic_error **err);

/* --- Sessions (single-threaded; results borrowed until next call) --- */
void datalogic_session_free(datalogic_session *session);
void datalogic_session_reset(datalogic_session *session);
size_t datalogic_session_allocated_bytes(const datalogic_session *session);
datalogic_status datalogic_session_evaluate(datalogic_session *session,
                                            const datalogic_rule *rule,
                                            const char *data_json,
                                            size_t data_len,
                                            const uint8_t **out_ptr,
                                            size_t *out_len,
                                            datalogic_error **err);
datalogic_status datalogic_session_evaluate_data(datalogic_session *session,
                                                 const datalogic_rule *rule,
                                                 const datalogic_data *data,
                                                 const uint8_t **out_ptr,
                                                 size_t *out_len,
                                                 datalogic_error **err);
datalogic_status datalogic_session_evaluate_bool(datalogic_session *session,
                                                 const datalogic_rule *rule,
                                                 const datalogic_data *data,
                                                 int32_t *out,
                                                 datalogic_error **err);
datalogic_status datalogic_session_evaluate_i64(datalogic_session *session,
                                                const datalogic_rule *rule,
                                                const datalogic_data *data,
                                                int64_t *out,
                                                datalogic_error **err);
datalogic_status datalogic_session_evaluate_f64(datalogic_session *session,
                                                const datalogic_rule *rule,
                                                const datalogic_data *data,
                                                double *out,
                                                datalogic_error **err);
datalogic_status datalogic_session_evaluate_truthy(datalogic_session *session,
                                                   const datalogic_rule *rule,
                                                   const datalogic_data *data,
                                                   int32_t *out,
                                                   datalogic_error **err);
datalogic_status datalogic_session_evaluate_batch(datalogic_session *session,
                                                  const datalogic_rule *rule,
                                                  const datalogic_data *const *datas,
                                                  size_t n,
                                                  datalogic_slice *out_results,
                                                  datalogic_status *out_statuses,
                                                  datalogic_error **err);
datalogic_status datalogic_session_evaluate_many(datalogic_session *session,
                                                 const datalogic_rule *const *rules,
                                                 size_t n,
                                                 const datalogic_data *data,
                                                 datalogic_slice *out_results,
                                                 datalogic_status *out_statuses,
                                                 datalogic_error **err);

/* --- Traced sessions --- */
datalogic_traced_session *datalogic_engine_traced_session(const datalogic_engine *engine);
void datalogic_traced_session_free(datalogic_traced_session *session);
datalogic_status datalogic_traced_session_evaluate(const datalogic_traced_session *session,
                                                   const char *rule_json,
                                                   size_t rule_len,
                                                   const char *data_json,
                                                   size_t data_len,
                                                   datalogic_buf *out,
                                                   datalogic_error **err);
