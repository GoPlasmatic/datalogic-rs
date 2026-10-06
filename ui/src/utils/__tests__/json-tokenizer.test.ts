import { describe, expect, it } from 'vitest';
import { tokenizeJson, tokenizeValue } from '../json-tokenizer';

const kinds = (text: string) => tokenizeJson(text).filter((t) => t.type !== 'whitespace');

describe('tokenizeJson', () => {
  it('returns no tokens for empty input', () => {
    expect(tokenizeJson('')).toEqual([]);
  });

  it('tells keys from string values', () => {
    expect(kinds('{"a" : "b"}')).toEqual([
      { type: 'bracket', value: '{' },
      { type: 'key', value: '"a"' },
      { type: 'punctuation', value: ':' },
      { type: 'string', value: '"b"' },
      { type: 'bracket', value: '}' },
    ]);
  });

  it('keeps escaped quotes inside a string', () => {
    expect(kinds('["a\\"b"]')[1]).toEqual({ type: 'string', value: '"a\\"b"' });
  });

  it('reads numbers, booleans and null', () => {
    expect(kinds('[-1.5e3, true, false, null]').map((t) => t.type)).toEqual([
      'bracket', 'number', 'punctuation', 'boolean', 'punctuation', 'boolean', 'punctuation', 'null', 'bracket',
    ]);
  });

  it('keeps whitespace so the text round-trips', () => {
    const text = '{\n  "a": [1, 2]\n}';
    expect(tokenizeJson(text).map((t) => t.value).join('')).toBe(text);
  });

  it('marks characters outside JSON as unknown', () => {
    expect(kinds('[x]')[1]).toEqual({ type: 'unknown', value: 'x' });
  });

  it('closes an unterminated string at the end of the input', () => {
    expect(kinds('"abc')).toEqual([{ type: 'string', value: '"abc' }]);
  });
});

describe('tokenizeValue', () => {
  it('pretty-prints and tokenizes a value', () => {
    expect(tokenizeValue({ a: 1 }).map((t) => t.value).join('')).toBe('{\n  "a": 1\n}');
  });

  it('shows undefined (and other unserializable values) as undefined', () => {
    expect(tokenizeValue(undefined)).toEqual([{ type: 'null', value: 'undefined' }]);
    expect(tokenizeValue(() => 1)).toEqual([{ type: 'null', value: 'undefined' }]);
  });
});
