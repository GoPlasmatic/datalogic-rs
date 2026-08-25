import { describe, expect, it } from 'vitest';
import { encodeShareableState, decodeShareableState } from '../src/utils/url-share';

describe('url-share round trip', () => {
  const logic = { if: [{ '>': [{ var: 'a' }, 1] }, 'big', 'small'] };
  const data = { a: 2, nested: { list: [1, 2, 3] } };

  it('round-trips logic and data without templating or config', () => {
    const encoded = encodeShareableState(logic, data);
    expect(encoded).toMatch(/^[A-Za-z0-9_-]+$/);
    const decoded = decodeShareableState(encoded);
    expect(decoded).toEqual({ logic, data });
    expect(decoded?.templating).toBeUndefined();
    expect(decoded?.config).toBeUndefined();
  });

  it('keeps the positional templating flag working', () => {
    const decoded = decodeShareableState(encodeShareableState(logic, data, true));
    expect(decoded?.templating).toBe(true);
  });

  it('round-trips templating and engine config through the options object', () => {
    const config = {
      preset: 'strict' as const,
      division_by_zero: 'return_null' as const,
      numeric_coercion: { null_to_zero: false },
      max_recursion_depth: 64,
    };
    const encoded = encodeShareableState(logic, data, { templating: true, config });
    const decoded = decodeShareableState(encoded);
    expect(decoded).toEqual({ logic, data, templating: true, config });
  });

  it('omits an empty config object', () => {
    const decoded = decodeShareableState(encodeShareableState(logic, data, { config: {} }));
    expect(decoded?.config).toBeUndefined();
  });

  it('accepts non-object data roots (arrays and scalars)', () => {
    expect(decodeShareableState(encodeShareableState({ var: '1' }, [10, 20]))?.data).toEqual([10, 20]);
    expect(decodeShareableState(encodeShareableState({ var: '' }, 'scalar'))?.data).toBe('scalar');
  });

  it('encodes payloads whose compressed size exceeds the fromCharCode spread limit', () => {
    // Pseudo-random bytes (xorshift32) do not deflate, so the compressed
    // buffer stays large, well past the ~120 KB spread limit that used to
    // throw RangeError.
    let seed = 0x9e3779b9;
    const noise = Array.from({ length: 200_000 }, () => {
      seed ^= seed << 13;
      seed ^= seed >>> 17;
      seed ^= seed << 5;
      seed >>>= 0;
      return seed & 0xff;
    });
    const bigData = { noise };
    const encoded = encodeShareableState(logic, bigData);
    expect(encoded.length).toBeGreaterThan(100_000);
    expect(decodeShareableState(encoded)).toEqual({ logic, data: bigData });
  });

  it('returns null for garbage input', () => {
    expect(decodeShareableState('not base64!!')).toBeNull();
    expect(decodeShareableState('')).toBeNull();
  });
});
