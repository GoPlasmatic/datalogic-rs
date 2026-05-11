import { memo, useState, useCallback } from 'react';

interface JsonInputProps {
  id: string;
  value: unknown;
  onChange: (value: unknown) => void;
  disabled?: boolean;
  required?: boolean;
  rows?: number;
}

/**
 * Input for raw JSON values.
 * Provides validation feedback for invalid JSON.
 */
export const JsonInput = memo(function JsonInput({
  id,
  value,
  onChange,
  disabled = false,
  required = false,
  rows = 4,
}: JsonInputProps) {
  const [rawText, setRawText] = useState(() =>
    value !== undefined ? JSON.stringify(value, null, 2) : ''
  );
  const [error, setError] = useState<string | null>(null);

  const handleChange = useCallback(
    (e: React.ChangeEvent<HTMLTextAreaElement>) => {
      const text = e.target.value;
      setRawText(text);

      if (text.trim() === '') {
        setError(null);
        onChange(undefined);
        return;
      }

      try {
        const parsed = JSON.parse(text);
        setError(null);
        onChange(parsed);
      } catch {
        setError('Invalid JSON');
      }
    },
    [onChange]
  );

  return (
    <div className="panel-input-json-wrapper">
      <textarea
        id={id}
        className={`panel-input panel-input-json ${error ? 'panel-input-error' : ''}`}
        value={rawText}
        onChange={handleChange}
        disabled={disabled}
        required={required}
        rows={rows}
        spellCheck={false}
        autoComplete="off"
        placeholder='{"key": "value"}'
      />
      {error && <span className="panel-input-error-message">{error}</span>}
    </div>
  );
});
