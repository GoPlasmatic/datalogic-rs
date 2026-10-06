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
 *      there; we extract the matching one to a per-user cache directory
 *      and load it.
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
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.ArrayList;
import java.util.HexFormat;
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
        //    release JAR ships), extracted to a per-user cache directory.
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
     * Copy the classpath resource to a file the dynamic linker can open.
     * Returns {@code null} when the resource does not exist.
     *
     * <p>The library goes to a per-user cache directory named by its
     * SHA-256 ({@link #cachedCopy}), so every start of the same build
     * reuses one file. Extracting to a fresh temp directory each start
     * left one directory behind per run on Windows, where
     * {@code deleteOnExit} cannot remove a DLL the process still has
     * loaded. When the cache cannot be used, extraction falls back to a
     * fresh temp directory as before.
     */
    private static Path extractResource(String resource) throws IOException {
        ClassLoader cl = NativeLibrary.class.getClassLoader();
        byte[] bytes;
        try (InputStream in = cl != null
                ? cl.getResourceAsStream(resource)
                : ClassLoader.getSystemResourceAsStream(resource)) {
            if (in == null) {
                return null;
            }
            bytes = in.readAllBytes();
        }
        byte[] digest = sha256(bytes);
        try {
            Path cached = cachedCopy(bytes, digest);
            if (cached != null) {
                return cached;
            }
        } catch (IOException | RuntimeException e) {
            // Unwritable or unusable cache: fall through to a temp copy.
        }
        return tempCopy(bytes);
    }

    /**
     * The library at {@code <cache root>/<first 16 hex digits of its
     * SHA-256>/<file name>}, written there if no intact copy exists yet.
     * An existing file is used only when its own hash matches, so a
     * truncated or stale file is replaced, never loaded. The write goes
     * to a temp file in the same directory and is moved into place, so a
     * concurrent start never sees a partial library. Returns
     * {@code null} when there is no usable cache root.
     */
    private static Path cachedCopy(byte[] bytes, byte[] digest) throws IOException {
        Path root = cacheRoot();
        if (root == null) {
            return null;
        }
        Path dir = root.resolve(HexFormat.of().formatHex(digest, 0, 8));
        Path out = dir.resolve(fileName());
        if (isIntact(out, digest)) {
            return out;
        }
        Files.createDirectories(dir);
        Path tmp = Files.createTempFile(dir, "datalogic-", ".tmp");
        try {
            Files.write(tmp, bytes);
            try {
                Files.move(tmp, out, StandardCopyOption.REPLACE_EXISTING, StandardCopyOption.ATOMIC_MOVE);
            } catch (IOException e) {
                // Another process may have put an intact copy there first
                // and, on Windows, holds it open; that copy will do.
                if (!isIntact(out, digest)) {
                    throw e;
                }
            }
        } finally {
            Files.deleteIfExists(tmp);
        }
        return out;
    }

    /**
     * The per-user cache directory for extracted libraries, or
     * {@code null} when the platform's location cannot be determined.
     * Per-user, so no other account can plant a file there.
     */
    private static Path cacheRoot() {
        String home = System.getProperty("user.home");
        Path base;
        switch (osToken()) {
            case "win32" -> {
                String local = System.getenv("LOCALAPPDATA");
                if (local != null && !local.isBlank()) {
                    base = Path.of(local);
                } else if (home != null && !home.isBlank()) {
                    base = Path.of(home, "AppData", "Local");
                } else {
                    return null;
                }
            }
            case "darwin" -> {
                if (home == null || home.isBlank()) return null;
                base = Path.of(home, "Library", "Caches");
            }
            default -> {
                String xdg = System.getenv("XDG_CACHE_HOME");
                if (xdg != null && !xdg.isBlank() && Path.of(xdg).isAbsolute()) {
                    base = Path.of(xdg);
                } else if (home != null && !home.isBlank()) {
                    base = Path.of(home, ".cache");
                } else {
                    return null;
                }
            }
        }
        return base.resolve("datalogic").resolve("native");
    }

    /** Whether {@code file} exists and hashes to {@code digest}. */
    private static boolean isIntact(Path file, byte[] digest) {
        try {
            return Files.isRegularFile(file)
                    && MessageDigest.isEqual(sha256(Files.readAllBytes(file)), digest);
        } catch (IOException e) {
            return false;
        }
    }

    /** The previous behaviour: a fresh temp directory, deleted on exit where possible. */
    private static Path tempCopy(byte[] bytes) throws IOException {
        // Register the directory for exit-deletion before the file:
        // File.deleteOnExit runs LIFO, so the file goes first. Best
        // effort — Windows keeps loaded DLLs locked until exit.
        Path tempDir = Files.createTempDirectory("datalogic-native-");
        tempDir.toFile().deleteOnExit();
        Path out = tempDir.resolve(fileName());
        out.toFile().deleteOnExit();
        Files.write(out, bytes);
        return out;
    }

    private static byte[] sha256(byte[] bytes) {
        try {
            return MessageDigest.getInstance("SHA-256").digest(bytes);
        } catch (NoSuchAlgorithmException e) {
            // Every Java platform is required to provide SHA-256.
            throw new IllegalStateException(e);
        }
    }
}
