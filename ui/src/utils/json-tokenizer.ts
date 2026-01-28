/**
 * Unified JSON tokenizer for syntax highlighting
 *
 * This module provides a single implementation for tokenizing JSON strings,
 * used by JsonHighlighter, value-formatter, and embed components.
 */

export type JsonTokenType =
  | 'whitespace'
  | 'bracket'
  | 'punctuation'
  | 'key'
  | 'string'
  | 'number'
  | 'boolean'
  | 'null'
  | 'unknown';

export interface JsonToken {
  type: JsonTokenType;
  value: string;
}

/**
 * Tokenize a JSON string into typed tokens for syntax highlighting
 *
 * @param text - The JSON string to tokenize
 * @returns Array of tokens with type and value
 */
export function tokenizeJson(text: string): JsonToken[] {
  if (!text) return [];

  const tokens: JsonToken[] = [];
  let i = 0;

  while (i < text.length) {
    const char = text[i];

    // Whitespace
    if (/\s/.test(char)) {
      let ws = '';
      while (i < text.length && /\s/.test(text[i])) {
        ws += text[i];
        i++;
      }
      tokens.push({ type: 'whitespace', value: ws });
      continue;
    }

    // Brackets and braces
    if (char === '{' || char === '}' || char === '[' || char === ']') {
      tokens.push({ type: 'bracket', value: char });
      i++;
      continue;
    }

    // Punctuation (colon, comma)
    if (char === ':' || char === ',') {
      tokens.push({ type: 'punctuation', value: char });
      i++;
      continue;
    }

    // String
    if (char === '"') {
      let str = '"';
      i++;
      while (i < text.length && text[i] !== '"') {
        if (text[i] === '\\' && i + 1 < text.length) {
          str += text[i] + text[i + 1];
          i += 2;
        } else {
          str += text[i];
          i++;
        }
      }
      if (i < text.length) {
        str += '"';
        i++;
      }

      // Check if this is a key (followed by colon)
      let j = i;
      while (j < text.length && /\s/.test(text[j])) j++;
      const isKey = j < text.length && text[j] === ':';

      tokens.push({ type: isKey ? 'key' : 'string', value: str });
      continue;
    }

    // Number
    if (/[-\d]/.test(char)) {
      let num = '';
      while (i < text.length && /[-\d.eE+]/.test(text[i])) {
        num += text[i];
        i++;
      }
      tokens.push({ type: 'number', value: num });
      continue;
    }

    // Boolean or null
    if (text.slice(i, i + 4) === 'true') {
      tokens.push({ type: 'boolean', value: 'true' });
      i += 4;
      continue;
    }
    if (text.slice(i, i + 5) === 'false') {
      tokens.push({ type: 'boolean', value: 'false' });
      i += 5;
      continue;
    }
    if (text.slice(i, i + 4) === 'null') {
      tokens.push({ type: 'null', value: 'null' });
      i += 4;
      continue;
    }

    // Unknown character (for invalid JSON)
    tokens.push({ type: 'unknown', value: char });
    i++;
  }

  return tokens;
}

/**
 * Tokenize a JavaScript value by first converting to JSON string
 *
 * @param value - Any JavaScript value to tokenize
 * @returns Array of tokens with type and value
 */
export function tokenizeValue(value: unknown): JsonToken[] {
  if (value === undefined) {
    return [{ type: 'null', value: 'undefined' }];
  }
  const json = JSON.stringify(value, null, 2);
  if (!json) {
    return [{ type: 'null', value: 'undefined' }];
  }
  return tokenizeJson(json);
}
