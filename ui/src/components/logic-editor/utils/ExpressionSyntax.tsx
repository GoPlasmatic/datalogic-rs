import { memo, useMemo, type ReactElement } from 'react';

type TokenType =
  | 'operator'      // AND, OR
  | 'keyword'       // if, then, else
  | 'comparison'    // ==, >=, <, etc.
  | 'arithmetic'    // +, -, *, /, %
  | 'unary'         // !, !!
  | 'function'      // map, filter, val, exists, etc.
  | 'string'        // "quoted strings"
  | 'number'        // numeric values
  | 'boolean'       // true, false
  | 'null'          // null
  | 'variable'      // variable paths like user.age
  | 'bracket'       // (), [], {}
  | 'punctuation'   // commas, colons
  | 'whitespace'
  | 'text';         // fallback

interface Token {
  type: TokenType;
  value: string;
}

// Keywords and operators to highlight
const COMPARISON_OPERATORS = ['===', '!==', '==', '!=', '>=', '<=', '>', '<'];
const ARITHMETIC_OPERATORS = ['+', '-', '*', '/', '%'];
const UNARY_OPERATORS = ['!!', '!'];
const FUNCTIONS = ['map', 'reduce', 'filter', 'some', 'none', 'all', 'val', 'exists', 'in', 'cat', 'substr', 'merge', 'missing', 'missing_some'];

// Tokenize expression text for syntax highlighting
function tokenizeExpression(text: string): Token[] {
  const tokens: Token[] = [];
  let i = 0;

  while (i < text.length) {
    const remaining = text.slice(i);

    // Whitespace
    const wsMatch = remaining.match(/^(\s+)/);
    if (wsMatch) {
      tokens.push({ type: 'whitespace', value: wsMatch[1] });
      i += wsMatch[1].length;
      continue;
    }

    // String literals
    if (remaining[0] === '"') {
      let str = '"';
      let j = 1;
      while (j < remaining.length && remaining[j] !== '"') {
        if (remaining[j] === '\\' && j + 1 < remaining.length) {
          str += remaining[j] + remaining[j + 1];
          j += 2;
        } else {
          str += remaining[j];
          j++;
        }
      }
      if (j < remaining.length) {
        str += '"';
        j++;
      }
      tokens.push({ type: 'string', value: str });
      i += j;
      continue;
    }

    // Logical operators (AND, OR) - must be whole words
    const logicalMatch = remaining.match(/^(AND|OR)(?![a-zA-Z0-9_])/);
    if (logicalMatch) {
      tokens.push({ type: 'operator', value: logicalMatch[1] });
      i += logicalMatch[1].length;
      continue;
    }

    // Keywords (if, then, else) - must be whole words
    const keywordMatch = remaining.match(/^(if|then|else)(?![a-zA-Z0-9_])/);
    if (keywordMatch) {
      tokens.push({ type: 'keyword', value: keywordMatch[1] });
      i += keywordMatch[1].length;
      continue;
    }

    // Boolean literals
    const boolMatch = remaining.match(/^(true|false)(?![a-zA-Z0-9_])/);
    if (boolMatch) {
      tokens.push({ type: 'boolean', value: boolMatch[1] });
      i += boolMatch[1].length;
      continue;
    }

    // Null literal
    const nullMatch = remaining.match(/^(null)(?![a-zA-Z0-9_])/);
    if (nullMatch) {
      tokens.push({ type: 'null', value: nullMatch[1] });
      i += nullMatch[1].length;
      continue;
    }

    // Function calls - word followed by (
    const funcMatch = remaining.match(/^([a-zA-Z_][a-zA-Z0-9_]*)(?=\()/);
    if (funcMatch && FUNCTIONS.includes(funcMatch[1])) {
      tokens.push({ type: 'function', value: funcMatch[1] });
      i += funcMatch[1].length;
      continue;
    }

    // Comparison operators (check longer ones first)
    let foundComparison = false;
    for (const op of COMPARISON_OPERATORS) {
      if (remaining.startsWith(op)) {
        tokens.push({ type: 'comparison', value: op });
        i += op.length;
        foundComparison = true;
        break;
      }
    }
    if (foundComparison) continue;

    // Unary operators (check longer ones first)
    let foundUnary = false;
    for (const op of UNARY_OPERATORS) {
      if (remaining.startsWith(op)) {
        tokens.push({ type: 'unary', value: op });
        i += op.length;
        foundUnary = true;
        break;
      }
    }
    if (foundUnary) continue;

    // Arithmetic operators
    if (ARITHMETIC_OPERATORS.includes(remaining[0])) {
      tokens.push({ type: 'arithmetic', value: remaining[0] });
      i++;
      continue;
    }

    // Brackets
    if ('()[]{}' .includes(remaining[0])) {
      tokens.push({ type: 'bracket', value: remaining[0] });
      i++;
      continue;
    }

    // Punctuation
    if (',;:'.includes(remaining[0])) {
      tokens.push({ type: 'punctuation', value: remaining[0] });
      i++;
      continue;
    }

    // Numbers (including negative and decimals)
    const numMatch = remaining.match(/^(-?\d+\.?\d*(?:[eE][+-]?\d+)?)/);
    if (numMatch) {
      tokens.push({ type: 'number', value: numMatch[1] });
      i += numMatch[1].length;
      continue;
    }

    // Variable paths (identifiers with dots) or plain identifiers
    const varMatch = remaining.match(/^([a-zA-Z_][a-zA-Z0-9_]*(?:\.[a-zA-Z_][a-zA-Z0-9_]*)*)/);
    if (varMatch) {
      tokens.push({ type: 'variable', value: varMatch[1] });
      i += varMatch[1].length;
      continue;
    }

    // Ellipsis (...)
    if (remaining.startsWith('...')) {
      tokens.push({ type: 'punctuation', value: '...' });
      i += 3;
      continue;
    }

    // Any other character
    tokens.push({ type: 'text', value: remaining[0] });
    i++;
  }

  return tokens;
}

// CSS class mapping for token types
const TOKEN_CLASSES: Record<TokenType, string> = {
  operator: 'expr-operator',
  keyword: 'expr-keyword',
  comparison: 'expr-comparison',
  arithmetic: 'expr-arithmetic',
  unary: 'expr-unary',
  function: 'expr-function',
  string: 'expr-string',
  number: 'expr-number',
  boolean: 'expr-boolean',
  null: 'expr-null',
  variable: 'expr-variable',
  bracket: 'expr-bracket',
  punctuation: 'expr-punctuation',
  whitespace: '',
  text: '',
};

// Render tokens as highlighted spans
function renderTokens(tokens: Token[]): ReactElement[] {
  return tokens.map((token, index) => {
    const className = TOKEN_CLASSES[token.type];
    if (!className) {
      return <span key={index}>{token.value}</span>;
    }
    return (
      <span key={index} className={className}>
        {token.value}
      </span>
    );
  });
}

interface ExpressionSyntaxProps {
  text: string;
}

/**
 * Expression syntax highlighting component for use in nodes
 */
export const ExpressionSyntax = memo(function ExpressionSyntax({ text }: ExpressionSyntaxProps) {
  const highlighted = useMemo(() => {
    if (!text) return null;
    const tokens = tokenizeExpression(text);
    return renderTokens(tokens);
  }, [text]);

  return <>{highlighted}</>;
});
