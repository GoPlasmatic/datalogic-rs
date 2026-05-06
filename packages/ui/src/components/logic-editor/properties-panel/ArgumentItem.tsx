import { memo, useCallback, useState, useEffect, useRef } from 'react';
import { Link2Off, ExternalLink } from 'lucide-react';
import type { LiteralNodeData, JsonLogicValue } from '../types';
import { formatNodeValue, type ArgumentInfo } from './utils/argument-parser';
import { formatOperandLabel } from '../utils/formatting';

interface ArgumentItemProps {
  arg: ArgumentInfo;
  isVariableArity: boolean;
  canRemoveArg: boolean;
  onSelect: (childId: string) => void;
  onRemove: (argIndex: number) => void;
  onLiteralChange: (childId: string, value: JsonLogicValue, valueType: LiteralNodeData['valueType']) => void;
  onInlineLiteralChange: (argIndex: number, value: JsonLogicValue) => void;
}

export const ArgumentItem = memo(function ArgumentItem({
  arg,
  isVariableArity,
  canRemoveArg,
  onSelect,
  onRemove,
  onLiteralChange,
  onInlineLiteralChange,
}: ArgumentItemProps) {
  const { index, isInline, value, valueType, childNode, childId, rowLabel, fieldId, placeholder } = arg;
  const indexLabel = rowLabel || String(index + 1);

  // For linked child nodes that are literals
  const isChildLiteral = childNode?.data.type === 'literal';
  const childLiteralData = isChildLiteral ? (childNode.data as LiteralNodeData) : null;

  // Local state for editing - prevents focus loss by not updating parent on every keystroke
  const [localValue, setLocalValue] = useState<string>(() => {
    if (isInline) {
      return value !== null && value !== undefined ? String(value) : '';
    }
    if (childLiteralData) {
      return childLiteralData.value !== null && childLiteralData.value !== undefined
        ? String(childLiteralData.value)
        : '';
    }
    return '';
  });

  // Track if we're currently editing (to prevent external value updates from overwriting)
  const isEditingRef = useRef(false);

  // Sync local value with external value when not editing
  /* eslint-disable react-hooks/set-state-in-effect -- Syncing local editing state with external prop changes */
  useEffect(() => {
    if (!isEditingRef.current) {
      if (isInline) {
        setLocalValue(value !== null && value !== undefined ? String(value) : '');
      } else if (childLiteralData) {
        setLocalValue(
          childLiteralData.value !== null && childLiteralData.value !== undefined
            ? String(childLiteralData.value)
            : ''
        );
      }
    }
  }, [isInline, value, childLiteralData]);
  /* eslint-enable react-hooks/set-state-in-effect */

  // Handlers for inline literal edits - update local state immediately, commit on blur
  const handleInlineNumberChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      isEditingRef.current = true;
      setLocalValue(e.target.value);
    },
    []
  );

  const handleInlineNumberBlur = useCallback(() => {
    isEditingRef.current = false;
    const num = parseFloat(localValue);
    onInlineLiteralChange(index, isNaN(num) ? 0 : num);
  }, [index, localValue, onInlineLiteralChange]);

  const handleInlineStringChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      isEditingRef.current = true;
      setLocalValue(e.target.value);
    },
    []
  );

  const handleInlineStringBlur = useCallback(() => {
    isEditingRef.current = false;
    onInlineLiteralChange(index, localValue);
  }, [index, localValue, onInlineLiteralChange]);

  const handleInlineBooleanChange = useCallback(
    (e: React.ChangeEvent<HTMLSelectElement>) => {
      // Booleans commit immediately since they're select elements
      onInlineLiteralChange(index, e.target.value === 'true');
    },
    [index, onInlineLiteralChange]
  );

  // Handlers for child node literal edits - update local state immediately, commit on blur
  const handleChildNumberChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      isEditingRef.current = true;
      setLocalValue(e.target.value);
    },
    []
  );

  const handleChildNumberBlur = useCallback(() => {
    if (!childId) return;
    isEditingRef.current = false;
    const num = parseFloat(localValue);
    onLiteralChange(childId, isNaN(num) ? 0 : num, 'number');
  }, [childId, localValue, onLiteralChange]);

  const handleChildStringChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      isEditingRef.current = true;
      setLocalValue(e.target.value);
    },
    []
  );

  const handleChildStringBlur = useCallback(() => {
    if (!childId) return;
    isEditingRef.current = false;
    onLiteralChange(childId, localValue, 'string');
  }, [childId, localValue, onLiteralChange]);

  const handleChildBooleanChange = useCallback(
    (e: React.ChangeEvent<HTMLSelectElement>) => {
      if (!childId) return;
      // Booleans commit immediately since they're select elements
      onLiteralChange(childId, e.target.value === 'true', 'boolean');
    },
    [childId, onLiteralChange]
  );

  // Render inline literal (value stored in parent's expression)
  if (isInline) {
    return (
      <div className="argument-item">
        <div className="argument-index">{indexLabel}</div>
        <div className="argument-literal-input">
          {valueType === 'number' && (
            <input
              type="number"
              className="argument-input argument-input--number"
              value={localValue}
              onChange={handleInlineNumberChange}
              onBlur={handleInlineNumberBlur}
              step={fieldId === 'scopeLevel' ? '1' : 'any'}
              min={fieldId === 'scopeLevel' ? '0' : undefined}
            />
          )}
          {valueType === 'string' && (
            <input
              type="text"
              className="argument-input argument-input--string"
              value={localValue}
              onChange={handleInlineStringChange}
              onBlur={handleInlineStringBlur}
              placeholder={placeholder || '(empty string)'}
            />
          )}
          {valueType === 'boolean' && (
            <select
              className="argument-input argument-input--boolean"
              value={value ? 'true' : 'false'}
              onChange={handleInlineBooleanChange}
            >
              <option value="true">true</option>
              <option value="false">false</option>
            </select>
          )}
          {valueType === 'null' && (
            <span className="argument-input argument-input--readonly">null</span>
          )}
          {valueType === 'array' && (
            <span className="argument-input argument-input--readonly">
              {formatOperandLabel(value ?? null)}
            </span>
          )}
        </div>
        {isVariableArity && canRemoveArg && (
          <RemoveButton index={index} onRemove={onRemove} />
        )}
      </div>
    );
  }

  // Render child node literal (has its own node that can be updated)
  if (isChildLiteral && childLiteralData && childId) {
    return (
      <div className="argument-item">
        <div className="argument-index">{indexLabel}</div>
        <div className="argument-literal-input">
          {childLiteralData.valueType === 'number' && (
            <input
              type="number"
              className="argument-input argument-input--number"
              value={localValue}
              onChange={handleChildNumberChange}
              onBlur={handleChildNumberBlur}
              step="any"
            />
          )}
          {childLiteralData.valueType === 'string' && (
            <input
              type="text"
              className="argument-input argument-input--string"
              value={localValue}
              onChange={handleChildStringChange}
              onBlur={handleChildStringBlur}
              placeholder="(empty string)"
            />
          )}
          {childLiteralData.valueType === 'boolean' && (
            <select
              className="argument-input argument-input--boolean"
              value={childLiteralData.value ? 'true' : 'false'}
              onChange={handleChildBooleanChange}
            >
              <option value="true">true</option>
              <option value="false">false</option>
            </select>
          )}
          {childLiteralData.valueType === 'null' && (
            <span className="argument-input argument-input--readonly">null</span>
          )}
          {childLiteralData.valueType === 'array' && (
            <button
              type="button"
              className="argument-value argument-value--complex"
              onClick={() => onSelect(childId)}
              title="Click to edit this array"
            >
              [{(childLiteralData.value as unknown[])?.length ?? 0} items]
              <ExternalLink size={12} />
            </button>
          )}
        </div>
        {isVariableArity && canRemoveArg && (
          <RemoveButton index={childNode?.data.argIndex ?? index} onRemove={onRemove} />
        )}
      </div>
    );
  }

  // Render complex expression (link to child node)
  return (
    <div className="argument-item">
      <div className="argument-index">{index + 1}</div>
      {childNode && childId ? (
        <button
          type="button"
          className="argument-value argument-value--complex"
          onClick={() => onSelect(childId)}
          title="Click to edit this expression"
        >
          {formatNodeValue(childNode)}
          <ExternalLink size={12} />
        </button>
      ) : (
        <span className="argument-input argument-input--readonly">
          (unknown)
        </span>
      )}
      {isVariableArity && canRemoveArg && (
        <RemoveButton index={childNode?.data.argIndex ?? index} onRemove={onRemove} />
      )}
    </div>
  );
});

function RemoveButton({ index, onRemove }: { index: number; onRemove: (i: number) => void }) {
  return (
    <button
      type="button"
      className="argument-remove"
      onClick={() => onRemove(index)}
      title="Remove this argument"
    >
      <Link2Off size={14} />
    </button>
  );
}
