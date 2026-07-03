// SPDX-License-Identifier: Apache-2.0
//
// Locates `datalogic_c` across local-dev and NuGet-packaged scenarios.
//
// Resolution order (first match wins):
//   1. DATALOGIC_NATIVE_LIB env var (absolute path) — escape hatch.
//   2. The standard NuGet `runtimes/<rid>/native/` layout (set up by
//      `dotnet publish` automatically when the NuGet package is consumed).
//   3. The C ABI's cargo target dir at the repo root — for in-tree dev
//      where neither runtimes/ nor a published NuGet exists.

using System.Reflection;
using System.Runtime.InteropServices;

namespace Goplasmatic.Datalogic.Native;

internal static class NativeLibraryResolver
{
    private static int _installed; // 0 = not yet, 1 = done

    /// <summary>
    /// Idempotent installer — invoked by <see cref="NativeInit"/>'s
    /// static constructor (which every public type touches) before any
    /// P/Invoke runs.
    /// </summary>
    internal static void Install()
    {
        if (Interlocked.CompareExchange(ref _installed, 1, 0) != 0) return;
        NativeLibrary.SetDllImportResolver(typeof(NativeMethods).Assembly, Resolve);
    }

    private static IntPtr Resolve(string libraryName, Assembly assembly, DllImportSearchPath? searchPath)
    {
        if (libraryName != NativeMethods.LibraryName) return IntPtr.Zero;

        var env = Environment.GetEnvironmentVariable("DATALOGIC_NATIVE_LIB");
        if (!string.IsNullOrEmpty(env) && NativeLibrary.TryLoad(env, out var handle))
        {
            return handle;
        }

        // Fall back to the default loader (which already covers
        // `runtimes/<rid>/native/`).
        if (NativeLibrary.TryLoad(libraryName, assembly, searchPath, out handle))
        {
            return handle;
        }

        // In-tree dev fallback: the C ABI's cargo target dir.
        foreach (var candidate in LocalDevCandidates(libraryName))
        {
            if (NativeLibrary.TryLoad(candidate, out handle)) return handle;
        }

        return IntPtr.Zero;
    }

    private static IEnumerable<string> LocalDevCandidates(string libraryName)
    {
        var assemblyDir = Path.GetDirectoryName(typeof(NativeMethods).Assembly.Location) ?? Environment.CurrentDirectory;

        // The assembly lives at e.g. bindings/dotnet/.../bin/<cfg>/<tfm>/
        // — walk up to bindings/dotnet, then over to bindings/c/target/release.
        var here = new DirectoryInfo(assemblyDir);
        for (int i = 0; i < 8 && here is not null; i++, here = here.Parent)
        {
            var sibling = Path.Combine(here.FullName, "..", "c", "target", "release");
            if (Directory.Exists(sibling))
            {
                var file = NativePlatform.LibraryFileName(libraryName);
                yield return Path.GetFullPath(Path.Combine(sibling, file));
            }
        }
    }
}

internal static class NativePlatform
{
    /// <summary>
    /// Returns the platform-conventional native library filename
    /// (`libdatalogic_c.so` / `libdatalogic_c.dylib` / `datalogic_c.dll`)
    /// for the given base name (`datalogic_c`).
    /// </summary>
    internal static string LibraryFileName(string baseName)
    {
        if (OperatingSystem.IsWindows()) return $"{baseName}.dll";
        if (OperatingSystem.IsMacOS()) return $"lib{baseName}.dylib";
        return $"lib{baseName}.so";
    }
}
