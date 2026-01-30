import React, { useCallback } from 'react';
import { highlightJsonText } from './json-highlight-utils';

interface JsonEditorProps {
  value: string;
  onChange: (e: React.ChangeEvent<HTMLTextAreaElement>) => void;
  hasError?: boolean;
  placeholder?: string;
  className?: string;
}

export function JsonEditor({ value, onChange, hasError, placeholder, className }: JsonEditorProps) {
  const textareaRef = React.useRef<HTMLTextAreaElement>(null);
  const highlightRef = React.useRef<HTMLPreElement>(null);

  // Sync scroll between textarea and highlight overlay
  const handleScroll = useCallback(() => {
    if (textareaRef.current && highlightRef.current) {
      highlightRef.current.scrollTop = textareaRef.current.scrollTop;
      highlightRef.current.scrollLeft = textareaRef.current.scrollLeft;
    }
  }, []);

  return (
    <div className={`json-editor ${className || ''} ${hasError ? 'has-error' : ''}`}>
      <pre
        ref={highlightRef}
        className="json-editor-highlight json-highlight"
        aria-hidden="true"
      >
        {value ? highlightJsonText(value) : <span className="json-placeholder">{placeholder}</span>}
      </pre>
      <textarea
        ref={textareaRef}
        className="json-editor-input"
        value={value}
        onChange={onChange}
        onScroll={handleScroll}
        spellCheck={false}
        placeholder=""
      />
    </div>
  );
}
