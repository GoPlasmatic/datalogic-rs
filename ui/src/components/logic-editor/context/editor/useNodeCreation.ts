/**
 * Node Creation Hook
 *
 * Handles creating new logic nodes (variable, literal, operator, condition).
 */

import { useCallback, type Dispatch, type SetStateAction } from 'react';
import { v4 as uuidv4 } from 'uuid';
import type { LogicNode, OperatorNodeData, LiteralNodeData } from '../../types';
import type { CreateNodeType } from './types';
import { getOperator } from '../../config/operators';

export function useNodeCreation(
  pushToUndoStack: (nodes: LogicNode[]) => void,
  setInternalNodes: Dispatch<SetStateAction<LogicNode[]>>,
  onNodesChange: ((nodes: LogicNode[]) => void) | undefined,
  setSelectedNodeId: (id: string | null) => void,
  setPanelValues: Dispatch<SetStateAction<Record<string, unknown>>>,
  hasEditedRef: React.RefObject<boolean>,
  internalNodes: LogicNode[]
) {
  const hasNodes = useCallback(() => {
    return internalNodes.length > 0;
  }, [internalNodes]);

  const createNode = useCallback(
    (type: CreateNodeType, operatorName?: string) => {
      setInternalNodes((prev) => {
        pushToUndoStack(prev);

        const newNodeId = uuidv4();
        let newNode: LogicNode;

        switch (type) {
          case 'variable': {
            const varOpConfig = getOperator('var');
            newNode = {
              id: newNodeId,
              type: 'operator',
              position: { x: 0, y: 0 },
              data: {
                type: 'operator',
                operator: 'var',
                category: varOpConfig?.category || 'accessor',
                label: varOpConfig?.label || 'var',
                icon: 'database',
                cells: [
                  { type: 'editable', fieldId: 'path', fieldType: 'text', value: '', placeholder: 'path', label: '', index: 0 },
                ],
                expression: { var: '' },
              } as OperatorNodeData,
            };
            break;
          }
          case 'literal': {
            newNode = {
              id: newNodeId,
              type: 'literal',
              position: { x: 0, y: 0 },
              data: {
                type: 'literal',
                value: 0,
                valueType: 'number',
                expression: 0,
              } as LiteralNodeData,
            };
            break;
          }
          case 'operator': {
            const opName = operatorName || '+';
            const opConfig = getOperator(opName);
            newNode = {
              id: newNodeId,
              type: 'operator',
              position: { x: 0, y: 0 },
              data: {
                type: 'operator',
                operator: opName,
                category: opConfig?.category || 'arithmetic',
                label: opConfig?.label || opName,
                icon: 'list',
                cells: [],
                expression: { [opName]: [] },
              } as OperatorNodeData,
            };
            break;
          }
          case 'condition': {
            const conditionId = uuidv4();
            const thenId = uuidv4();
            const elseId = uuidv4();

            const conditionNode: LogicNode = {
              id: conditionId,
              type: 'literal',
              position: { x: 0, y: 0 },
              data: {
                type: 'literal',
                value: true,
                valueType: 'boolean',
                expression: true,
                parentId: newNodeId,
                argIndex: 0,
              } as LiteralNodeData,
            };

            const thenNode: LogicNode = {
              id: thenId,
              type: 'literal',
              position: { x: 0, y: 0 },
              data: {
                type: 'literal',
                value: 'yes',
                valueType: 'string',
                expression: 'yes',
                parentId: newNodeId,
                argIndex: 1,
              } as LiteralNodeData,
            };

            const elseNode: LogicNode = {
              id: elseId,
              type: 'literal',
              position: { x: 0, y: 0 },
              data: {
                type: 'literal',
                value: 'no',
                valueType: 'string',
                expression: 'no',
                parentId: newNodeId,
                argIndex: 2,
              } as LiteralNodeData,
            };

            const ifConfig = getOperator('if');
            newNode = {
              id: newNodeId,
              type: 'operator',
              position: { x: 0, y: 0 },
              data: {
                type: 'operator',
                operator: 'if',
                category: ifConfig?.category || 'control',
                label: ifConfig?.label || 'if',
                icon: 'diamond',
                cells: [
                  { type: 'branch', icon: 'diamond', rowLabel: 'If', branchId: conditionId, index: 0 },
                  { type: 'branch', icon: 'check', rowLabel: 'Then', branchId: thenId, index: 1 },
                  { type: 'branch', icon: 'x', rowLabel: 'Else', branchId: elseId, index: 2 },
                ],
                expression: { if: [true, 'yes', 'no'] },
              } as OperatorNodeData,
            };

            if (prev.length === 0) {
              const newNodes = [newNode, conditionNode, thenNode, elseNode];
              hasEditedRef.current = true;
              onNodesChange?.(newNodes);
              setSelectedNodeId(newNodeId);
              setPanelValues({});
              return newNodes;
            }

            const rootNode = prev.find((n) => !n.data.parentId);
            if (rootNode) {
              const updatedRoot = {
                ...rootNode,
                data: {
                  ...rootNode.data,
                  parentId: newNodeId,
                  argIndex: 1,
                },
              };

              const updatedIfNode = {
                ...newNode,
                data: {
                  ...newNode.data,
                  cells: [
                    { type: 'branch' as const, icon: 'diamond', rowLabel: 'If', branchId: conditionId, index: 0 },
                    { type: 'branch' as const, icon: 'check', rowLabel: 'Then', branchId: rootNode.id, index: 1 },
                    { type: 'branch' as const, icon: 'x', rowLabel: 'Else', branchId: elseId, index: 2 },
                  ],
                },
              };

              const newNodes: LogicNode[] = [
                updatedIfNode as LogicNode,
                conditionNode,
                elseNode,
                ...prev.map((n) => (n.id === rootNode.id ? updatedRoot : n)),
              ];
              hasEditedRef.current = true;
              onNodesChange?.(newNodes);
              setSelectedNodeId(newNodeId);
              setPanelValues({});
              return newNodes;
            }

            return prev;
          }
          default:
            return prev;
        }

        if (prev.length === 0) {
          const newNodes = [newNode];
          hasEditedRef.current = true;
          onNodesChange?.(newNodes);
          setSelectedNodeId(newNodeId);
          setPanelValues({});
          return newNodes;
        }

        if (type === 'operator') {
          const rootNode = prev.find((n) => !n.data.parentId);
          if (rootNode) {
            const updatedRoot = {
              ...rootNode,
              data: {
                ...rootNode.data,
                parentId: newNodeId,
                argIndex: 0,
              },
            };

            const updatedOp = {
              ...newNode,
              data: {
                ...newNode.data,
                cells: [{ type: 'branch' as const, branchId: rootNode.id, index: 0 }],
              },
            };

            const newNodes: LogicNode[] = [
              updatedOp as LogicNode,
              ...prev.map((n) => (n.id === rootNode.id ? updatedRoot : n)),
            ];
            hasEditedRef.current = true;
            onNodesChange?.(newNodes);
            setSelectedNodeId(newNodeId);
            setPanelValues({});
            return newNodes;
          }
        }

        const newNodes = [newNode];
        hasEditedRef.current = true;
        onNodesChange?.(newNodes);
        setSelectedNodeId(newNodeId);
        setPanelValues({});
        return newNodes;
      });
    },
    [onNodesChange, pushToUndoStack, setInternalNodes, setSelectedNodeId, setPanelValues, hasEditedRef]
  );

  return { createNode, hasNodes };
}
