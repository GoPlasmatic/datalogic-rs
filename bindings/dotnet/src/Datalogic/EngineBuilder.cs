// SPDX-License-Identifier: Apache-2.0

using System.Runtime.InteropServices;

using Goplasmatic.Datalogic.Native;

namespace Goplasmatic.Datalogic;

/// <summary>
/// Custom JSONLogic operator implemented in C#. The argument JSON is
/// the operator's pre-evaluated arguments as a JSON-array string
/// (e.g. <c>"[1, 2, \"x\"]"</c>); return the operator's result as a
/// JSON-value string (e.g. <c>"42"</c>, <c>"\"a\""</c>,
/// <c>"{\"k\":1}"</c>). Throw to signal an evaluation error — the
/// exception's message bubbles back to the caller.
/// </summary>
public delegate string CustomOperator(string argsJson);

/// <summary>
/// Builder for engines with custom operators. Mirrors the cross-binding
/// contract (registering a name that collides with a built-in like
/// <c>+</c> / <c>if</c> / <c>var</c> silently dispatches to the built-in
/// — built-ins always win).
/// </summary>
public sealed class EngineBuilder
{
    private IntPtr _handle;
    private bool _consumed;
    // Pin every registered callback so its function pointer stays valid
    // until the resulting Engine is disposed.
    private readonly List<GCHandle> _pinned = new();

    static EngineBuilder() => NativeLibraryResolver.Install();

    /// <summary>Construct a fresh, empty builder.</summary>
    public EngineBuilder()
    {
        _handle = NativeMethods.datalogic_engine_builder_new();
        if (_handle == IntPtr.Zero)
        {
            throw DatalogicException.FromLastError("builder_new failed");
        }
    }

    /// <summary>
    /// Toggle templating mode on the resulting engine.
    /// </summary>
    public EngineBuilder WithTemplating(bool enabled)
    {
        EnsureFresh();
        NativeMethods.datalogic_engine_builder_set_templating(_handle, enabled ? 1 : 0);
        return this;
    }

    /// <summary>
    /// Register a custom JSONLogic operator under <paramref name="name"/>.
    /// </summary>
    public EngineBuilder AddOperator(string name, CustomOperator op)
    {
        ArgumentException.ThrowIfNullOrEmpty(name);
        ArgumentNullException.ThrowIfNull(op);
        EnsureFresh();

        // Pin a GCHandle so the runtime won't move/free the closure
        // while the engine is alive. The pin is released when the
        // owning Engine is disposed.
        var handle = GCHandle.Alloc(op);
        _pinned.Add(handle);

        unsafe
        {
            delegate* unmanaged[Cdecl]<IntPtr, IntPtr, IntPtr*, IntPtr> trampoline = &Trampoline;
            var fnPtr = (IntPtr)trampoline;
            var rc = NativeMethods.datalogic_engine_builder_add_operator(
                _handle,
                name,
                fnPtr,
                GCHandle.ToIntPtr(handle));
            if (rc != 0)
            {
                throw DatalogicException.FromLastError("add_operator failed");
            }
        }
        return this;
    }

    /// <summary>
    /// Finalise the builder into an <see cref="Engine"/>. The builder is
    /// consumed; subsequent calls throw.
    /// </summary>
    public Engine Build()
    {
        EnsureFresh();
        var enginePtr = NativeMethods.datalogic_engine_builder_build(_handle);
        // Per the C ABI contract, the builder handle still needs freeing
        // after build() drains it.
        NativeMethods.datalogic_engine_builder_free(_handle);
        _handle = IntPtr.Zero;
        _consumed = true;

        if (enginePtr == IntPtr.Zero)
        {
            ReleasePins();
            throw DatalogicException.FromLastError("builder build failed");
        }
        var engine = new Engine(enginePtr);
        // Ownership of pin list transfers to the engine; the builder
        // keeps its `_pinned` list as the same reference but treats it
        // as immutable from here (builder is consumed, can't add more).
        engine.AdoptPinnedCallbacks(_pinned);
        return engine;
    }

    private void EnsureFresh()
    {
        if (_consumed) throw new InvalidOperationException("EngineBuilder has already been built.");
        if (_handle == IntPtr.Zero) throw new ObjectDisposedException(nameof(EngineBuilder));
    }

    private void ReleasePins()
    {
        foreach (var h in _pinned) h.Free();
        _pinned.Clear();
    }

    /// <summary>
    /// `extern "C"` trampoline invoked by the Rust engine for every
    /// custom-operator call. Resolves the <see cref="GCHandle"/> back to
    /// the C# delegate and forwards the call.
    /// </summary>
    [UnmanagedCallersOnly(CallConvs = new[] { typeof(System.Runtime.CompilerServices.CallConvCdecl) })]
    private static unsafe IntPtr Trampoline(IntPtr argsJsonPtr, IntPtr userData, IntPtr* errorOut)
    {
        try
        {
            var handle = GCHandle.FromIntPtr(userData);
            var op = (CustomOperator?)handle.Target;
            if (op is null)
            {
                WriteError(errorOut, "internal: operator handle had wrong type");
                return IntPtr.Zero;
            }
            var argsJson = Marshal.PtrToStringUTF8(argsJsonPtr) ?? "[]";
            var result = op(argsJson);
            // Allocate with libc malloc so the Rust side's `free()` works
            // (`Marshal.StringToCoTaskMemUTF8` uses CoTaskMemAlloc on
            // Windows — not safe for `free()`). Use libc strdup
            // equivalent via Marshal.AllocHGlobal which on every platform
            // .NET supports calls into the C runtime's allocator.
            return AllocLibcUtf8(result);
        }
        catch (Exception ex)
        {
            WriteError(errorOut, ex.Message);
            return IntPtr.Zero;
        }
    }

    private static unsafe void WriteError(IntPtr* errorOut, string message)
    {
        if (errorOut != null && *errorOut == IntPtr.Zero)
        {
            *errorOut = AllocLibcUtf8(message);
        }
    }

    private static unsafe IntPtr AllocLibcUtf8(string s)
    {
        // Use NativeMemory.Alloc — backed by the C runtime's `malloc`
        // on every supported platform (.NET 6+). Critical for Windows:
        // Marshal.AllocHGlobal uses LocalAlloc, which is NOT freeable by
        // libc's free(). The Rust side calls libc `free()` on this
        // pointer, so we have to allocate with the matching allocator.
        var bytes = System.Text.Encoding.UTF8.GetByteCount(s);
        var ptr = NativeMemory.Alloc((nuint)(bytes + 1));
        var span = new Span<byte>(ptr, bytes + 1);
        System.Text.Encoding.UTF8.GetBytes(s, span);
        span[bytes] = 0;
        return (IntPtr)ptr;
    }

}
