<?php

declare(strict_types=1);

/* SPDX-License-Identifier: Apache-2.0 */

namespace Goplasmatic\Datalogic\Tests;

use FFI;
use Goplasmatic\Datalogic\Internal\Native;
use PHPUnit\Framework\TestCase;

/**
 * Loader-level contract: the single header file must drive BOTH load
 * paths (FFI::cdef with the `#define`s stripped, FFI::load verbatim
 * with FFI_LIB rewritten), and both must land on ABI v2.
 */
final class NativeTest extends TestCase
{
    public function test_cdef_path_parses_and_reports_abi_v2(): void
    {
        // Independent of the Native::ffi() singleton: parse the stripped
        // header from scratch, exactly like the fallback path does.
        $ffi = FFI::cdef(Native::declarations(), Native::locateLibrary());
        self::assertSame(Native::ABI_VERSION, $ffi->datalogic_abi_version());
    }

    public function test_preload_header_loads_via_ffi_load_and_reports_abi_v2(): void
    {
        // FFI::load in-process: outside opcache.preload no persistent
        // scope is registered, but the returned instance exposes the
        // full surface — proving the header is FFI::load-clean.
        $ffi = Native::preload();
        self::assertSame(Native::ABI_VERSION, $ffi->datalogic_abi_version());

        // Drive a call through the preload-path instance end to end.
        $engine = $ffi->datalogic_engine_new(0);
        self::assertNotNull($engine);
        $rule = '{"+":[19,23]}';
        $data = '{}';
        $buf = $ffi->new('datalogic_buf');
        $rc = $ffi->datalogic_engine_apply(
            $engine,
            $rule,
            strlen($rule),
            $data,
            strlen($data),
            FFI::addr($buf),
            null,
        );
        self::assertSame(Native::STATUS_OK, $rc);
        self::assertSame('42', FFI::string($buf->ptr, $buf->len));
        $ffi->datalogic_buf_free($buf);
        $ffi->datalogic_engine_free($engine);
    }

    public function test_singleton_ffi_reports_abi_v2(): void
    {
        self::assertSame(Native::ABI_VERSION, Native::ffi()->datalogic_abi_version());
    }

    public function test_assert_abi_version_rejects_mismatch(): void
    {
        // A library reporting any other generation must be refused loudly.
        Native::assertAbiVersion(2); // current: no throw
        $this->expectException(\RuntimeException::class);
        $this->expectExceptionMessageMatches('/ABI version mismatch/');
        Native::assertAbiVersion(1);
    }

    public function test_header_declares_scope_and_lib_and_cdef_surface_is_preprocessor_free(): void
    {
        $header = file_get_contents(Native::headerPath());
        self::assertIsString($header);
        self::assertMatchesRegularExpression('/^#define\s+FFI_SCOPE\s+"datalogic"$/m', $header);
        self::assertSame(
            1,
            preg_match_all('/^#define\s+FFI_LIB\s+"[^"]*"$/m', $header),
            'exactly one FFI_LIB line (preload() rewrites it)',
        );

        $decls = Native::declarations();
        foreach (explode("\n", $decls) as $line) {
            self::assertFalse(
                str_starts_with(ltrim($line), '#'),
                'cdef surface must be preprocessor-free, found: ' . $line,
            );
        }
        // Spot-check the v2 surface made it through the strip.
        foreach ([
            'datalogic_abi_version',
            'datalogic_data_parse',
            'datalogic_session_evaluate_batch',
            'datalogic_session_evaluate_many',
            'datalogic_session_evaluate_truthy',
            'datalogic_op_result_set_json',
            'datalogic_error_path_json',
        ] as $symbol) {
            self::assertStringContainsString($symbol, $decls);
        }
    }
}
