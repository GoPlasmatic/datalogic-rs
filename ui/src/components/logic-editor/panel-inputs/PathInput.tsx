import { memo } from 'react';

interface PathInputProps {
  id: string;
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  disabled?: boolean;
  required?: boolean;
}

/**
 * Input for dot-notation paths (e.g., "user.profile.name")
 */
export const PathInput = memo(function PathInput({
  id,
  value,
  onChange,
  placeholder = 'path.to.value',
  disabled = false,
  required = false,
}: PathInputProps) {
  return (
    <div className="panel-input-path-wrapper">
      <span className="panel-input-path-prefix">$</span>
      <input
        id={id}
        type="text"
        className="panel-input panel-input-path"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        disabled={disabled}
        required={required}
        spellCheck={false}
        autoComplete="off"
      />
    </div>
  );
});
