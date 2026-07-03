// SPDX-License-Identifier: Apache-2.0

using Goplasmatic.Datalogic.Native;

namespace Goplasmatic.Datalogic;

/// <summary>
/// An immutable, pre-parsed JSON document — parse the data once, then
/// evaluate any number of rules against it with zero parse work per
/// call (<see cref="Rule.Evaluate(DataHandle)"/>,
/// <see cref="Session.Evaluate(Rule, DataHandle)"/>, the typed
/// <c>Session.Evaluate*</c> variants, and the batch APIs).
/// </summary>
/// <remarks>
/// Thread-safe and engine-independent: one handle can be shared across
/// threads and fed to rules compiled by different engines. Dispose
/// releases the native arena — keep the handle alive until the last
/// evaluation that uses it (evaluations never consume it).
/// </remarks>
/// <example>
/// <code>
/// using var engine = new Engine();
/// using var rule = engine.Compile("""{"var":"x"}""");
/// using var data = DataHandle.Parse("""{"x":42}""");
/// var result = rule.Evaluate(data);  // "42"
/// </code>
/// </example>
public sealed class DataHandle : IDisposable
{
    private IntPtr _handle;

    static DataHandle() => NativeInit.EnsureLoaded();

    private DataHandle(IntPtr handle) { _handle = handle; }

    /// <summary>
    /// Parse a JSON document into a resident <see cref="DataHandle"/>.
    /// </summary>
    /// <exception cref="ParseException">The JSON is malformed.</exception>
    public static DataHandle Parse(string json)
    {
        ArgumentNullException.ThrowIfNull(json);
        unsafe
        {
            using var jsonU8 = Utf8Input.From(json, stackalloc byte[Utf8Input.StackBufferSize]);
            var err = IntPtr.Zero;
            DatalogicStatus status;
            IntPtr handle;
            fixed (byte* jp = jsonU8.Span)
            {
                status = NativeMethods.datalogic_data_parse(jp, (nuint)jsonU8.Span.Length, out handle, ref err);
            }
            if (status != DatalogicStatus.Ok)
            {
                throw DatalogicException.FromNative(status, err, "data parse failed");
            }
            return new DataHandle(handle);
        }
    }

    internal IntPtr Handle
    {
        get
        {
            if (_handle == IntPtr.Zero) throw new ObjectDisposedException(nameof(DataHandle));
            return _handle;
        }
    }

    /// <summary>
    /// Bytes held by the handle's backing arena (input copy + parsed
    /// tree). Useful for sizing or diagnostics.
    /// </summary>
    public nuint AllocatedBytes
    {
        get
        {
            var bytes = NativeMethods.datalogic_data_allocated_bytes(Handle);
            GC.KeepAlive(this);
            return bytes;
        }
    }

    /// <inheritdoc />
    public void Dispose()
    {
        if (_handle != IntPtr.Zero)
        {
            NativeMethods.datalogic_data_free(_handle);
            _handle = IntPtr.Zero;
        }
        GC.SuppressFinalize(this);
    }

    /// <summary>Finaliser falls back to <see cref="Dispose"/>.</summary>
    ~DataHandle() { Dispose(); }
}
