import type { Node } from '@xyflow/react';
import type {
  LogicNode,
  OperatorNodeData,
  LiteralNodeData,
  StructureNodeData,
} from '../types';

// Type guard for operator nodes (now handles all operators including var, val, exists, if, etc.)
export function isOperatorNode(node: LogicNode): node is Node<OperatorNodeData> {
  return node.data.type === 'operator';
}

// Type guard for literal nodes
export function isLiteralNode(node: LogicNode): node is Node<LiteralNodeData> {
  return node.data.type === 'literal';
}

// Type guard for structure nodes
export function isStructureNode(node: LogicNode): node is Node<StructureNodeData> {
  return node.data.type === 'structure';
}

// Type guard for collapsible nodes (all operator nodes are collapsible)
export function isCollapsibleNode(
  node: LogicNode
): node is Node<OperatorNodeData> {
  return node.data.type === 'operator';
}

// Helper to safely get operator node data
export function getOperatorNodeData(node: LogicNode): OperatorNodeData | null {
  return isOperatorNode(node) ? node.data : null;
}
