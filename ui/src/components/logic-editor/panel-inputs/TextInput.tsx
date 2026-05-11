import { memo } from 'react';

interface TextInputProps {
  id: string;
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  disabled?: boolean;
  required?: boolean;
}

export const TextInput = memo(function TextInput({
  id,
  value,
  onChange,
  placeholder,
  disabled = false,
  required = false,
}: TextInputProps) {
  return (
    <input
      id={id}
      type="text"
      className="panel-input panel-input-text"
      value={value}
      onChange={(e) => onChange(e.target.value)}
      placeholder={placeholder}
      disabled={disabled}
      required={required}
    />
  );
});
