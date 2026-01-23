import type { JsonLogicValue } from '../../types';

// Format value for display in literal nodes (recursive for small arrays)
export function formatValue(value: JsonLogicValue): string {
  if (value === null) return 'null';
  if (typeof value === 'boolean') return value ? 'true' : 'false';
  if (typeof value === 'number') return String(value);
  if (typeof value === 'string') return `"${value}"`;
  if (Array.isArray(value)) {
    if (value.length <= 3) {
      return `[${value.map(formatValue).join(', ')}]`;
    }
    return `[${value.length} items]`;
  }
  return JSON.stringify(value);
}

// Format evaluation result value for compact display (with truncation)
export function formatResultValue(value: unknown): string {
  if (value === null) return 'null';
  if (value === undefined) return 'undefined';
  if (typeof value === 'boolean') return value ? 'true' : 'false';
  if (typeof value === 'number') return String(value);
  if (typeof value === 'string') {
    // Truncate long strings
    if (value.length > 15) {
      return `"${value.slice(0, 12)}..."`;
    }
    return `"${value}"`;
  }
  if (Array.isArray(value)) {
    if (value.length === 0) return '[]';
    return `[${value.length}]`;
  }
  if (typeof value === 'object') {
    const keys = Object.keys(value as object);
    if (keys.length === 0) return '{}';
    return `{${keys.length}}`;
  }
  return String(value);
}

// Check if a value is complex enough to show in a popover
export function isComplexValue(value: unknown): boolean {
  if (value === null || value === undefined) return false;
  if (typeof value === 'boolean' || typeof value === 'number') return false;
  if (typeof value === 'string') return value.length > 50;
  if (Array.isArray(value)) return value.length > 0;
  if (typeof value === 'object') return Object.keys(value as object).length > 0;
  return false;
}

// Get CSS class for color-coding values by type
export function getValueColorClass(value: unknown): string {
  if (value === null) return 'debug-value-null';
  if (value === undefined) return 'debug-value-undefined';
  if (typeof value === 'boolean') {
    return value ? 'debug-value-boolean-true' : 'debug-value-boolean-false';
  }
  if (typeof value === 'number') return 'debug-value-number';
  if (typeof value === 'string') return 'debug-value-string';
  if (Array.isArray(value)) return 'debug-value-array';
  if (typeof value === 'object') return 'debug-value-object';
  return '';
}

// Token types for JSON syntax highlighting
export type JsonTokenType = 'key' | 'string' | 'number' | 'boolean-true' | 'boolean-false' | 'null' | 'punctuation';

export interface JsonToken {
  type: JsonTokenType | null;
  content: string;
}

// Tokenize JSON string for syntax highlighting
export function tokenizeJson(value: unknown): JsonToken[] {
  const json = JSON.stringify(value, null, 2);
  if (!json) return [{ type: null, content: 'undefined' }];

  const tokens: JsonToken[] = [];
  let i = 0;

  while (i < json.length) {
    const char = json[i];

    // Whitespace
    if (/\s/.test(char)) {
      let whitespace = '';
      while (i < json.length && /\s/.test(json[i])) {
        whitespace += json[i];
        i++;
      }
      tokens.push({ type: null, content: whitespace });
      continue;
    }

    // Punctuation: { } [ ] : ,
    if (/[{}[\]:,]/.test(char)) {
      tokens.push({ type: 'punctuation', content: char });
      i++;
      continue;
    }

    // String (could be key or value)
    if (char === '"') {
      let str = '"';
      i++;
      while (i < json.length && json[i] !== '"') {
        if (json[i] === '\\' && i + 1 < json.length) {
          str += json[i] + json[i + 1];
          i += 2;
        } else {
          str += json[i];
          i++;
        }
      }
      str += '"';
      i++;

      // Check if this is a key (followed by :)
      let lookAhead = i;
      while (lookAhead < json.length && /\s/.test(json[lookAhead])) {
        lookAhead++;
      }
      const isKey = json[lookAhead] === ':';

      tokens.push({ type: isKey ? 'key' : 'string', content: str });
      continue;
    }

    // Number
    if (/[-\d]/.test(char)) {
      let num = '';
      while (i < json.length && /[-\d.eE+]/.test(json[i])) {
        num += json[i];
        i++;
      }
      tokens.push({ type: 'number', content: num });
      continue;
    }

    // true
    if (json.slice(i, i + 4) === 'true') {
      tokens.push({ type: 'boolean-true', content: 'true' });
      i += 4;
      continue;
    }

    // false
    if (json.slice(i, i + 5) === 'false') {
      tokens.push({ type: 'boolean-false', content: 'false' });
      i += 5;
      continue;
    }

    // null
    if (json.slice(i, i + 4) === 'null') {
      tokens.push({ type: 'null', content: 'null' });
      i += 4;
      continue;
    }

    // Unknown character
    tokens.push({ type: null, content: char });
    i++;
  }

  return tokens;
}
