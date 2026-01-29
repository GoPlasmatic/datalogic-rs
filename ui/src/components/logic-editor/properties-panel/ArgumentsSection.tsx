/**
 * Arguments Section Component
 *
 * Displays and manages arguments for N-ary operator nodes.
 * Allows adding/removing arguments for operators that support variable arity.
 * Literal arguments are rendered as inline editable fields.
 *
 * Note: The converter inlines simple literals into the parent node's expression
 * data (they don't become separate child nodes). This component extracts
 * arguments from the expression and displays them appropriately.
 */

import { memo, useMemo, useCallback, useState, useEffect, useRef } from 'react';
import { Plus, Link2Off, ExternalLink } from 'lucide-react';
import { useEditorContext } from '../context/editor';
import type { LogicNode, OperatorNodeData, VerticalCellNodeData, LiteralNodeData, JsonLogicValue } from '../types';
import { getOperator } from '../config/operators';
import {
  supportsVariableArgs,
  hasArguments,
  getOperatorName,
  formatNodeValue,
  formatRawValue,
  extractOperatorArguments,
  extractVerticalCellArguments,
  type ArgumentInfo,
} from './utils/argument-parser';

/**
 * Format a value as an inline label for verticalCell cells
 */
function formatInlineLabel(value: JsonLogicValue): string {
  if (value === null) return 'null';
  if (typeof value === 'string') return `"${value}"`;
  if (typeof value === 'boolean') return value ? 'true' : 'false';
  if (Array.isArray(value)) return `[${value.length} items]`;
  if (typeof value === 'object') return '{...}';
  return String(value);
}

interface ArgumentsSectionProps {
  node: LogicNode;
}

export const ArgumentsSection = memo(function ArgumentsSection({
  node,
}: ArgumentsSectionProps) {
  const {
    getChildNodes,
    addArgumentToNode,
    removeArgumentFromNode,
    selectNode,
    updateNode,
  } = useEditorContext();

  const operatorName = getOperatorName(node.data);
  const opConfig = operatorName ? getOperator(operatorName) : undefined;

  // Get child nodes (these are only the complex expressions, not inlined literals)
  const childNodes = useMemo(() => {
    return getChildNodes(node.id);
  }, [getChildNodes, node.id]);

  // Build a map of argIndex -> childNode for correct matching (for operator nodes)
  // Child nodes have an argIndex that corresponds to their position in the expression
  const childNodeByArgIndex = useMemo(() => {
    const map = new Map<number, LogicNode>();
    childNodes.forEach((child) => {
      const argIndex = child.data.argIndex;
      if (argIndex !== undefined) {
        map.set(argIndex, child);
      }
    });
    return map;
  }, [childNodes]);

  // Build a map of childId -> node for verticalCell nodes (they use branchId references)
  const childNodeMap = useMemo(() => {
    const map = new Map<string, LogicNode>();
    childNodes.forEach((child) => {
      map.set(child.id, child);
    });
    return map;
  }, [childNodes]);

  // Extract arguments from the node's expression data
  // This handles both inlined literals and linked child nodes
  const arguments_ = useMemo((): ArgumentInfo[] => {
    const nodeData = node.data;

    if (nodeData.type === 'operator') {
      return extractOperatorArguments(nodeData as OperatorNodeData, childNodeByArgIndex);
    }

    if (nodeData.type === 'verticalCell') {
      return extractVerticalCellArguments(nodeData as VerticalCellNodeData, childNodeMap);
    }

    return [];
  }, [node.data, childNodeByArgIndex, childNodeMap]);

  // Check if we can add/remove arguments
  const canAddArg = useMemo(() => {
    if (!supportsVariableArgs(opConfig)) return false;
    const max = opConfig?.arity.max;
    return max === undefined || arguments_.length < max;
  }, [opConfig, arguments_.length]);

  const canRemoveArg = useMemo(() => {
    if (!supportsVariableArgs(opConfig)) return false;
    const min = opConfig?.arity.min ?? 0;
    return arguments_.length > min;
  }, [opConfig, arguments_.length]);

  // Check if this is a variable-arity operator (can add/remove args)
  const isVariableArity = supportsVariableArgs(opConfig);

  const handleAddArgument = useCallback(() => {
    // Default to adding a literal node
    addArgumentToNode(node.id, 'literal');
  }, [addArgumentToNode, node.id]);

  const handleRemoveArgument = useCallback((argIndex: number) => {
    removeArgumentFromNode(node.id, argIndex);
  }, [removeArgumentFromNode, node.id]);

  const handleSelectChild = useCallback((childId: string) => {
    selectNode(childId);
  }, [selectNode]);

  // Handle updating a literal argument value (for child node literals)
  const handleLiteralChange = useCallback(
    (childId: string, newValue: JsonLogicValue, valueType: LiteralNodeData['valueType']) => {
      updateNode(childId, {
        value: newValue,
        valueType,
        expression: newValue,
      });
    },
    [updateNode]
  );

  // Handle updating an inline literal value (stored in parent's expression)
  const handleInlineLiteralChange = useCallback(
    (argIndex: number, newValue: JsonLogicValue) => {
      const nodeData = node.data;

      if (nodeData.type === 'operator') {
        const opData = nodeData as OperatorNodeData;
        const expr = opData.expression;

        if (expr && typeof expr === 'object' && !Array.isArray(expr)) {
          const operator = Object.keys(expr)[0];
          const operands = (expr as Record<string, unknown>)[operator];
          const operandArray: JsonLogicValue[] = Array.isArray(operands)
            ? [...operands]
            : [operands as JsonLogicValue];

          // Update the operand at the given index
          operandArray[argIndex] = newValue;

          // Rebuild the expression
          const newExpression = { [operator]: operandArray };

          updateNode(node.id, {
            expression: newExpression,
            expressionText: undefined, // Will be regenerated
          });
        }
      }

      if (nodeData.type === 'verticalCell') {
        const vcData = nodeData as VerticalCellNodeData;
        const expr = vcData.expression;

        if (expr && typeof expr === 'object' && !Array.isArray(expr)) {
          const operator = Object.keys(expr)[0];
          const operands = (expr as Record<string, unknown>)[operator];
          const operandArray: JsonLogicValue[] = Array.isArray(operands)
            ? [...operands]
            : [operands as JsonLogicValue];

          // Update the operand at the given index
          operandArray[argIndex] = newValue;

          // Update the cell's label to reflect the new value
          const newCells = vcData.cells.map((cell) => {
            if (cell.index === argIndex && cell.type === 'inline') {
              return {
                ...cell,
                label: formatInlineLabel(newValue),
              };
            }
            return cell;
          });

          // Rebuild the expression
          const newExpression = { [operator]: operandArray };

          updateNode(node.id, {
            cells: newCells,
            expression: newExpression,
            expressionText: undefined, // Will be regenerated
          });
        }
      }
    },
    [node.id, node.data, updateNode]
  );

  // Don't render if operator has no arguments (nullary)
  if (!hasArguments(opConfig)) {
    return null;
  }

  return (
    <div className="properties-panel-section">
      <div className="properties-panel-section-header">
        <span>Arguments ({arguments_.length})</span>
      </div>

      <div className="arguments-list">
        {arguments_.map((arg) => (
          <ArgumentItem
            key={`arg-${arg.index}`}
            arg={arg}
            isVariableArity={isVariableArity}
            canRemoveArg={canRemoveArg}
            onSelect={handleSelectChild}
            onRemove={handleRemoveArgument}
            onLiteralChange={handleLiteralChange}
            onInlineLiteralChange={handleInlineLiteralChange}
          />
        ))}

        {arguments_.length === 0 && (
          <div className="arguments-empty">
            {isVariableArity
              ? 'No arguments. Click below to add one.'
              : 'No arguments connected.'}
          </div>
        )}
      </div>

      {isVariableArity && canAddArg && (
        <button
          type="button"
          className="arguments-add-btn"
          onClick={handleAddArgument}
        >
          <Plus size={14} />
          <span>Add Argument</span>
        </button>
      )}

      {/* Arity hint for variable-arity operators */}
      {isVariableArity && opConfig?.arity.min !== undefined && opConfig.arity.min > 0 && (
        <div className="arguments-hint">
          Minimum: {opConfig.arity.min} argument{opConfig.arity.min !== 1 ? 's' : ''}
          {opConfig.arity.max && ` | Maximum: ${opConfig.arity.max}`}
        </div>
      )}

      {/* Arity hint for fixed-arity operators */}
      {!isVariableArity && opConfig?.arity && (
        <div className="arguments-hint">
          {opConfig.arity.type === 'unary' && 'Requires exactly 1 argument'}
          {opConfig.arity.type === 'binary' && 'Requires exactly 2 arguments'}
          {opConfig.arity.type === 'ternary' && 'Requires exactly 3 arguments'}
          {opConfig.arity.type === 'range' &&
            `Requires ${opConfig.arity.min ?? 0}-${opConfig.arity.max ?? '∞'} arguments`}
        </div>
      )}
    </div>
  );
});

/**
 * Individual argument item - renders differently based on whether it's an inline literal or a linked node
 */
interface ArgumentItemProps {
  arg: ArgumentInfo;
  isVariableArity: boolean;
  canRemoveArg: boolean;
  onSelect: (childId: string) => void;
  onRemove: (argIndex: number) => void;
  onLiteralChange: (childId: string, value: JsonLogicValue, valueType: LiteralNodeData['valueType']) => void;
  onInlineLiteralChange: (argIndex: number, value: JsonLogicValue) => void;
}

const ArgumentItem = memo(function ArgumentItem({
  arg,
  isVariableArity,
  canRemoveArg,
  onSelect,
  onRemove,
  onLiteralChange,
  onInlineLiteralChange,
}: ArgumentItemProps) {
  const { index, isInline, value, valueType, childNode, childId } = arg;

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
        <div className="argument-index">{index + 1}</div>
        <div className="argument-literal-input">
          {valueType === 'number' && (
            <input
              type="number"
              className="argument-input argument-input--number"
              value={localValue}
              onChange={handleInlineNumberChange}
              onBlur={handleInlineNumberBlur}
              step="any"
            />
          )}
          {valueType === 'string' && (
            <input
              type="text"
              className="argument-input argument-input--string"
              value={localValue}
              onChange={handleInlineStringChange}
              onBlur={handleInlineStringBlur}
              placeholder="(empty string)"
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
              {formatRawValue(value ?? null)}
            </span>
          )}
        </div>
        {isVariableArity && canRemoveArg && (
          <button
            type="button"
            className="argument-remove"
            onClick={() => onRemove(index)}
            title="Remove this argument"
          >
            <Link2Off size={14} />
          </button>
        )}
      </div>
    );
  }

  // Render child node literal (has its own node that can be updated)
  if (isChildLiteral && childLiteralData && childId) {
    return (
      <div className="argument-item">
        <div className="argument-index">{index + 1}</div>
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
          <button
            type="button"
            className="argument-remove"
            onClick={() => onRemove(childNode?.data.argIndex ?? index)}
            title="Remove this argument"
          >
            <Link2Off size={14} />
          </button>
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
        <button
          type="button"
          className="argument-remove"
          onClick={() => onRemove(childNode?.data.argIndex ?? index)}
          title="Remove this argument"
        >
          <Link2Off size={14} />
        </button>
      )}
    </div>
  );
});
