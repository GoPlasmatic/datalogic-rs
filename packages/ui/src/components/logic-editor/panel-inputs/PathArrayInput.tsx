import { memo, useCallback } from 'react';
import { Plus, X } from 'lucide-react';

interface PathArrayInputProps {
  id: string;
  value: string[];
  onChange: (value: string[]) => void;
  disabled?: boolean;
  required?: boolean;
}

/**
 * Input for array-style paths with editable segments.
 * Renders as a list of path components that can be added/removed.
 */
export const PathArrayInput = memo(function PathArrayInput({
  id,
  value,
  onChange,
  disabled = false,
}: PathArrayInputProps) {
  const handleSegmentChange = useCallback(
    (index: number, newValue: string) => {
      const updated = [...value];
      updated[index] = newValue;
      onChange(updated);
    },
    [value, onChange]
  );

  const handleAddSegment = useCallback(() => {
    onChange([...value, '']);
  }, [value, onChange]);

  const handleRemoveSegment = useCallback(
    (index: number) => {
      const updated = value.filter((_, i) => i !== index);
      onChange(updated);
    },
    [value, onChange]
  );

  return (
    <div id={id} className="panel-input-path-array">
      {value.map((segment, index) => (
        <div key={index} className="panel-input-path-segment">
          <input
            type="text"
            className="panel-input panel-input-segment"
            value={segment}
            onChange={(e) => handleSegmentChange(index, e.target.value)}
            placeholder={`segment ${index + 1}`}
            disabled={disabled}
            spellCheck={false}
            autoComplete="off"
          />
          <button
            type="button"
            className="panel-input-segment-remove"
            onClick={() => handleRemoveSegment(index)}
            disabled={disabled}
            title="Remove segment"
          >
            <X size={14} />
          </button>
        </div>
      ))}
      <button
        type="button"
        className="panel-input-add-segment"
        onClick={handleAddSegment}
        disabled={disabled}
      >
        <Plus size={14} />
        <span>Add segment</span>
      </button>
      {value.length > 0 && (
        <div className="panel-input-path-preview">
          Preview: [{value.map((s) => `"${s}"`).join(', ')}]
        </div>
      )}
    </div>
  );
});
