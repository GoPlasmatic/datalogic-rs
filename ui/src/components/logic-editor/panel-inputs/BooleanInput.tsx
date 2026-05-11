import { memo } from 'react';

interface BooleanInputProps {
  id: string;
  value: boolean;
  onChange: (value: boolean) => void;
  disabled?: boolean;
}

export const BooleanInput = memo(function BooleanInput({
  id,
  value,
  onChange,
  disabled = false,
}: BooleanInputProps) {
  return (
    <label className="panel-input-boolean">
      <input
        id={id}
        type="checkbox"
        className="panel-input-checkbox"
        checked={value}
        onChange={(e) => onChange(e.target.checked)}
        disabled={disabled}
      />
      <span className="panel-input-toggle" />
    </label>
  );
});
