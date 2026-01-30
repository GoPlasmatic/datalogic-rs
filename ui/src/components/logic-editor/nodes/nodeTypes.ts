import type { NodeTypes } from '@xyflow/react';
import { UnifiedOperatorNode } from './UnifiedOperatorNode';
import { LiteralNode } from './LiteralNode';
import { StructureNode } from './StructureNode';

// Cast to NodeTypes to satisfy ReactFlow's type requirements
// The components are correctly typed internally but ReactFlow's generic constraints are strict
export const nodeTypes: NodeTypes = {
  operator: UnifiedOperatorNode as NodeTypes['string'],
  literal: LiteralNode as NodeTypes['string'],
  structure: StructureNode as NodeTypes['string'],
};
