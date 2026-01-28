import { memo } from 'react';

interface TextAreaInputProps {
  id: string;
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  disabled?: boolean;
  required?: boolean;
  rows?: number;
}

export const TextAreaInput = memo(function TextAreaInput({
  id,
  value,
  onChange,
  placeholder,
  disabled = false,
  required = false,
  rows = 3,
}: TextAreaInputProps) {
  return (
    <textarea
      id={id}
      className="panel-input panel-input-textarea"
      value={value}
      onChange={(e) => onChange(e.target.value)}
      placeholder={placeholder}
      disabled={disabled}
      required={required}
      rows={rows}
    />
  );
});
