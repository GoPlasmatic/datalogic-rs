<?php

declare(strict_types=1);

/*
 * SPDX-License-Identifier: Apache-2.0
 *
 * FFI singleton for libdatalogic_c. The signature list below is a curated
 * subset of `bindings/c/include/datalogic.h` — PHP's FFI::cdef parser is
 * stricter than a full C compiler (no #include / #ifdef / extern "C" /
 * attributes), so we mirror only the typedefs and function declarations
 * the binding actually uses.
 *
 * Keep in sync when the C ABI surface changes. The C smoke tests + a
 * `Native::load()` call in this binding's test suite are the first lines
 * of defense.
 */

namespace Goplasmatic\Datalogic\Internal;

use FFI;

/**
 * Lazy, process-wide FFI singleton. The C cdylib is loaded once and
 * reused for every call. Use {@see Native::ffi()} to get the FFI
 * instance.
 */
final class Native
{
    private static ?FFI $ffi = null;

    /** PHP-FFI-friendly C signatures. */
    private const HEADER = <<<'C'
typedef struct datalogic_engine datalogic_engine;
typedef struct datalogic_engine_builder datalogic_engine_builder;
typedef struct datalogic_rule datalogic_rule;
typedef struct datalogic_session datalogic_session;
typedef struct datalogic_traced_session datalogic_traced_session;

typedef char *(*datalogic_op_callback)(const char *args_json, void *user_data, char **error_out);

const char *datalogic_version(void);
void datalogic_string_free(char *ptr);

datalogic_engine *datalogic_engine_new(int templating);
void datalogic_engine_free(datalogic_engine *engine);
datalogic_rule *datalogic_engine_compile(datalogic_engine *engine, const char *rule_json);
char *datalogic_engine_apply(datalogic_engine *engine, const char *rule_json, const char *data_json);
datalogic_session *datalogic_engine_session(datalogic_engine *engine);
datalogic_traced_session *datalogic_engine_traced_session(datalogic_engine *engine);

datalogic_engine_builder *datalogic_engine_builder_new(void);
void datalogic_engine_builder_free(datalogic_engine_builder *builder);
void datalogic_engine_builder_set_templating(datalogic_engine_builder *builder, int enabled);
int datalogic_engine_builder_add_operator(datalogic_engine_builder *builder, const char *name, datalogic_op_callback callback, void *user_data);
datalogic_engine *datalogic_engine_builder_build(datalogic_engine_builder *builder);

void datalogic_rule_free(datalogic_rule *rule);
char *datalogic_rule_evaluate(datalogic_rule *rule, const char *data_json);

void datalogic_session_free(datalogic_session *session);
char *datalogic_session_evaluate(datalogic_session *session, datalogic_rule *rule, const char *data_json);
void datalogic_session_reset(datalogic_session *session);
size_t datalogic_session_allocated_bytes(datalogic_session *session);

void datalogic_traced_session_free(datalogic_traced_session *session);
char *datalogic_traced_session_evaluate(datalogic_traced_session *session, const char *rule_json, const char *data_json);

void datalogic_last_error_clear(void);
const char *datalogic_last_error_message(void);
const char *datalogic_last_error_type(void);
const char *datalogic_last_error_operator(void);
const char *datalogic_last_error_path_json(void);
C;

    /**
     * Resolve and load the cdylib. Lookup order:
     *  1. `DATALOGIC_NATIVE_LIB` env var (absolute path)
     *  2. `lib/<os>-<arch>/libdatalogic_c.<ext>` relative to this package
     *     (populated by the release workflow)
     *  3. `../c/target/release/libdatalogic_c.<ext>` for in-tree dev
     *  4. The OS's default library search path (LD_LIBRARY_PATH, etc.)
     */
    public static function ffi(): FFI
    {
        if (self::$ffi !== null) {
            return self::$ffi;
        }
        $path = self::locateLibrary();
        self::$ffi = FFI::cdef(self::HEADER, $path);
        return self::$ffi;
    }

    /** Return the resolved cdylib path. */
    public static function locateLibrary(): string
    {
        $env = getenv('DATALOGIC_NATIVE_LIB');
        if (is_string($env) && $env !== '' && is_file($env)) {
            return $env;
        }
        $filename = self::libraryFileName();
        $platform = self::platformDir();

        $candidates = [
            __DIR__ . '/../../lib/' . $platform . '/' . $filename,
            __DIR__ . '/../../../c/target/release/' . $filename,
        ];
        foreach ($candidates as $candidate) {
            if (is_file($candidate)) {
                return $candidate;
            }
        }
        // Last resort: bare filename — PHP FFI will hand it to the OS
        // loader, which checks LD_LIBRARY_PATH / DYLD_LIBRARY_PATH / PATH.
        return $filename;
    }

    /** Platform-conventional cdylib filename. */
    private static function libraryFileName(): string
    {
        return match (PHP_OS_FAMILY) {
            'Windows' => 'datalogic_c.dll',
            'Darwin'  => 'libdatalogic_c.dylib',
            default   => 'libdatalogic_c.so',
        };
    }

    /** Platform directory name matching the release workflow's lib/ layout. */
    private static function platformDir(): string
    {
        $os = match (PHP_OS_FAMILY) {
            'Windows' => 'windows',
            'Darwin'  => 'darwin',
            default   => 'linux',
        };
        $arch = match (php_uname('m')) {
            'x86_64', 'amd64' => 'x86_64',
            'arm64', 'aarch64' => 'aarch64',
            default => php_uname('m'),
        };
        return $os . '-' . $arch;
    }

    /**
     * Convert a borrowed C string (library-owned, do not free) into a
     * PHP string, or null if the pointer is NULL. PHP FFI auto-converts
     * `const char*` return values to PHP `string` directly, so this
     * helper accepts both shapes (CData<char*> for non-const returns,
     * raw `string` for const returns).
     */
    public static function borrowString(mixed $ptr): ?string
    {
        if ($ptr === null) return null;
        if (is_string($ptr)) return $ptr;
        if ($ptr instanceof FFI\CData) {
            return FFI::string($ptr);
        }
        return null;
    }

    /**
     * Take ownership of a C string returned by an `_evaluate` / `_apply`
     * / `_string_free`-pairing entry point. Copies the bytes into a PHP
     * string and releases the native buffer. PHP FFI returns these as
     * `CData<char*>` (non-const) — no auto-conversion to string.
     */
    public static function takeString(?FFI\CData $ptr): ?string
    {
        if ($ptr === null) return null;
        $s = FFI::string($ptr);
        self::ffi()->datalogic_string_free($ptr);
        return $s;
    }
}
