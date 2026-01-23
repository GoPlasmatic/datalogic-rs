import type { Node } from '@xyflow/react';
import type {
  LogicNode,
  OperatorNodeData,
  VariableNodeData,
  LiteralNodeData,
  VerticalCellNodeData,
  DecisionNodeData,
  StructureNodeData,
} from '../types';

// Type guard for operator nodes
export function isOperatorNode(node: LogicNode): node is Node<OperatorNodeData> {
  return node.data.type === 'operator';
}

// Type guard for variable nodes
export function isVariableNode(node: LogicNode): node is Node<VariableNodeData> {
  return node.data.type === 'variable';
}

// Type guard for literal nodes
export function isLiteralNode(node: LogicNode): node is Node<LiteralNodeData> {
  return node.data.type === 'literal';
}

// Type guard for vertical cell nodes
export function isVerticalCellNode(node: LogicNode): node is Node<VerticalCellNodeData> {
  return node.data.type === 'verticalCell';
}

// Type guard for decision nodes
export function isDecisionNode(node: LogicNode): node is Node<DecisionNodeData> {
  return node.data.type === 'decision';
}

// Type guard for structure nodes
export function isStructureNode(node: LogicNode): node is Node<StructureNodeData> {
  return node.data.type === 'structure';
}

// Type guard for collapsible nodes (operator or verticalCell)
export function isCollapsibleNode(
  node: LogicNode
): node is Node<OperatorNodeData> | Node<VerticalCellNodeData> {
  return node.data.type === 'operator' || node.data.type === 'verticalCell';
}

// Helper to safely get operator node data
export function getOperatorNodeData(node: LogicNode): OperatorNodeData | null {
  return isOperatorNode(node) ? node.data : null;
}

// Helper to safely get vertical cell node data
export function getVerticalCellNodeData(node: LogicNode): VerticalCellNodeData | null {
  return isVerticalCellNode(node) ? node.data : null;
}
