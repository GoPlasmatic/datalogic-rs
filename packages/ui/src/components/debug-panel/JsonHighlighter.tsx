import { useMemo, useRef, useEffect, type ChangeEvent, type ReactElement } from 'react';
import { tokenizeJson, type JsonToken } from '../../utils/json-tokenizer';

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

// Render tokens as highlighted spans
function renderTokens(tokens: JsonToken[]): ReactElement[] {
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
