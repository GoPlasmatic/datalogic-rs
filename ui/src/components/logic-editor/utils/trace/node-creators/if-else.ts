import type {
  LogicNode,
  JsonLogicValue,
  CellData,
  VerticalCellNodeData,
} from '../../../types';
import type { ExpressionNode } from '../../../types/trace';
import type { ParentInfo } from '../../converters/types';
import type { TraceContext, NodeType } from '../types';
import { generateExpressionText } from '../../formatting';
import { createBranchEdge, createArgEdge } from '../../node-factory';
import { findMatchingChild, getNextUnusedChild } from '../child-matching';
import { determineNodeType } from '../node-type';

// Forward declaration for processExpressionNode and createFallbackNode
type ProcessExpressionNodeFn = (
  exprNode: ExpressionNode,
  context: TraceContext,
  parentInfo: ParentInfo,
  originalExpression?: JsonLogicValue
) => string;

type CreateFallbackNodeFn = (
  nodeId: string,
  value: JsonLogicValue,
  context: TraceContext,
  parentInfo: ParentInfo
) => void;

/**
 * Create a VerticalCellNode for if/else expressions from trace data
 * Each condition and then value gets its own row for handle clarity
 */
export function createIfElseNodeFromTrace(
  nodeId: string,
  expression: JsonLogicValue,
  children: ExpressionNode[],
  context: TraceContext,
  parentInfo: ParentInfo,
  processExpressionNode: ProcessExpressionNodeFn,
  createFallbackNode: CreateFallbackNodeFn
): void {
  const obj = expression as Record<string, unknown>;
  const operator = Object.keys(obj)[0];
  const ifArgs = obj[operator] as JsonLogicValue[];
  const usedChildIndices = new Set<number>();

  const cells: CellData[] = [];
  let cellIndex = 0;
  let branchIndex = 0;

  // Parse the if-else chain
  let idx = 0;
  while (idx < ifArgs.length - 1) {
    const condition = ifArgs[idx];
    const thenValue = ifArgs[idx + 1];
    const isFirst = idx === 0;

    // Process condition branch
    let conditionBranchId: string;
    const condMatch = findMatchingChild(condition, children, usedChildIndices);
    if (condMatch) {
      usedChildIndices.add(condMatch.index);
      conditionBranchId = processExpressionNode(condMatch.child, context, {
        parentId: nodeId,
        argIndex: idx,
      });
    } else {
      // Try positional matching if exact matching fails and value is complex
      const condNodeType: NodeType = determineNodeType(condition, context.preserveStructure);
      const nextUnused = (condNodeType !== 'literal') ? getNextUnusedChild(children, usedChildIndices) : null;
      if (nextUnused) {
        // Use the trace child for proper debug step mapping
        usedChildIndices.add(nextUnused.index);
        conditionBranchId = processExpressionNode(nextUnused.child, context, {
          parentId: nodeId,
          argIndex: idx,
        }, condition); // Pass original value to preserve key ordering
      } else {
        // True fallback: create node without trace mapping
        conditionBranchId = `${nodeId}-cond-${idx}`;
        createFallbackNode(conditionBranchId, condition, context, {
          parentId: nodeId,
          argIndex: idx,
        });
      }
    }

    // Create condition edge
    context.edges.push(createBranchEdge(nodeId, conditionBranchId, branchIndex));

    // Create cell for condition (If or Else If)
    const conditionText = generateExpressionText(condition, 40);
    cells.push({
      type: 'branch',
      icon: 'diamond',
      rowLabel: isFirst ? 'If' : 'Else If',
      label: conditionText,
      branchId: conditionBranchId,
      index: cellIndex,
    });
    cellIndex++;
    branchIndex++;

    // Process then branch
    let thenBranchId: string;
    const thenMatch = findMatchingChild(thenValue, children, usedChildIndices);
    if (thenMatch) {
      usedChildIndices.add(thenMatch.index);
      thenBranchId = processExpressionNode(thenMatch.child, context, {
        parentId: nodeId,
        argIndex: idx + 1,
        branchType: 'yes',
      });
    } else {
      // Try positional matching if exact matching fails and value is complex
      const thenNodeType: NodeType = determineNodeType(thenValue, context.preserveStructure);
      const nextUnused = (thenNodeType !== 'literal') ? getNextUnusedChild(children, usedChildIndices) : null;
      if (nextUnused) {
        // Use the trace child for proper debug step mapping
        usedChildIndices.add(nextUnused.index);
        thenBranchId = processExpressionNode(nextUnused.child, context, {
          parentId: nodeId,
          argIndex: idx + 1,
          branchType: 'yes',
        }, thenValue); // Pass original value to preserve key ordering
      } else {
        // True fallback: create node without trace mapping
        thenBranchId = `${nodeId}-then-${idx}`;
        createFallbackNode(thenBranchId, thenValue, context, {
          parentId: nodeId,
          argIndex: idx + 1,
          branchType: 'yes',
        });
      }
    }

    // Create then edge
    context.edges.push(createBranchEdge(nodeId, thenBranchId, branchIndex));

    // Create cell for then value
    const thenText = generateExpressionText(thenValue, 40);
    cells.push({
      type: 'branch',
      icon: 'check',
      rowLabel: 'Then',
      label: thenText,
      branchId: thenBranchId,
      index: cellIndex,
    });
    cellIndex++;
    branchIndex++;

    idx += 2;
  }

  // Handle final else (if exists)
  const hasFinalElse = ifArgs.length % 2 === 1;
  if (hasFinalElse) {
    const elseValue = ifArgs[ifArgs.length - 1];

    // Process else branch
    let elseBranchId: string;
    const elseMatch = findMatchingChild(elseValue, children, usedChildIndices);
    if (elseMatch) {
      usedChildIndices.add(elseMatch.index);
      elseBranchId = processExpressionNode(elseMatch.child, context, {
        parentId: nodeId,
        argIndex: ifArgs.length - 1,
        branchType: 'no',
      });
    } else {
      // Try positional matching if exact matching fails and value is complex
      const elseNodeType: NodeType = determineNodeType(elseValue, context.preserveStructure);
      const nextUnused = (elseNodeType !== 'literal') ? getNextUnusedChild(children, usedChildIndices) : null;
      if (nextUnused) {
        // Use the trace child for proper debug step mapping
        usedChildIndices.add(nextUnused.index);
        elseBranchId = processExpressionNode(nextUnused.child, context, {
          parentId: nodeId,
          argIndex: ifArgs.length - 1,
          branchType: 'no',
        }, elseValue); // Pass original value to preserve key ordering
      } else {
        // True fallback: create node without trace mapping
        elseBranchId = `${nodeId}-else`;
        createFallbackNode(elseBranchId, elseValue, context, {
          parentId: nodeId,
          argIndex: ifArgs.length - 1,
          branchType: 'no',
        });
      }
    }

    // Create else edge
    context.edges.push(createBranchEdge(nodeId, elseBranchId, branchIndex));

    const elseText = generateExpressionText(elseValue, 40);

    cells.push({
      type: 'branch',
      icon: 'x',
      rowLabel: 'Else',
      label: elseText,
      branchId: elseBranchId,
      index: cellIndex,
    });
  }

  // Generate expression text for the entire if/else
  const expressionText = generateExpressionText(expression);

  // Create the VerticalCellNode
  const ifElseNode: LogicNode = {
    id: nodeId,
    type: 'verticalCell',
    position: { x: 0, y: 0 },
    data: {
      type: 'verticalCell',
      operator: 'if',
      category: 'control',
      label: 'If / Then / Else',
      icon: 'diamond',
      cells,
      collapsed: false,
      expressionText,
      collapsedCellIndices: [],
      parentId: parentInfo.parentId,
      argIndex: parentInfo.argIndex,
      branchType: parentInfo.branchType,
      expression,
    } as VerticalCellNodeData,
  };
  context.nodes.push(ifElseNode);

  // Add edge from parent if exists and not a branch type
  if (parentInfo.parentId && !parentInfo.branchType) {
    context.edges.push(createArgEdge(parentInfo.parentId, nodeId, parentInfo.argIndex ?? 0));
  }
}
