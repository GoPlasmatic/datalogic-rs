import type { NodeTypes } from '@xyflow/react';
import { OperatorNode } from './OperatorNode';
import { VariableNode } from './VariableNode';
import { LiteralNode } from './LiteralNode';
import { VerticalCellNode } from './VerticalCellNode';
import { StructureNode } from './StructureNode';

// Cast to NodeTypes to satisfy ReactFlow's type requirements
// The components are correctly typed internally but ReactFlow's generic constraints are strict
export const nodeTypes: NodeTypes = {
  operator: OperatorNode as NodeTypes['string'],
  variable: VariableNode as NodeTypes['string'],
  literal: LiteralNode as NodeTypes['string'],
  verticalCell: VerticalCellNode as NodeTypes['string'],
  structure: StructureNode as NodeTypes['string'],
};
