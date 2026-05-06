import { highlightJsonText } from './json-highlight-utils';

interface JsonHighlightProps {
  value: unknown;
  placeholder?: string;
}

export function JsonHighlight({ value, placeholder = '' }: JsonHighlightProps) {
  if (value === undefined) {
    return <pre className="json-highlight">{placeholder}</pre>;
  }

  const text = JSON.stringify(value, null, 2);
  return <pre className="json-highlight">{highlightJsonText(text)}</pre>;
}
