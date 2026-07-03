// SPDX-License-Identifier: Apache-2.0

namespace Goplasmatic.Datalogic.Native;

/// <summary>
/// One-time native-library initialisation: installs the
/// <see cref="NativeLibraryResolver"/> and asserts that the resolved
/// library speaks C ABI v2 (`datalogic_abi_version() == 2`). Every
/// public entry-point type touches this from its static constructor, so
/// a stale native library fails loudly on first use instead of
/// corrupting memory at call time.
/// </summary>
internal static class NativeInit
{
    static NativeInit()
    {
        NativeLibraryResolver.Install();
        var actual = NativeMethods.datalogic_abi_version();
        if (actual != NativeMethods.AbiVersion)
        {
            throw new InvalidOperationException(
                $"The loaded '{NativeMethods.LibraryName}' native library reports datalogic C ABI version {actual}, "
                + $"but this version of Goplasmatic.Datalogic requires ABI version {NativeMethods.AbiVersion}. "
                + "A stale native library is being resolved. Fix: rebuild it in-tree with `cargo build --release` "
                + "under bindings/c/, point the DATALOGIC_NATIVE_LIB environment variable at a matching build, "
                + "or upgrade/reinstall the NuGet package so the managed and native halves match.");
        }
    }

    /// <summary>
    /// Idempotent trigger — touching this method runs the static
    /// constructor above exactly once per process. On ABI mismatch the
    /// first caller gets the <see cref="InvalidOperationException"/>
    /// (wrapped in a <see cref="TypeInitializationException"/>) and
    /// every later caller fails the same way.
    /// </summary>
    internal static void EnsureLoaded()
    {
        // Intentionally empty: the work happens in the static ctor.
    }
}
