import React from 'react';
import { tokenizeJson, type JsonToken } from '../utils/json-tokenizer';

/**
 * Render tokenized JSON as React nodes with syntax highlighting
 */
function renderJsonTokens(tokens: JsonToken[]): React.ReactNode[] {
  return tokens.map((token, index) => {
    if (token.type === 'whitespace') {
      return token.value;
    }
    // Map token types to CSS class names
    const classMap: Record<string, string> = {
      key: 'json-key',
      string: 'json-string',
      number: 'json-number',
      boolean: 'json-boolean',
      null: 'json-null',
      bracket: 'json-punctuation',
      punctuation: 'json-punctuation',
      unknown: '',
    };
    const className = classMap[token.type] || '';
    if (!className) {
      return token.value;
    }
    return React.createElement('span', { key: index, className }, token.value);
  });
}

/**
 * Highlight JSON text with syntax coloring
 */
export function highlightJsonText(text: string): React.ReactNode[] {
  if (!text) return [];
  const tokens = tokenizeJson(text);
  return renderJsonTokens(tokens);
}
