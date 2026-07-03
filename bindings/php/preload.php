<?php

declare(strict_types=1);

/*
 * SPDX-License-Identifier: Apache-2.0
 *
 * opcache.preload entry point for the datalogic binding.
 *
 * Registers the persistent FFI scope "datalogic" at server start so
 * request-time code pays zero header parsing and works under the
 * default `ffi.enable=preload` (which forbids runtime FFI::cdef in
 * non-CLI SAPIs). Wire it up in php.ini:
 *
 *   opcache.preload=/path/to/vendor/goplasmatic/datalogic/preload.php
 *   opcache.preload_user=www-data
 *   ffi.enable=preload
 *
 * The script resolves the native library exactly like the runtime
 * loader (DATALOGIC_NATIVE_LIB env var, the package's lib/<os>-<arch>/
 * layout, the in-tree C ABI target dir, then the OS loader path),
 * rewrites the header's FFI_LIB line to that path, and FFI::load()s
 * it. To pin a specific library, set DATALOGIC_NATIVE_LIB before the
 * server starts.
 *
 * If your deployment already has an application-wide preload script,
 * `require` this file from it (idempotent), or call
 * \Goplasmatic\Datalogic\Internal\Native::preload() yourself.
 */

require_once __DIR__ . '/src/Internal/Native.php';

\Goplasmatic\Datalogic\Internal\Native::preload();

// Warm the wrapper classes into opcache alongside the FFI scope (a
// nicety, not a requirement — skipped when opcache is absent, e.g.
// when this script is required outside opcache.preload).
if (function_exists('opcache_compile_file')) {
    $files = array_merge(
        glob(__DIR__ . '/src/*.php') ?: [],
        glob(__DIR__ . '/src/*/*.php') ?: [],
    );
    foreach ($files as $file) {
        // Native.php was require'd above — compiling it again would
        // warn ("already declared").
        if (realpath($file) === realpath(__DIR__ . '/src/Internal/Native.php')) {
            continue;
        }
        @opcache_compile_file($file);
    }
}
