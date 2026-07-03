<?php

declare(strict_types=1);

/*
 * SPDX-License-Identifier: Apache-2.0
 *
 * FFI singleton for libdatalogic_c (C ABI v2).
 *
 * The C declarations live in ONE place — `src/datalogic-ffi.h` — which
 * serves both load paths:
 *
 *  - preload:  `preload.php` (opcache.preload) rewrites the header's
 *    FFI_LIB line to the resolved absolute library path and calls
 *    FFI::load(), registering the persistent scope "datalogic". At
 *    request time {@see Native::ffi()} picks it up via FFI::scope()
 *    with zero header parsing.
 *  - cdef:     when no scope is preloaded, the same header is read,
 *    its `#define` lines stripped, and handed to FFI::cdef() together
 *    with the located library.
 *
 * Either way the very first call asserts `datalogic_abi_version()`
 * returns {@see Native::ABI_VERSION} and refuses to run otherwise.
 */

namespace Goplasmatic\Datalogic\Internal;

use FFI;
use FFI\CData;

/**
 * Lazy, process-wide FFI singleton over the v2 C ABI. Use
 * {@see Native::ffi()} to get the FFI instance.
 *
 * @internal
 */
final class Native
{
    /** FFI_SCOPE registered by preload.php / {@see Native::preload()}. */
    public const SCOPE = 'datalogic';

    /** The C ABI generation this binding is written against. */
    public const ABI_VERSION = 2;

    /* datalogic_status values (mirrors the datalogic_status enum). */
    public const STATUS_OK = 0;
    public const STATUS_INVALID_ARG = 1;
    public const STATUS_PARSE = 2;
    public const STATUS_EVAL = 3;
    public const STATUS_TYPE_MISMATCH = 4;
    public const STATUS_INTERNAL = 5;

    private static ?FFI $ffi = null;

    /**
     * Resolve the process-wide FFI instance: a preloaded
     * `FFI::scope("datalogic")` when available (opcache.preload +
     * preload.php), otherwise `FFI::cdef` over the header file and the
     * located cdylib. Asserts the ABI version on first load.
     */
    public static function ffi(): FFI
    {
        if (self::$ffi !== null) {
            return self::$ffi;
        }
        $ffi = self::fromPreloadedScope()
            ?? FFI::cdef(self::declarations(), self::locateLibrary());
        self::assertAbiVersion($ffi->datalogic_abi_version());
        return self::$ffi = $ffi;
    }

    /** The preloaded scope, or null when preload.php did not run. */
    private static function fromPreloadedScope(): ?FFI
    {
        try {
            return FFI::scope(self::SCOPE);
        } catch (FFI\Exception) {
            return null;
        }
    }

    /**
     * Refuse to run against a stale native library. Both load paths
     * funnel through this (with the library's reported
     * `datalogic_abi_version()`) before an instance is handed out.
     *
     * @throws \RuntimeException on an ABI generation mismatch
     */
    public static function assertAbiVersion(int $got): void
    {
        if ($got !== self::ABI_VERSION) {
            throw new \RuntimeException(sprintf(
                'libdatalogic_c ABI version mismatch: binding requires v%d, library reports v%d. ' .
                'Rebuild/upgrade the native library (bindings/c) to match this package.',
                self::ABI_VERSION,
                $got,
            ));
        }
    }

    /** Absolute path of the shared FFI header (`src/datalogic-ffi.h`). */
    public static function headerPath(): string
    {
        return dirname(__DIR__) . '/datalogic-ffi.h';
    }

    /**
     * The header's C declarations with every preprocessor line
     * (`#define FFI_SCOPE` / `#define FFI_LIB`) stripped — the exact
     * string handed to FFI::cdef. Reading the same file FFI::load
     * consumes keeps the two surfaces in sync by construction.
     */
    public static function declarations(): string
    {
        $header = @file_get_contents(self::headerPath());
        if ($header === false) {
            throw new \RuntimeException('datalogic FFI header not found: ' . self::headerPath());
        }
        $lines = array_filter(
            explode("\n", $header),
            static fn (string $line): bool => !str_starts_with(ltrim($line), '#'),
        );
        return implode("\n", $lines);
    }

    /**
     * FFI::load the header with its FFI_LIB line rewritten to the
     * resolved library path. Called by `preload.php` under
     * opcache.preload (registering the persistent "datalogic" scope);
     * callable directly (CLI, tests) in which case the returned FFI
     * instance is the only handle to the loaded surface — no scope is
     * registered outside preloading.
     *
     * @param string|null $library absolute cdylib path; defaults to
     *                             {@see Native::locateLibrary()}
     *
     * @throws \RuntimeException on load failure or ABI mismatch
     */
    public static function preload(?string $library = null): FFI
    {
        // Idempotent under repeated preload includes.
        $scoped = self::fromPreloadedScope();
        if ($scoped !== null) {
            return $scoped;
        }

        $library ??= self::locateLibrary();
        $header = @file_get_contents(self::headerPath());
        if ($header === false) {
            throw new \RuntimeException('datalogic FFI header not found: ' . self::headerPath());
        }
        $rewritten = preg_replace(
            '/^#define\s+FFI_LIB\s+"[^"]*"/m',
            '#define FFI_LIB "' . $library . '"',
            $header,
            count: $replaced,
        );
        if ($rewritten === null || $replaced !== 1) {
            throw new \RuntimeException('datalogic FFI header has no single FFI_LIB line to rewrite');
        }

        $tmp = sys_get_temp_dir() . '/datalogic-ffi-' . getmypid() . '-' . bin2hex(random_bytes(6)) . '.h';
        if (@file_put_contents($tmp, $rewritten) === false) {
            throw new \RuntimeException('cannot write temporary FFI header: ' . $tmp);
        }
        try {
            $ffi = FFI::load($tmp);
        } finally {
            @unlink($tmp);
        }
        if ($ffi === null) {
            throw new \RuntimeException('FFI::load failed for the datalogic header');
        }
        self::assertAbiVersion($ffi->datalogic_abi_version());
        return $ffi;
    }

    /**
     * Return the resolved cdylib path. Lookup order:
     *  1. `DATALOGIC_NATIVE_LIB` env var (absolute path)
     *  2. `lib/<os>-<arch>/libdatalogic_c.<ext>` relative to this package
     *     (populated by the release workflow)
     *  3. `../c/target/release/libdatalogic_c.<ext>` for in-tree dev
     *  4. The OS's default library search path (LD_LIBRARY_PATH, etc.)
     */
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

    /* --- v2 calling-convention helpers ------------------------------- */

    /** Fresh NULL-initialised `datalogic_error *` out-param slot. */
    public static function newErrorOut(): CData
    {
        return self::ffi()->new('datalogic_error*');
    }

    /**
     * Copy an owned `datalogic_buf` into a PHP string and release it.
     * `$buf` must be the struct CData a one-shot entry point just
     * filled; it is consumed (freed) here.
     */
    public static function takeBuf(CData $buf): string
    {
        $s = $buf->len > 0 ? FFI::string($buf->ptr, $buf->len) : '';
        self::ffi()->datalogic_buf_free($buf);
        return $s;
    }

    /**
     * Copy borrowed `(const uint8_t*, len)` bytes returned by an error
     * accessor or session evaluate into a PHP string; null for a NULL
     * pointer (absent field).
     */
    public static function copyBytes(?CData $ptr, int $len): ?string
    {
        if ($ptr === null || FFI::isNull($ptr)) {
            return null;
        }
        return $len > 0 ? FFI::string($ptr, $len) : '';
    }
}
