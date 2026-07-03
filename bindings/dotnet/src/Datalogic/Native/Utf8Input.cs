// SPDX-License-Identifier: Apache-2.0

using System.Buffers;
using System.Text;

namespace Goplasmatic.Datalogic.Native;

/// <summary>
/// Scoped UTF-8 encoding of a managed string for the C ABI's
/// (pointer, length) input contract: encodes into a caller-provided
/// stack buffer when it fits, otherwise into a pooled array returned on
/// Dispose. Never NUL-terminates — v2 inputs are raw byte ranges.
/// </summary>
/// <remarks>
/// Usage pattern (the stackalloc lives in the caller's frame so the
/// span stays valid for the ref struct's lifetime):
/// <code>
/// using var jsonU8 = Utf8Input.From(json, stackalloc byte[Utf8Input.StackBufferSize]);
/// fixed (byte* p = jsonU8.Span) { ... native call with (p, jsonU8.Span.Length) ... }
/// </code>
/// An empty string yields an empty span; `fixed` then produces a NULL
/// pointer with length 0, which the C ABI reads as the empty string.
/// </remarks>
internal ref struct Utf8Input
{
    /// <summary>Recommended stackalloc size for call sites.</summary>
    internal const int StackBufferSize = 512;

    private byte[]? _rented;
    private ReadOnlySpan<byte> _span;

    internal static Utf8Input From(string s, Span<byte> stackBuffer)
    {
        var input = new Utf8Input();
        if (Encoding.UTF8.GetMaxByteCount(s.Length) <= stackBuffer.Length)
        {
            var written = Encoding.UTF8.GetBytes(s, stackBuffer);
            input._span = stackBuffer[..written];
        }
        else
        {
            var count = Encoding.UTF8.GetByteCount(s);
            var rented = ArrayPool<byte>.Shared.Rent(count);
            var written = Encoding.UTF8.GetBytes(s, rented);
            input._rented = rented;
            input._span = rented.AsSpan(0, written);
        }
        return input;
    }

    /// <summary>The encoded UTF-8 bytes (no NUL terminator).</summary>
    internal readonly ReadOnlySpan<byte> Span => _span;

    public void Dispose()
    {
        if (_rented is not null)
        {
            ArrayPool<byte>.Shared.Return(_rented);
            _rented = null;
        }
        _span = default;
    }
}
