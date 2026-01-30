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

import { memo, useMemo, useCallback } from 'react';
import { Plus } from 'lucide-react';
import { useEditorContext } from '../context/editor';
import type { LogicNode, OperatorNodeData, LiteralNodeData, JsonLogicValue } from '../types';
import { getOperator } from '../config/operators';
import {
  supportsVariableArgs,
  hasArguments,
  getOperatorName,
  extractArguments,
  type ArgumentInfo,
} from './utils/argument-parser';
import { rebuildVariableExpression } from './utils/expression-rebuilder';
import { formatOperandLabel } from '../utils/formatting';
import { ArgumentItem } from './ArgumentItem';

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

  // Build a map of childId -> node for cells (they use branchId references)
  const childNodeMap = useMemo(() => {
    const map = new Map<string, LogicNode>();
    childNodes.forEach((child) => {
      map.set(child.id, child);
    });
    return map;
  }, [childNodes]);

  // Extract arguments from the node's cells data
  const arguments_ = useMemo((): ArgumentInfo[] => {
    const nodeData = node.data;

    if (nodeData.type === 'operator') {
      return extractArguments(nodeData as OperatorNodeData, childNodeMap);
    }

    return [];
  }, [node.data, childNodeMap]);

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
    // For all operators, add as literal - the mutation service handles
    // operator-specific behavior (val adds editable path, if adds else-if pair, etc.)
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

        // Special handling for variable operators (var, val, exists) with editable cells
        const editableCell = opData.cells.find((c) => c.index === argIndex && c.type === 'editable');
        if (editableCell) {
          // Update the cell's value
          const newCells = opData.cells.map((cell) => {
            if (cell.index === argIndex) {
              const updatedCell = { ...cell, value: newValue };
              // Update label for scope cells
              if (cell.fieldId === 'scopeLevel' && typeof newValue === 'number') {
                updatedCell.label = `${newValue} level${newValue !== 1 ? 's' : ''} up`;
              }
              return updatedCell;
            }
            return cell;
          });

          // Rebuild expression based on operator type
          const newExpression = rebuildVariableExpression(opData.operator, newCells);

          updateNode(node.id, {
            cells: newCells,
            expression: newExpression,
            expressionText: undefined,
          });
          return;
        }

        // Standard inline literal handling
        const expr = opData.expression;
        if (expr && typeof expr === 'object' && !Array.isArray(expr)) {
          const operator = Object.keys(expr)[0];
          const operands = (expr as Record<string, unknown>)[operator];
          const operandArray: JsonLogicValue[] = Array.isArray(operands)
            ? [...operands]
            : [operands as JsonLogicValue];

          // Update the operand at the given index
          operandArray[argIndex] = newValue;

          // Update the cell's label to reflect the new value
          const newCells = opData.cells.map((cell) => {
            if (cell.index === argIndex && cell.type === 'inline') {
              return {
                ...cell,
                label: formatOperandLabel(newValue),
              };
            }
            return cell;
          });

          // Rebuild the expression
          const newExpression = { [operator]: operandArray };

          updateNode(node.id, {
            cells: newCells,
            expression: newExpression,
            expressionText: undefined,
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
          <span>{opConfig?.ui?.addArgumentLabel ?? 'Add Argument'}</span>
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
