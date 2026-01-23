import { useMemo, useRef, useEffect, type ChangeEvent, type ReactElement } from 'react';

interface JsonEditorProps {
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  hasError?: boolean;
}

interface JsonDisplayProps {
  value: unknown;
  className?: string;
}

// Tokenize JSON string for syntax highlighting
function tokenizeJson(text: string): Array<{ type: string; value: string }> {
  const tokens: Array<{ type: string; value: string }> = [];
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

// Render tokens as highlighted spans
function renderTokens(tokens: Array<{ type: string; value: string }>): ReactElement[] {
  return tokens.map((token, index) => {
    if (token.type === 'whitespace') {
      return <span key={index}>{token.value}</span>;
    }
    return (
      <span key={index} className={`json-${token.type}`}>
        {token.value}
      </span>
    );
  });
}

// JSON Editor with syntax highlighting
export function JsonEditor({ value, onChange, placeholder, hasError }: JsonEditorProps) {
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const wrapperRef = useRef<HTMLDivElement>(null);

  const highlighted = useMemo(() => {
    if (!value) return null;
    const tokens = tokenizeJson(value);
    return renderTokens(tokens);
  }, [value]);

  // Sync scroll between textarea and highlight layer
  const handleScroll = () => {
    if (textareaRef.current && wrapperRef.current) {
      const highlightLayer = wrapperRef.current.querySelector('.json-highlight-layer') as HTMLElement;
      if (highlightLayer) {
        highlightLayer.scrollTop = textareaRef.current.scrollTop;
        highlightLayer.scrollLeft = textareaRef.current.scrollLeft;
      }
    }
  };

  useEffect(() => {
    const textarea = textareaRef.current;
    if (textarea) {
      textarea.addEventListener('scroll', handleScroll);
      return () => textarea.removeEventListener('scroll', handleScroll);
    }
  }, []);

  const handleChange = (e: ChangeEvent<HTMLTextAreaElement>) => {
    onChange(e.target.value);
  };

  return (
    <div className={`json-editor-container ${hasError ? 'error' : ''}`}>
      <div className="json-editor-wrapper" ref={wrapperRef}>
        <div className="json-highlight-layer">
          {highlighted || <span className="json-placeholder">{placeholder}</span>}
        </div>
        <textarea
          ref={textareaRef}
          className="json-editor-textarea"
          value={value}
          onChange={handleChange}
          placeholder={placeholder}
          spellCheck={false}
        />
      </div>
    </div>
  );
}

// JSON Display (read-only) with syntax highlighting
export function JsonDisplay({ value, className = '' }: JsonDisplayProps) {
  const highlighted = useMemo(() => {
    const text = value === undefined ? 'undefined' : JSON.stringify(value, null, 2);
    const tokens = tokenizeJson(text);
    return renderTokens(tokens);
  }, [value]);

  // Determine result class based on value
  let resultClass = '';
  if (typeof value === 'boolean') {
    resultClass = value ? 'truthy' : 'falsy';
  }

  return (
    <div className={`json-result-display ${resultClass} ${className}`}>
      {highlighted}
    </div>
  );
}
