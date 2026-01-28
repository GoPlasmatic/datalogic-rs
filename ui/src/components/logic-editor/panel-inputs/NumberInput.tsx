import { memo } from 'react';

interface NumberInputProps {
  id: string;
  value: number | undefined;
  onChange: (value: number | undefined) => void;
  placeholder?: string;
  disabled?: boolean;
  required?: boolean;
  min?: number;
  max?: number;
  step?: number;
}

export const NumberInput = memo(function NumberInput({
  id,
  value,
  onChange,
  placeholder,
  disabled = false,
  required = false,
  min,
  max,
  step = 1,
}: NumberInputProps) {
  const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const rawValue = e.target.value;
    if (rawValue === '') {
      onChange(undefined);
    } else {
      const numValue = parseFloat(rawValue);
      if (!isNaN(numValue)) {
        onChange(numValue);
      }
    }
  };

  return (
    <input
      id={id}
      type="number"
      className="panel-input panel-input-number"
      value={value ?? ''}
      onChange={handleChange}
      placeholder={placeholder}
      disabled={disabled}
      required={required}
      min={min}
      max={max}
      step={step}
    />
  );
});
