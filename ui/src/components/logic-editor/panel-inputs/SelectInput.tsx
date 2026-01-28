import { memo } from 'react';
import type { SelectOption } from '../config/operators.types';

interface SelectInputProps {
  id: string;
  value: string | number | boolean;
  onChange: (value: string | number | boolean) => void;
  options: SelectOption[];
  disabled?: boolean;
  required?: boolean;
}

export const SelectInput = memo(function SelectInput({
  id,
  value,
  onChange,
  options,
  disabled = false,
  required = false,
}: SelectInputProps) {
  const handleChange = (e: React.ChangeEvent<HTMLSelectElement>) => {
    const selectedOption = options.find(
      (opt) => String(opt.value) === e.target.value
    );
    if (selectedOption) {
      onChange(selectedOption.value);
    }
  };

  return (
    <div className="panel-input-select-wrapper">
      <select
        id={id}
        className="panel-input panel-input-select"
        value={String(value)}
        onChange={handleChange}
        disabled={disabled}
        required={required}
      >
        {options.map((option) => (
          <option
            key={String(option.value)}
            value={String(option.value)}
            title={option.description}
          >
            {option.label}
          </option>
        ))}
      </select>
      <span className="panel-input-select-arrow">▼</span>
    </div>
  );
});
