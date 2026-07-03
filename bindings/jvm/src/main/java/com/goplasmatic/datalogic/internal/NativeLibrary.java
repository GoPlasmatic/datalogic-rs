/*
 * SPDX-License-Identifier: Apache-2.0
 *
 * Locates and loads libdatalogic_c, returning a SymbolLookup for the
 * FFM layer. Mirrors the JNA-era resolution semantics 1:1 so the
 * release packaging (scripts/stage-jvm-natives.sh) needs no changes:
 *
 *   1. `datalogic.library.path` system property — a DIRECTORY holding
 *      the platform library (set by Maven surefire for in-tree tests;
 *      users can set it themselves to override).
 *   2. Classpath resource extraction from `<os-arch>/<libname>` at the
 *      classpath ROOT, where <os-arch> is the exact resource-prefix
 *      string JNA used (`darwin-aarch64`, `linux-x86-64`,
 *      `win32-x86-64`, ...). The release workflow stages the cdylibs
 *      there; we extract to a temp file and load it.
 *   3. `System.loadLibrary("datalogic_c")` — java.library.path and the
 *      OS's default loader paths.
 */

package com.goplasmatic.datalogic.internal;

import java.io.IOException;
import java.io.InputStream;
import java.lang.foreign.Arena;
import java.lang.foreign.SymbolLookup;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardCopyOption;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;

/** Resolves the native library; used once from {@link DatalogicNative}. */
final class NativeLibrary {

    private NativeLibrary() {}

    /** Base library name (no prefix/suffix), as passed to System.loadLibrary. */
    private static final String BASE_NAME = "datalogic_c";

    /**
     * Resolve and load libdatalogic_c, trying the three lookup tiers in
     * order. Throws {@link UnsatisfiedLinkError} listing every attempt
     * when none succeeds.
     */
    static SymbolLookup load() {
        List<String> attempts = new ArrayList<>();

        // 1. Explicit directory via -Ddatalogic.library.path=<dir>.
        String dir = System.getProperty("datalogic.library.path");
        if (dir != null && !dir.isBlank()) {
            Path candidate = Path.of(dir, fileName()).toAbsolutePath();
            if (Files.isRegularFile(candidate)) {
                return SymbolLookup.libraryLookup(candidate, Arena.global());
            }
            attempts.add("datalogic.library.path: " + candidate + " (no such file)");
        } else {
            attempts.add("datalogic.library.path system property not set");
        }

        // 2. Classpath-root resource `<os-arch>/<libname>` (the layout the
        //    release JAR ships), extracted to a temp file.
        String resource = resourcePrefix() + "/" + fileName();
        try {
            Path extracted = extractResource(resource);
            if (extracted != null) {
                return SymbolLookup.libraryLookup(extracted, Arena.global());
            }
            attempts.add("classpath resource " + resource + " (not on classpath)");
        } catch (IOException e) {
            attempts.add("classpath resource " + resource + " (extraction failed: " + e + ")");
        }

        // 3. java.library.path / OS default loader paths.
        try {
            System.loadLibrary(BASE_NAME);
            return SymbolLookup.loaderLookup();
        } catch (UnsatisfiedLinkError e) {
            attempts.add("System.loadLibrary(\"" + BASE_NAME + "\"): " + e.getMessage());
        }

        throw new UnsatisfiedLinkError(
                "Unable to locate the datalogic native library (" + fileName() + "). Tried:\n  - "
                        + String.join("\n  - ", attempts)
                        + "\nBuild it with `cargo build --release` in bindings/c/ and point "
                        + "-Ddatalogic.library.path at the directory containing it, or use the "
                        + "published JAR which bundles the library per platform.");
    }

    /**
     * Platform resource prefix — MUST stay byte-identical to JNA's
     * {@code Platform.RESOURCE_PREFIX} strings for the six platforms the
     * release workflow stages ({@code scripts/stage-jvm-natives.sh}):
     * darwin-aarch64, darwin-x86-64, linux-aarch64, linux-x86-64,
     * win32-aarch64, win32-x86-64.
     */
    static String resourcePrefix() {
        return osToken() + "-" + archToken();
    }

    /** Platform file name of the shared library. */
    static String fileName() {
        return switch (osToken()) {
            case "darwin" -> "lib" + BASE_NAME + ".dylib";
            case "win32" -> BASE_NAME + ".dll";
            default -> "lib" + BASE_NAME + ".so";
        };
    }

    private static String osToken() {
        String os = System.getProperty("os.name", "").toLowerCase(Locale.ROOT);
        if (os.contains("mac") || os.contains("darwin")) return "darwin";
        if (os.contains("win")) return "win32";
        return "linux";
    }

    private static String archToken() {
        String arch = System.getProperty("os.arch", "").toLowerCase(Locale.ROOT);
        return switch (arch) {
            case "aarch64", "arm64" -> "aarch64";
            case "x86_64", "amd64", "x86-64" -> "x86-64";
            default -> arch; // best effort; lookup simply won't find a resource
        };
    }

    /**
     * Copy the classpath resource to a temp file the dynamic linker can
     * open. Returns {@code null} when the resource does not exist.
     */
    private static Path extractResource(String resource) throws IOException {
        ClassLoader cl = NativeLibrary.class.getClassLoader();
        try (InputStream in = cl != null
                ? cl.getResourceAsStream(resource)
                : ClassLoader.getSystemResourceAsStream(resource)) {
            if (in == null) {
                return null;
            }
            // Register the directory for exit-deletion before the file:
            // File.deleteOnExit runs LIFO, so the file goes first. Best
            // effort — Windows keeps loaded DLLs locked until exit.
            Path tempDir = Files.createTempDirectory("datalogic-native-");
            tempDir.toFile().deleteOnExit();
            Path out = tempDir.resolve(fileName());
            out.toFile().deleteOnExit();
            Files.copy(in, out, StandardCopyOption.REPLACE_EXISTING);
            return out;
        }
    }
}
