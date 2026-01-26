import type {
  LogicNode,
  LogicEdge,
  ConversionResult,
  JsonLogicValue,
  CellData,
  LiteralNodeData,
  VariableNodeData,
  VerticalCellNodeData,
  OperatorNodeData,
  StructureNodeData,
  StructureElement,
} from '../types';
import type { TracedResult, ExpressionNode } from '../types/trace';
import type { ParentInfo } from './converters/types';

// Extended result type that includes trace-to-visual node mapping
export interface TraceConversionResult extends ConversionResult {
  traceNodeMap: Map<string, string>;  // trace-{id} -> visual node ID
}
import { getOperatorMeta, getOperatorTitle, TRUNCATION_LIMITS } from '../constants';
import { CATEGORY_ICONS, ITERATOR_ARG_ICONS, getOperandTypeIcon, CONTROL_ICONS, type IconName } from './icons';
import { generateExpressionText, generateArgSummary, formatOperandLabel } from './formatting';
import { isSimpleOperand, getValueType, isDataStructure, isJsonLogicExpression } from './type-helpers';
import { createBranchEdge, createArgEdge } from './node-factory';

// Convert trace node ID to string node ID
function traceIdToNodeId(id: number): string {
  return `trace-${id}`;
}

type ValueType = 'boolean' | 'number' | 'string' | 'null' | 'array' | 'object' | 'undefined';

// Build evaluation results map from trace execution steps
export function buildEvaluationResultsFromTrace(trace: TracedResult): Map<string, { value: unknown; error: string | null; type: ValueType }> {
  const results = new Map<string, { value: unknown; error: string | null; type: ValueType }>();

  if (!trace.steps) {
    return results;
  }

  for (const step of trace.steps) {
    const nodeId = traceIdToNodeId(step.node_id);

    // Determine the value type
    let valueType: ValueType = 'undefined';
    const value = step.result;
    if (value === null) valueType = 'null';
    else if (value === undefined) valueType = 'undefined';
    else if (Array.isArray(value)) valueType = 'array';
    else if (typeof value === 'boolean') valueType = 'boolean';
    else if (typeof value === 'number') valueType = 'number';
    else if (typeof value === 'string') valueType = 'string';
    else if (typeof value === 'object') valueType = 'object';

    results.set(nodeId, {
      value: step.result,
      error: step.error ?? null,
      type: valueType,
    });
  }

  return results;
}

// Options for trace conversion
export interface TraceToNodesOptions {
  /** Enable structure preserve mode for JSON templates with embedded JSONLogic */
  preserveStructure?: boolean;
  /** Original expression value - used to preserve key ordering in structure nodes */
  originalValue?: JsonLogicValue;
}

// Main conversion function
export function traceToNodes(trace: TracedResult, options: TraceToNodesOptions = {}): TraceConversionResult {
  if (!trace.expression_tree) {
    return { nodes: [], edges: [], rootId: null, traceNodeMap: new Map() };
  }

  const nodes: LogicNode[] = [];
  const edges: LogicEdge[] = [];
  const traceNodeMap: Map<string, string> = new Map();

  // Use original value if provided (preserves key ordering), otherwise parse from trace
  const rootExpression = options.originalValue ?? JSON.parse(trace.expression_tree.expression);

  processExpressionNode(trace.expression_tree, {
    nodes,
    edges,
    traceNodeMap,
    preserveStructure: options.preserveStructure ?? false,
  }, {}, rootExpression);

  return {
    nodes,
    edges,
    rootId: traceIdToNodeId(trace.expression_tree.id),
    traceNodeMap,
  };
}

interface TraceContext {
  nodes: LogicNode[];
  edges: LogicEdge[];
  traceNodeMap: Map<string, string>;
  preserveStructure: boolean;
}

// Map all children of an expression node to a parent visual node ID (for inlined children)
function mapInlinedChildren(
  children: ExpressionNode[],
  parentVisualId: string,
  traceNodeMap: Map<string, string>
): void {
  for (const child of children) {
    const traceId = traceIdToNodeId(child.id);
    traceNodeMap.set(traceId, parentVisualId);
    // Also recursively map any nested children
    if (child.children && child.children.length > 0) {
      mapInlinedChildren(child.children, parentVisualId, traceNodeMap);
    }
  }
}

// Process a single expression node from the trace
// originalExpression can be provided to preserve key ordering (used for root and structure nodes)
function processExpressionNode(
  exprNode: ExpressionNode,
  context: TraceContext,
  parentInfo: ParentInfo = {},
  originalExpression?: JsonLogicValue
): string {
  const nodeId = traceIdToNodeId(exprNode.id);
  // Use original expression if provided (preserves key ordering), otherwise parse from trace
  const expression: JsonLogicValue = originalExpression ?? JSON.parse(exprNode.expression);

  // Register this node in the trace map - it maps to itself since it creates a visual node
  context.traceNodeMap.set(nodeId, nodeId);

  // Determine the type of expression and create appropriate node
  const nodeType = determineNodeType(expression, context.preserveStructure);

  switch (nodeType) {
    case 'literal':
      createLiteralNodeFromTrace(nodeId, expression, exprNode.children, context, parentInfo);
      break;
    case 'variable':
      createVariableNodeFromTrace(nodeId, expression, exprNode.children, context, parentInfo);
      break;
    case 'if':
      createIfElseNodeFromTrace(nodeId, expression, exprNode.children, context, parentInfo);
      break;
    case 'verticalCell':
      createVerticalCellNodeFromTrace(nodeId, expression, exprNode.children, context, parentInfo);
      break;
    case 'operator':
      createOperatorNodeFromTrace(nodeId, expression, exprNode.children, context, parentInfo);
      break;
    case 'structure':
      createStructureNodeFromTrace(nodeId, expression, exprNode.children, context, parentInfo);
      break;
  }

  return nodeId;
}

type NodeType = 'literal' | 'variable' | 'if' | 'verticalCell' | 'operator' | 'structure';

// Create a fallback node when no trace match is found
// This properly handles all node types (operators, variables, structures, etc.)
function createFallbackNode(
  nodeId: string,
  value: JsonLogicValue,
  context: TraceContext,
  parentInfo: ParentInfo
): void {
  // Determine the appropriate node type based on the value
  const nodeType = determineNodeType(value, context.preserveStructure);

  // Create the appropriate node type, passing empty children since we don't have trace data
  switch (nodeType) {
    case 'literal':
      createLiteralNodeFromTrace(nodeId, value, [], context, parentInfo);
      break;
    case 'variable':
      createVariableNodeFromTrace(nodeId, value, [], context, parentInfo);
      break;
    case 'if':
      createIfElseNodeFromTrace(nodeId, value, [], context, parentInfo);
      break;
    case 'verticalCell':
      createVerticalCellNodeFromTrace(nodeId, value, [], context, parentInfo);
      break;
    case 'operator':
      createOperatorNodeFromTrace(nodeId, value, [], context, parentInfo);
      break;
    case 'structure':
      createStructureNodeFromTrace(nodeId, value, [], context, parentInfo);
      break;
  }
}

// Determine what kind of node to create based on expression
function determineNodeType(expr: JsonLogicValue, preserveStructure: boolean): NodeType {
  // In preserveStructure mode, check for data structures first
  if (preserveStructure && isDataStructure(expr)) {
    return 'structure';
  }

  // Primitives and arrays -> literal
  if (expr === null || typeof expr !== 'object' || Array.isArray(expr)) {
    return 'literal';
  }

  const keys = Object.keys(expr);
  if (keys.length !== 1) return 'literal'; // Invalid JSONLogic, treat as literal

  const operator = keys[0];

  // Variable operators
  if (['var', 'val', 'exists'].includes(operator)) {
    return 'variable';
  }

  // If/else -> special handling
  if (operator === 'if' || operator === '?:') {
    return 'if';
  }

  // Multi-arg operators -> verticalCell
  const operands = (expr as Record<string, unknown>)[operator];
  const args = Array.isArray(operands) ? operands : [operands];
  if (args.length > 1) {
    return 'verticalCell';
  }

  return 'operator';
}

// Create a literal node
function createLiteralNodeFromTrace(
  nodeId: string,
  value: JsonLogicValue,
  children: ExpressionNode[],
  context: TraceContext,
  parentInfo: ParentInfo
): void {
  // Map any children to this node (shouldn't happen for literals, but be safe)
  if (children && children.length > 0) {
    mapInlinedChildren(children, nodeId, context.traceNodeMap);
  }

  const node: LogicNode = {
    id: nodeId,
    type: 'literal',
    position: { x: 0, y: 0 },
    data: {
      type: 'literal',
      value,
      valueType: getValueType(value),
      expression: value,
      parentId: parentInfo.parentId,
      argIndex: parentInfo.argIndex,
      branchType: parentInfo.branchType,
    } as LiteralNodeData,
  };
  context.nodes.push(node);

  // Add edge from parent if exists and not a branch type
  if (parentInfo.parentId && !parentInfo.branchType) {
    context.edges.push(createArgEdge(parentInfo.parentId, nodeId, parentInfo.argIndex ?? 0));
  }
}

// Create a variable node
function createVariableNodeFromTrace(
  nodeId: string,
  expression: JsonLogicValue,
  children: ExpressionNode[],
  context: TraceContext,
  parentInfo: ParentInfo
): void {
  // Map any children to this node (e.g., nested default value expressions)
  if (children && children.length > 0) {
    mapInlinedChildren(children, nodeId, context.traceNodeMap);
  }

  const obj = expression as Record<string, unknown>;
  const operator = Object.keys(obj)[0] as 'var' | 'val' | 'exists';
  const operands = obj[operator];

  let path: string;
  let defaultValue: JsonLogicValue | undefined;

  if (Array.isArray(operands)) {
    path = String(operands[0] ?? '');
    defaultValue = operands[1] as JsonLogicValue | undefined;
  } else {
    path = String(operands ?? '');
  }

  const node: LogicNode = {
    id: nodeId,
    type: 'variable',
    position: { x: 0, y: 0 },
    data: {
      type: 'variable',
      operator,
      path,
      defaultValue,
      expression,
      parentId: parentInfo.parentId,
      argIndex: parentInfo.argIndex,
      branchType: parentInfo.branchType,
    } as VariableNodeData,
  };
  context.nodes.push(node);

  // Add edge from parent if exists and not a branch type
  if (parentInfo.parentId && !parentInfo.branchType) {
    context.edges.push(createArgEdge(parentInfo.parentId, nodeId, parentInfo.argIndex ?? 0));
  }
}

// Create a VerticalCellNode for if/else expressions
// Each condition and then value gets its own row for handle clarity
function createIfElseNodeFromTrace(
  nodeId: string,
  expression: JsonLogicValue,
  children: ExpressionNode[],
  context: TraceContext,
  parentInfo: ParentInfo
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
      const condNodeType = determineNodeType(condition, context.preserveStructure);
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
      const thenNodeType = determineNodeType(thenValue, context.preserveStructure);
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
      const elseNodeType = determineNodeType(elseValue, context.preserveStructure);
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

// Find the matching child node for an operand by comparing expressions
function findMatchingChild(
  operand: JsonLogicValue,
  children: ExpressionNode[],
  usedIndices: Set<number>
): { child: ExpressionNode; index: number } | null {
  const operandStr = JSON.stringify(operand);

  for (let i = 0; i < children.length; i++) {
    if (usedIndices.has(i)) continue;
    // Normalize child expression by parsing and re-stringifying to ensure consistent format
    try {
      const childExprStr = JSON.stringify(JSON.parse(children[i].expression));
      if (childExprStr === operandStr) {
        return { child: children[i], index: i };
      }
    } catch {
      // If parsing fails, try direct comparison
      if (children[i].expression === operandStr) {
        return { child: children[i], index: i };
      }
    }
  }
  return null;
}

// Get the next unused child (for positional matching when exact matching fails)
function getNextUnusedChild(
  children: ExpressionNode[],
  usedIndices: Set<number>
): { child: ExpressionNode; index: number } | null {
  for (let i = 0; i < children.length; i++) {
    if (!usedIndices.has(i)) {
      return { child: children[i], index: i };
    }
  }
  return null;
}

// Create a vertical cell node for multi-arg operators
function createVerticalCellNodeFromTrace(
  nodeId: string,
  expression: JsonLogicValue,
  children: ExpressionNode[],
  context: TraceContext,
  parentInfo: ParentInfo
): void {
  const obj = expression as Record<string, unknown>;
  const operator = Object.keys(obj)[0];
  const operands = obj[operator];
  const operandArray: JsonLogicValue[] = Array.isArray(operands) ? operands : [operands];

  const meta = getOperatorMeta(operator);
  const cells: CellData[] = [];
  let branchIndex = 0;
  const usedChildIndices = new Set<number>();

  // Determine icon
  let icon: IconName = CATEGORY_ICONS[meta.category] || 'list';
  if (operator === 'or') icon = CONTROL_ICONS.orOperator;

  const iteratorIcons = ITERATOR_ARG_ICONS[operator];

  operandArray.forEach((operand, idx) => {
    const typeIcon = getOperandTypeIcon(operand as JsonLogicValue);
    const cellIcon = iteratorIcons ? iteratorIcons[idx] || typeIcon : typeIcon;

    if (isSimpleOperand(operand as JsonLogicValue)) {
      // Simple operand is inlined - map the trace child to this parent node
      const match = findMatchingChild(operand as JsonLogicValue, children, usedChildIndices);
      if (match) {
        usedChildIndices.add(match.index);
        const traceId = traceIdToNodeId(match.child.id);
        context.traceNodeMap.set(traceId, nodeId);
        // Also map any nested children
        if (match.child.children && match.child.children.length > 0) {
          mapInlinedChildren(match.child.children, nodeId, context.traceNodeMap);
        }
      }

      cells.push({
        type: 'inline',
        label: formatOperandLabel(operand as JsonLogicValue),
        icon: cellIcon,
        index: idx,
      });
    } else {
      // Complex expression - find matching child by expression content
      const match = findMatchingChild(operand as JsonLogicValue, children, usedChildIndices);
      let branchId: string;

      if (match) {
        usedChildIndices.add(match.index);
        branchId = processExpressionNode(match.child, context, {
          parentId: nodeId,
          argIndex: idx,
        });
      } else {
        // Fallback: create appropriate node based on value type
        branchId = `${nodeId}-arg-${idx}`;
        createFallbackNode(branchId, operand as JsonLogicValue, context, {
          parentId: nodeId,
          argIndex: idx,
        });
      }

      const summary = generateArgSummary(operand as JsonLogicValue);
      summary.label = generateExpressionText(operand as JsonLogicValue, TRUNCATION_LIMITS.expressionText);

      cells.push({
        type: 'branch',
        icon: cellIcon,
        branchId,
        index: idx,
        summary,
      });

      context.edges.push(createBranchEdge(nodeId, branchId, branchIndex));
      branchIndex++;
    }
  });

  const expressionText = generateExpressionText(expression);

  const node: LogicNode = {
    id: nodeId,
    type: 'verticalCell',
    position: { x: 0, y: 0 },
    data: {
      type: 'verticalCell',
      operator,
      category: meta.category,
      label: getOperatorTitle(operator),
      icon,
      cells,
      collapsed: false,
      expressionText,
      expression,
      parentId: parentInfo.parentId,
      argIndex: parentInfo.argIndex,
      branchType: parentInfo.branchType,
    } as VerticalCellNodeData,
  };
  context.nodes.push(node);

  // Add edge from parent if exists and not a branch type
  if (parentInfo.parentId && !parentInfo.branchType) {
    context.edges.push(createArgEdge(parentInfo.parentId, nodeId, parentInfo.argIndex ?? 0));
  }
}

// Create an operator node (single-arg or unary)
function createOperatorNodeFromTrace(
  nodeId: string,
  expression: JsonLogicValue,
  children: ExpressionNode[],
  context: TraceContext,
  parentInfo: ParentInfo
): void {
  const obj = expression as Record<string, unknown>;
  const operator = Object.keys(obj)[0];
  const operands = obj[operator];
  const operandArray: JsonLogicValue[] = Array.isArray(operands) ? operands : [operands];

  const meta = getOperatorMeta(operator);
  const expressionText = generateExpressionText(expression);
  const childIds: string[] = [];

  // For unary operators with simple operands, show inline
  const singleOperand = operandArray[0];
  const isUnaryWithSimpleArg = operandArray.length === 1 && isSimpleOperand(singleOperand);

  if (isUnaryWithSimpleArg) {
    // Map the simple operand child to this node (since it's inlined)
    const match = findMatchingChild(singleOperand, children, new Set());
    if (match) {
      const traceId = traceIdToNodeId(match.child.id);
      context.traceNodeMap.set(traceId, nodeId);
      // Also map any nested children
      if (match.child.children && match.child.children.length > 0) {
        mapInlinedChildren(match.child.children, nodeId, context.traceNodeMap);
      }
    }

    // Create inline operator node
    const node: LogicNode = {
      id: nodeId,
      type: 'operator',
      position: { x: 0, y: 0 },
      data: {
        type: 'operator',
        operator,
        category: meta.category,
        label: getOperatorTitle(operator),
        childIds: [],
        collapsed: false,
        expressionText,
        expression,
        inlineDisplay: expressionText,
        parentId: parentInfo.parentId,
        argIndex: parentInfo.argIndex,
        branchType: parentInfo.branchType,
      } as OperatorNodeData,
    };
    context.nodes.push(node);

    // Add edge from parent if exists and not a branch type
    if (parentInfo.parentId && !parentInfo.branchType) {
      context.edges.push(createArgEdge(parentInfo.parentId, nodeId, parentInfo.argIndex ?? 0));
    }
    return;
  }

  // Process children - match by expression content
  const usedChildIndices = new Set<number>();
  operandArray.forEach((operand, idx) => {
    if (!isSimpleOperand(operand)) {
      const match = findMatchingChild(operand, children, usedChildIndices);
      if (match) {
        usedChildIndices.add(match.index);
        const childId = processExpressionNode(match.child, context, {
          parentId: nodeId,
          argIndex: idx,
        });
        childIds.push(childId);
      }
    } else {
      // Simple operand is inlined - map the trace child to this parent node
      const match = findMatchingChild(operand, children, usedChildIndices);
      if (match) {
        usedChildIndices.add(match.index);
        const traceId = traceIdToNodeId(match.child.id);
        context.traceNodeMap.set(traceId, nodeId);
        // Also map any nested children
        if (match.child.children && match.child.children.length > 0) {
          mapInlinedChildren(match.child.children, nodeId, context.traceNodeMap);
        }
      }
    }
  });

  const node: LogicNode = {
    id: nodeId,
    type: 'operator',
    position: { x: 0, y: 0 },
    data: {
      type: 'operator',
      operator,
      category: meta.category,
      label: getOperatorTitle(operator),
      childIds,
      collapsed: false,
      expressionText,
      expression,
      parentId: parentInfo.parentId,
      argIndex: parentInfo.argIndex,
      branchType: parentInfo.branchType,
    } as OperatorNodeData,
  };
  context.nodes.push(node);

  // Add edge from parent if exists and not a branch type
  if (parentInfo.parentId && !parentInfo.branchType) {
    context.edges.push(createArgEdge(parentInfo.parentId, nodeId, parentInfo.argIndex ?? 0));
  }
}

// Placeholder marker used in formatted JSON for expressions
const EXPR_PLACEHOLDER = '{{EXPR}}';
// The placeholder as it appears in JSON.stringify output (with quotes)
const EXPR_PLACEHOLDER_QUOTED = `"${EXPR_PLACEHOLDER}"`;

// Create a structure node for data structures with embedded JSONLogic
function createStructureNodeFromTrace(
  nodeId: string,
  expression: JsonLogicValue,
  children: ExpressionNode[],
  context: TraceContext,
  parentInfo: ParentInfo
): void {
  const isArray = Array.isArray(expression);
  const elements: StructureElement[] = [];
  const usedChildIndices = new Set<number>();
  let expressionIndex = 0;

  // Build a modified structure for JSON formatting with placeholders
  const structureWithPlaceholders = walkAndCollectFromTrace(
    expression as Record<string, unknown> | unknown[],
    [],
    (path, item, key) => {
      if (isJsonLogicExpression(item)) {
        // Find matching child in trace
        const match = findMatchingChild(item as JsonLogicValue, children, usedChildIndices);
        let branchId: string;

        if (match) {
          usedChildIndices.add(match.index);
          branchId = processExpressionNode(match.child, context, {
            parentId: nodeId,
            argIndex: expressionIndex,
          });
        } else {
          // Fallback: create appropriate node based on value type
          // Use branchType to prevent createFallbackNode from adding edges (structure node handles its own edges)
          branchId = `${nodeId}-expr-${expressionIndex}`;
          createFallbackNode(branchId, item as JsonLogicValue, context, {
            parentId: nodeId,
            argIndex: expressionIndex,
            branchType: 'branch', // Prevents edge creation in fallback
          });
        }

        elements.push({
          type: 'expression',
          path,
          key,
          branchId,
          startOffset: 0,
          endOffset: 0,
        });

        expressionIndex++;
        return EXPR_PLACEHOLDER;
      }
      return item;
    },
    context
  );

  // Format the JSON with placeholders
  const formattedJson = JSON.stringify(structureWithPlaceholders, null, 2);

  // Calculate offsets for expression placeholders
  // Note: JSON.stringify wraps strings in quotes, so we search for "{{EXPR}}"
  let searchPos = 0;
  for (const element of elements) {
    if (element.type === 'expression') {
      const placeholderPos = formattedJson.indexOf(EXPR_PLACEHOLDER_QUOTED, searchPos);
      if (placeholderPos !== -1) {
        element.startOffset = placeholderPos;
        element.endOffset = placeholderPos + EXPR_PLACEHOLDER_QUOTED.length;
        searchPos = element.endOffset;
      }
    }
  }

  // Generate expression text for collapsed view
  const expressionText = generateExpressionText(expression, 100);

  // Create the structure node
  const node: LogicNode = {
    id: nodeId,
    type: 'structure',
    position: { x: 0, y: 0 },
    data: {
      type: 'structure',
      isArray,
      formattedJson,
      elements,
      collapsed: false,
      expressionText,
      expression,
      parentId: parentInfo.parentId,
      argIndex: parentInfo.argIndex,
      branchType: parentInfo.branchType,
    } as StructureNodeData,
  };
  context.nodes.push(node);

  // Add edge from parent if exists and not a branch type
  if (parentInfo.parentId && !parentInfo.branchType) {
    context.edges.push(createArgEdge(parentInfo.parentId, nodeId, parentInfo.argIndex ?? 0));
  }

  // Add edges from structure node to expression child nodes
  let branchIdx = 0;
  for (const element of elements) {
    if (element.type === 'expression' && element.branchId) {
      context.edges.push(createBranchEdge(nodeId, element.branchId, branchIdx));
      branchIdx++;
    }
  }
}

// Check if a value should be treated as an expression branch in trace conversion
// This includes JSONLogic expressions and nested structures (when preserveStructure is enabled)
function isExpressionBranch(item: unknown, preserveStructure: boolean): boolean {
  if (isJsonLogicExpression(item)) return true;
  // In preserveStructure mode, nested structures are also separate expression nodes in the trace
  if (preserveStructure && isDataStructure(item)) return true;
  return false;
}

// Walk through a structure and transform values (for trace conversion)
function walkAndCollectFromTrace(
  value: unknown,
  path: string[],
  onValue: (path: string[], item: unknown, key?: string) => unknown,
  context: TraceContext
): unknown {
  if (Array.isArray(value)) {
    return value.map((item, index) => {
      const itemPath = [...path, String(index)];
      if (isExpressionBranch(item, context.preserveStructure)) {
        return onValue(itemPath, item);
      } else if (typeof item === 'object' && item !== null) {
        return walkAndCollectFromTrace(item, itemPath, onValue, context);
      }
      return item;
    });
  }

  if (typeof value === 'object' && value !== null) {
    const result: Record<string, unknown> = {};
    for (const [key, item] of Object.entries(value)) {
      const itemPath = [...path, key];
      if (isExpressionBranch(item, context.preserveStructure)) {
        result[key] = onValue(itemPath, item, key);
      } else if (typeof item === 'object' && item !== null) {
        result[key] = walkAndCollectFromTrace(item, itemPath, onValue, context);
      } else {
        result[key] = item;
      }
    }
    return result;
  }

  return value;
}
