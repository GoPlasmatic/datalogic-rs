/**
 * Editor Context Provider
 *
 * Provides state management for the visual editor including
 * node selection, edit mode, and panel field values.
 */

import { useState, useCallback, useMemo, useEffect, useRef, type ReactNode } from 'react';
import { v4 as uuidv4 } from 'uuid';
import type { LogicNode, LogicNodeData, OperatorNodeData, LiteralNodeData, VerticalCellNodeData, VariableNodeData } from '../../types';
import type { EditorContextValue, CreateNodeType, ClipboardData } from './types';
import { EditorContext } from './context';
import { panelValuesToNodeData } from '../../utils/node-updaters';
import { deleteNodeAndDescendants } from '../../utils/node-deletion';
import { getOperator } from '../../config/operators';

interface EditorProviderProps {
  children: ReactNode;
  nodes: LogicNode[];
  initialEditMode?: boolean;
  /** Callback when nodes change (for propagating changes up) */
  onNodesChange?: (nodes: LogicNode[]) => void;
}

export function EditorProvider({
  children,
  nodes: propNodes,
  initialEditMode = false,
  onNodesChange,
}: EditorProviderProps) {
  const [selectedNodeId, setSelectedNodeId] = useState<string | null>(null);
  const [selectedNodeIds, setSelectedNodeIds] = useState<Set<string>>(new Set());
  const [isEditMode, setIsEditMode] = useState(initialEditMode);
  const [panelValues, setPanelValues] = useState<Record<string, unknown>>({});

  // Internal nodes state - starts from props but can be modified
  const [internalNodes, setInternalNodes] = useState<LogicNode[]>(propNodes);

  // Track if we should use internal nodes (after first edit) or prop nodes
  const hasEditedRef = useRef(false);

  // Undo/Redo history stacks
  const MAX_HISTORY_SIZE = 50;
  const undoStackRef = useRef<LogicNode[][]>([]);
  const redoStackRef = useRef<LogicNode[][]>([]);
  const [historyVersion, setHistoryVersion] = useState(0); // Force re-render on history change

  // Clipboard for copy/paste
  const clipboardRef = useRef<ClipboardData | null>(null);
  const [clipboardVersion, setClipboardVersion] = useState(0); // Force re-render on clipboard change

  // Ref for property panel focus
  const propertyPanelFocusRef = useRef<{ focusField: (fieldId?: string) => void } | null>(null);

  // Helper to push to undo stack
  const pushToUndoStack = useCallback((nodes: LogicNode[]) => {
    undoStackRef.current = [
      ...undoStackRef.current.slice(-MAX_HISTORY_SIZE + 1),
      JSON.parse(JSON.stringify(nodes)), // Deep clone
    ];
    redoStackRef.current = []; // Clear redo stack on new action
    setHistoryVersion((v) => v + 1);
  }, []);

  // Sync nodes from props when they change (e.g., expression change from parent)
  useEffect(() => {
    // Only sync from props if we haven't made edits, or if prop nodes fundamentally changed
    // (different length or different root node)
    if (
      !hasEditedRef.current ||
      propNodes.length !== internalNodes.length ||
      propNodes[0]?.id !== internalNodes[0]?.id
    ) {
      // eslint-disable-next-line react-hooks/set-state-in-effect
      setInternalNodes(propNodes);
      hasEditedRef.current = false;
    }
  }, [propNodes]); // eslint-disable-line react-hooks/exhaustive-deps

  /* eslint-disable react-hooks/set-state-in-effect -- Synchronizing state with props is intentional */
  // Sync edit mode when prop changes (e.g., when user switches modes)
  useEffect(() => {
    setIsEditMode(initialEditMode);
    if (!initialEditMode) {
      setSelectedNodeId(null);
      setSelectedNodeIds(new Set());
      setPanelValues({});
      // Reset to prop nodes when exiting edit mode
      setInternalNodes(propNodes);
      hasEditedRef.current = false;
    }
  }, [initialEditMode, propNodes]);
  /* eslint-enable react-hooks/set-state-in-effect */

  // Use internal nodes when in edit mode, otherwise prop nodes
  const nodes = isEditMode ? internalNodes : propNodes;

  // Find the selected node from the nodes array
  // If the node no longer exists, selectedNode will be null
  const selectedNode = useMemo(() => {
    if (!selectedNodeId) return null;
    return nodes.find((n) => n.id === selectedNodeId) ?? null;
  }, [nodes, selectedNodeId]);

  // Compute all selected nodes (filter out any that no longer exist)
  const selectedNodes = useMemo(() => {
    return nodes.filter((n) => selectedNodeIds.has(n.id));
  }, [nodes, selectedNodeIds]);

  // Compute effective selectedNodeId - null if node doesn't exist
  const effectiveSelectedNodeId = selectedNode ? selectedNodeId : null;

  // Compute effective selectedNodeIds - only include existing nodes
  const effectiveSelectedNodeIds = useMemo(() => {
    const existingIds = new Set(nodes.map((n) => n.id));
    return new Set([...selectedNodeIds].filter((id) => existingIds.has(id)));
  }, [nodes, selectedNodeIds]);

  const selectNode = useCallback((nodeId: string | null) => {
    setSelectedNodeId(nodeId);
    // Clear multi-selection when doing single select
    setSelectedNodeIds(nodeId ? new Set([nodeId]) : new Set());
    // Reset panel values when selection changes
    setPanelValues({});
  }, []);

  // Set selection from ReactFlow (supports multi-select)
  const setSelection = useCallback((nodeIds: string[]) => {
    setSelectedNodeIds(new Set(nodeIds));
    // Set the first node as primary selection for properties panel
    setSelectedNodeId(nodeIds.length > 0 ? nodeIds[0] : null);
    setPanelValues({});
  }, []);

  // Toggle a node in multi-selection (Cmd/Ctrl+Click)
  const toggleNodeSelection = useCallback((nodeId: string) => {
    setSelectedNodeIds((prev) => {
      const next = new Set(prev);
      if (next.has(nodeId)) {
        next.delete(nodeId);
        // If we removed the primary selection, pick another or clear
        if (selectedNodeId === nodeId) {
          const remaining = [...next];
          setSelectedNodeId(remaining.length > 0 ? remaining[0] : null);
        }
      } else {
        next.add(nodeId);
        // If no primary selection, set this as primary
        if (!selectedNodeId) {
          setSelectedNodeId(nodeId);
        }
      }
      return next;
    });
    setPanelValues({});
  }, [selectedNodeId]);

  // Add a node to multi-selection (Shift+Click)
  const addToSelection = useCallback((nodeId: string) => {
    setSelectedNodeIds((prev) => {
      const next = new Set(prev);
      next.add(nodeId);
      return next;
    });
    // If no primary selection, set this as primary
    if (!selectedNodeId) {
      setSelectedNodeId(nodeId);
    }
    setPanelValues({});
  }, [selectedNodeId]);

  // Clear all selections
  const clearSelection = useCallback(() => {
    setSelectedNodeId(null);
    setSelectedNodeIds(new Set());
    setPanelValues({});
  }, []);

  // Select all nodes
  const selectAllNodes = useCallback(() => {
    const allIds = new Set(internalNodes.map((n) => n.id));
    setSelectedNodeIds(allIds);
    // Set the first node as primary selection
    if (internalNodes.length > 0) {
      setSelectedNodeId(internalNodes[0].id);
    }
    setPanelValues({});
  }, [internalNodes]);

  // Check if a node is selected
  const isNodeSelected = useCallback(
    (nodeId: string) => selectedNodeIds.has(nodeId),
    [selectedNodeIds]
  );

  const setEditMode = useCallback((enabled: boolean) => {
    setIsEditMode(enabled);
    // Clear selection when exiting edit mode
    if (!enabled) {
      setSelectedNodeId(null);
      setSelectedNodeIds(new Set());
      setPanelValues({});
    }
  }, []);

  const updatePanelValue = useCallback((fieldId: string, value: unknown) => {
    setPanelValues((prev) => ({ ...prev, [fieldId]: value }));
  }, []);

  const resetPanelValues = useCallback((values?: Record<string, unknown>) => {
    setPanelValues(values ?? {});
  }, []);

  // Update a node's data
  const updateNode = useCallback(
    (nodeId: string, newData: Partial<LogicNodeData>) => {
      setInternalNodes((prev) => {
        // Save current state to undo stack
        pushToUndoStack(prev);

        const newNodes = prev.map((node) => {
          if (node.id === nodeId) {
            return {
              ...node,
              data: { ...node.data, ...newData } as LogicNodeData,
            };
          }
          return node;
        });
        hasEditedRef.current = true;
        // Notify parent of change
        onNodesChange?.(newNodes);
        return newNodes;
      });
    },
    [onNodesChange, pushToUndoStack]
  );

  // Delete a node and its descendants
  const deleteNode = useCallback(
    (nodeId: string) => {
      setInternalNodes((prev) => {
        // Save current state to undo stack
        pushToUndoStack(prev);

        const newNodes = deleteNodeAndDescendants(nodeId, prev);
        hasEditedRef.current = true;
        // Clear selection if deleted node was selected
        if (selectedNodeId === nodeId) {
          setSelectedNodeId(null);
          setPanelValues({});
        }
        // Notify parent of change
        onNodesChange?.(newNodes);
        return newNodes;
      });
    },
    [selectedNodeId, onNodesChange, pushToUndoStack]
  );

  // Get child nodes for a given parent
  const getChildNodes = useCallback(
    (parentId: string): LogicNode[] => {
      const parentNode = nodes.find((n) => n.id === parentId);

      // For verticalCell nodes, get children from cells array
      if (parentNode?.data.type === 'verticalCell') {
        const vcData = parentNode.data as VerticalCellNodeData;
        const childIds: string[] = [];

        // Collect all branch IDs from cells
        for (const cell of vcData.cells) {
          if (cell.branchId) childIds.push(cell.branchId);
          if (cell.conditionBranchId) childIds.push(cell.conditionBranchId);
          if (cell.thenBranchId) childIds.push(cell.thenBranchId);
        }

        // Find and return the child nodes in order
        return childIds
          .map((id) => nodes.find((n) => n.id === id))
          .filter((n): n is LogicNode => n !== undefined);
      }

      // For operator nodes, use parentId matching
      return nodes
        .filter((n) => n.data.parentId === parentId)
        .sort((a, b) => (a.data.argIndex ?? 0) - (b.data.argIndex ?? 0));
    },
    [nodes]
  );

  // Add a new argument to an N-ary operator node
  const addArgumentToNode = useCallback(
    (nodeId: string, defaultValue: unknown = 0) => {
      setInternalNodes((prev) => {
        const parentNode = prev.find((n) => n.id === nodeId);
        if (!parentNode) return prev;

        // Save current state to undo stack
        pushToUndoStack(prev);

        const parentData = parentNode.data;

        // Handle different node types
        if (parentData.type === 'operator') {
          const operatorData = parentData as OperatorNodeData;
          const opConfig = getOperator(operatorData.operator);

          // Only allow adding for N-ary or variadic operators
          if (
            opConfig &&
            (opConfig.arity.type === 'nary' ||
              opConfig.arity.type === 'variadic' ||
              opConfig.arity.type === 'chainable')
          ) {
            // Check max arity
            if (opConfig.arity.max && operatorData.childIds.length >= opConfig.arity.max) {
              return prev;
            }

            // Find the next argument index
            const existingChildren = prev.filter((n) => n.data.parentId === nodeId);
            const maxIndex = Math.max(-1, ...existingChildren.map((n) => n.data.argIndex ?? 0));
            const newArgIndex = maxIndex + 1;

            // Create new literal node
            const newNodeId = uuidv4();
            const newNode: LogicNode = {
              id: newNodeId,
              type: 'literal',
              position: { x: 0, y: 0 },
              data: {
                type: 'literal',
                value: defaultValue,
                valueType: typeof defaultValue === 'number' ? 'number' : typeof defaultValue === 'string' ? 'string' : typeof defaultValue === 'boolean' ? 'boolean' : 'null',
                expression: defaultValue,
                parentId: nodeId,
                argIndex: newArgIndex,
              } as LiteralNodeData,
            };

            // Update parent's childIds
            const updatedParent: LogicNode = {
              ...parentNode,
              data: {
                ...operatorData,
                childIds: [...operatorData.childIds, newNodeId],
              },
            };

            const newNodes = prev.map((n) => (n.id === nodeId ? updatedParent : n));
            newNodes.push(newNode);

            hasEditedRef.current = true;
            onNodesChange?.(newNodes);
            return newNodes;
          }
        } else if (parentData.type === 'verticalCell') {
          // Handle vertical cell nodes (comparison chains, logical operators)
          const verticalData = parentData as VerticalCellNodeData;
          const opConfig = getOperator(verticalData.operator);

          if (
            opConfig &&
            (opConfig.arity.type === 'nary' ||
              opConfig.arity.type === 'variadic' ||
              opConfig.arity.type === 'chainable')
          ) {
            // Check max arity
            if (opConfig.arity.max && verticalData.cells.length >= opConfig.arity.max) {
              return prev;
            }

            // Find the next index
            const maxIndex = Math.max(-1, ...verticalData.cells.map((c) => c.index));
            const newIndex = maxIndex + 1;

            // Create new literal node
            const newNodeId = uuidv4();
            const newNode: LogicNode = {
              id: newNodeId,
              type: 'literal',
              position: { x: 0, y: 0 },
              data: {
                type: 'literal',
                value: defaultValue,
                valueType: typeof defaultValue === 'number' ? 'number' : typeof defaultValue === 'string' ? 'string' : typeof defaultValue === 'boolean' ? 'boolean' : 'null',
                expression: defaultValue,
                parentId: nodeId,
                argIndex: newIndex,
              } as LiteralNodeData,
            };

            // Update vertical cell's cells array
            const updatedParent: LogicNode = {
              ...parentNode,
              data: {
                ...verticalData,
                cells: [
                  ...verticalData.cells,
                  {
                    type: 'branch' as const,
                    branchId: newNodeId,
                    index: newIndex,
                  },
                ],
              },
            };

            const newNodes = prev.map((n) => (n.id === nodeId ? updatedParent : n));
            newNodes.push(newNode);

            hasEditedRef.current = true;
            onNodesChange?.(newNodes);
            return newNodes;
          }
        }

        return prev;
      });
    },
    [onNodesChange, pushToUndoStack]
  );

  // Remove an argument from an operator node
  const removeArgumentFromNode = useCallback(
    (nodeId: string, argIndex: number) => {
      setInternalNodes((prev) => {
        const parentNode = prev.find((n) => n.id === nodeId);
        if (!parentNode) return prev;

        // Save current state to undo stack
        pushToUndoStack(prev);

        const parentData = parentNode.data;

        if (parentData.type === 'operator') {
          const operatorData = parentData as OperatorNodeData;
          const opConfig = getOperator(operatorData.operator);

          // Check minimum arity
          const minArgs = opConfig?.arity.min ?? 0;
          if (operatorData.childIds.length <= minArgs) {
            return prev;
          }

          // Find the child to remove
          const childToRemove = prev.find(
            (n) => n.data.parentId === nodeId && n.data.argIndex === argIndex
          );
          if (!childToRemove) return prev;

          // Remove the child and its descendants
          let newNodes = deleteNodeAndDescendants(childToRemove.id, prev);

          // Update parent's childIds
          newNodes = newNodes.map((n) => {
            if (n.id === nodeId) {
              return {
                ...n,
                data: {
                  ...operatorData,
                  childIds: operatorData.childIds.filter((id) => id !== childToRemove.id),
                },
              };
            }
            // Reindex remaining children
            if (n.data.parentId === nodeId && (n.data.argIndex ?? 0) > argIndex) {
              return {
                ...n,
                data: {
                  ...n.data,
                  argIndex: (n.data.argIndex ?? 0) - 1,
                },
              };
            }
            return n;
          });

          hasEditedRef.current = true;
          onNodesChange?.(newNodes);
          return newNodes;
        } else if (parentData.type === 'verticalCell') {
          const verticalData = parentData as VerticalCellNodeData;
          const opConfig = getOperator(verticalData.operator);

          // Check minimum arity
          const minArgs = opConfig?.arity.min ?? 0;
          if (verticalData.cells.length <= minArgs) {
            return prev;
          }

          // Find the cell and its branch node
          const cellToRemove = verticalData.cells.find((c) => c.index === argIndex);
          if (!cellToRemove || !cellToRemove.branchId) return prev;

          // Remove the branch node and its descendants
          let newNodes = deleteNodeAndDescendants(cellToRemove.branchId, prev);

          // Update parent's cells array
          newNodes = newNodes.map((n) => {
            if (n.id === nodeId) {
              const updatedCells = verticalData.cells
                .filter((c) => c.index !== argIndex)
                .map((c) => ({
                  ...c,
                  index: c.index > argIndex ? c.index - 1 : c.index,
                }));
              return {
                ...n,
                data: {
                  ...verticalData,
                  cells: updatedCells,
                },
              };
            }
            // Reindex remaining children
            if (n.data.parentId === nodeId && (n.data.argIndex ?? 0) > argIndex) {
              return {
                ...n,
                data: {
                  ...n.data,
                  argIndex: (n.data.argIndex ?? 0) - 1,
                },
              };
            }
            return n;
          });

          hasEditedRef.current = true;
          onNodesChange?.(newNodes);
          return newNodes;
        }

        return prev;
      });
    },
    [onNodesChange, pushToUndoStack]
  );

  // Check if canvas has any nodes
  const hasNodes = useCallback(() => {
    return internalNodes.length > 0;
  }, [internalNodes]);

  // Create a new node (as root if canvas is empty, or wrap existing root)
  const createNode = useCallback(
    (type: CreateNodeType, operatorName?: string) => {
      setInternalNodes((prev) => {
        // Save current state to undo stack
        pushToUndoStack(prev);

        const newNodeId = uuidv4();
        let newNode: LogicNode;

        // Create node based on type
        switch (type) {
          case 'variable': {
            newNode = {
              id: newNodeId,
              type: 'variable',
              position: { x: 0, y: 0 },
              data: {
                type: 'variable',
                operator: 'var',
                path: '',
                expression: { var: '' },
              } as VariableNodeData,
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
                childIds: [],
                expression: { [opName]: [] },
              } as OperatorNodeData,
            };
            break;
          }
          case 'condition': {
            // Create a simple if/then/else structure
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
                childIds: [conditionId, thenId, elseId],
                expression: { if: [true, 'yes', 'no'] },
              } as OperatorNodeData,
            };

            // If canvas is empty, add as root
            if (prev.length === 0) {
              const newNodes = [newNode, conditionNode, thenNode, elseNode];
              hasEditedRef.current = true;
              onNodesChange?.(newNodes);
              // Select the new node
              setSelectedNodeId(newNodeId);
              setPanelValues({});
              return newNodes;
            }

            // If canvas has content, wrap existing root
            const rootNode = prev.find((n) => !n.data.parentId);
            if (rootNode) {
              // Make existing root the "then" branch
              const updatedRoot = {
                ...rootNode,
                data: {
                  ...rootNode.data,
                  parentId: newNodeId,
                  argIndex: 1, // then position
                },
              };

              // Update the if node's childIds to use existing root as then
              const updatedIfNode = {
                ...newNode,
                data: {
                  ...newNode.data,
                  childIds: [conditionId, rootNode.id, elseId],
                },
              };

              const newNodes = [
                updatedIfNode,
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

        // If canvas is empty, add as root
        if (prev.length === 0) {
          const newNodes = [newNode];
          hasEditedRef.current = true;
          onNodesChange?.(newNodes);
          // Select the new node
          setSelectedNodeId(newNodeId);
          setPanelValues({});
          return newNodes;
        }

        // If canvas has content, wrap existing root in the new operator
        if (type === 'operator') {
          const rootNode = prev.find((n) => !n.data.parentId);
          if (rootNode) {
            // Make existing root a child of the new operator
            const updatedRoot = {
              ...rootNode,
              data: {
                ...rootNode.data,
                parentId: newNodeId,
                argIndex: 0,
              },
            };

            // Update the operator's childIds
            const updatedOp = {
              ...newNode,
              data: {
                ...newNode.data,
                childIds: [rootNode.id],
              },
            };

            const newNodes = [
              updatedOp,
              ...prev.map((n) => (n.id === rootNode.id ? updatedRoot : n)),
            ];
            hasEditedRef.current = true;
            onNodesChange?.(newNodes);
            setSelectedNodeId(newNodeId);
            setPanelValues({});
            return newNodes;
          }
        }

        // For variable/literal when canvas has content, replace root
        const newNodes = [newNode];
        hasEditedRef.current = true;
        onNodesChange?.(newNodes);
        setSelectedNodeId(newNodeId);
        setPanelValues({});
        return newNodes;
      });
    },
    [onNodesChange, pushToUndoStack]
  );

  // Insert a new node on an edge (between source and target)
  const insertNodeOnEdge = useCallback(
    (
      sourceId: string,
      targetId: string,
      operatorName: string
    ) => {
      setInternalNodes((prev) => {
        const sourceNode = prev.find((n) => n.id === sourceId);
        const targetNode = prev.find((n) => n.id === targetId);

        if (!sourceNode || !targetNode) return prev;

        // Save current state to undo stack
        pushToUndoStack(prev);

        const newNodeId = uuidv4();
        let newNode: LogicNode;

        // Handle special pseudo-operators for variable/literal
        if (operatorName === '__variable__') {
          newNode = {
            id: newNodeId,
            type: 'variable',
            position: { x: 0, y: 0 },
            data: {
              type: 'variable',
              operator: 'var',
              path: '',
              expression: { var: '' },
              parentId: sourceId,
              argIndex: targetNode.data.argIndex,
            } as VariableNodeData,
          };
        } else if (operatorName === '__literal__') {
          newNode = {
            id: newNodeId,
            type: 'literal',
            position: { x: 0, y: 0 },
            data: {
              type: 'literal',
              value: 0,
              valueType: 'number',
              expression: 0,
              parentId: sourceId,
              argIndex: targetNode.data.argIndex,
            } as LiteralNodeData,
          };
        } else {
          // Create an operator node
          const opConfig = getOperator(operatorName);
          newNode = {
            id: newNodeId,
            type: 'operator',
            position: { x: 0, y: 0 },
            data: {
              type: 'operator',
              operator: operatorName,
              category: opConfig?.category || 'arithmetic',
              label: opConfig?.label || operatorName,
              childIds: [targetId], // The original target becomes a child
              expression: { [operatorName]: [] },
              parentId: sourceId,
              argIndex: targetNode.data.argIndex,
            } as OperatorNodeData,
          };
        }

        // Update nodes
        const newNodes = prev.map((n) => {
          // Update target node to point to new node as parent
          if (n.id === targetId) {
            return {
              ...n,
              data: {
                ...n.data,
                parentId: newNodeId,
                argIndex: 0, // First child of the new operator
              },
            };
          }

          // Update source node's childIds if it's an operator
          if (n.id === sourceId && n.data.type === 'operator') {
            const opData = n.data as OperatorNodeData;
            return {
              ...n,
              data: {
                ...opData,
                childIds: opData.childIds.map((id) =>
                  id === targetId ? newNodeId : id
                ),
              },
            };
          }

          // Update source node's cells if it's a verticalCell
          if (n.id === sourceId && n.data.type === 'verticalCell') {
            const vcData = n.data as VerticalCellNodeData;
            return {
              ...n,
              data: {
                ...vcData,
                cells: vcData.cells.map((cell) => {
                  if (cell.branchId === targetId) {
                    return { ...cell, branchId: newNodeId };
                  }
                  if (cell.conditionBranchId === targetId) {
                    return { ...cell, conditionBranchId: newNodeId };
                  }
                  if (cell.thenBranchId === targetId) {
                    return { ...cell, thenBranchId: newNodeId };
                  }
                  return cell;
                }),
              },
            };
          }

          return n;
        });

        // Add the new node
        newNodes.push(newNode);

        hasEditedRef.current = true;
        onNodesChange?.(newNodes);
        setSelectedNodeId(newNodeId);
        setPanelValues({});
        return newNodes;
      });
    },
    [onNodesChange, pushToUndoStack]
  );

  // Undo the last action
  const undo = useCallback(() => {
    if (undoStackRef.current.length === 0) return;

    const previousState = undoStackRef.current.pop()!;
    redoStackRef.current.push(JSON.parse(JSON.stringify(internalNodes)));
    setHistoryVersion((v) => v + 1);

    setInternalNodes(previousState);
    onNodesChange?.(previousState);

    // Clear selection to avoid stale references
    setSelectedNodeId(null);
    setSelectedNodeIds(new Set());
    setPanelValues({});
  }, [internalNodes, onNodesChange]);

  // Redo the last undone action
  const redo = useCallback(() => {
    if (redoStackRef.current.length === 0) return;

    const nextState = redoStackRef.current.pop()!;
    undoStackRef.current.push(JSON.parse(JSON.stringify(internalNodes)));
    setHistoryVersion((v) => v + 1);

    setInternalNodes(nextState);
    onNodesChange?.(nextState);

    // Clear selection to avoid stale references
    setSelectedNodeId(null);
    setSelectedNodeIds(new Set());
    setPanelValues({});
  }, [internalNodes, onNodesChange]);

  // Check if undo/redo are available (computed in useMemo with historyVersion dependency)
  const canUndo = useMemo(
    () => undoStackRef.current.length > 0,
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [historyVersion]
  );
  const canRedo = useMemo(
    () => redoStackRef.current.length > 0,
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [historyVersion]
  );

  // Helper to get all descendants of a node
  const getDescendants = useCallback(
    (nodeId: string, allNodes: LogicNode[]): LogicNode[] => {
      const descendants: LogicNode[] = [];
      const queue = [nodeId];

      while (queue.length > 0) {
        const currentId = queue.shift()!;
        const currentNode = allNodes.find((n) => n.id === currentId);

        // Get children based on node type
        let childIds: string[] = [];

        if (currentNode?.data.type === 'verticalCell') {
          // For verticalCell nodes, get children from cells array
          const vcData = currentNode.data as VerticalCellNodeData;
          for (const cell of vcData.cells) {
            if (cell.branchId) childIds.push(cell.branchId);
            if (cell.conditionBranchId) childIds.push(cell.conditionBranchId);
            if (cell.thenBranchId) childIds.push(cell.thenBranchId);
          }
        } else {
          // For other nodes, find children by parentId
          childIds = allNodes
            .filter((n) => n.data.parentId === currentId)
            .map((n) => n.id);
        }

        const children = childIds
          .map((id) => allNodes.find((n) => n.id === id))
          .filter((n): n is LogicNode => n !== undefined);

        descendants.push(...children);
        queue.push(...children.map((c) => c.id));
      }

      return descendants;
    },
    []
  );

  // Copy the selected node and its descendants to clipboard
  const copyNode = useCallback(() => {
    if (!selectedNode) return;

    // Get all descendants
    const descendants = getDescendants(selectedNode.id, internalNodes);

    // Clone the nodes for clipboard (deep copy)
    const copiedNodes = [selectedNode, ...descendants].map((n) =>
      JSON.parse(JSON.stringify(n))
    );

    clipboardRef.current = {
      nodes: copiedNodes,
      rootId: selectedNode.id,
    };
    setClipboardVersion((v) => v + 1);
  }, [selectedNode, internalNodes, getDescendants]);

  // Paste clipboard contents
  const pasteNode = useCallback(() => {
    const clipboard = clipboardRef.current;
    if (!clipboard || clipboard.nodes.length === 0) return;

    setInternalNodes((prev) => {
      // Save current state to undo stack
      pushToUndoStack(prev);

      // Create ID mapping for all copied nodes
      const idMap = new Map<string, string>();
      clipboard.nodes.forEach((n) => {
        idMap.set(n.id, uuidv4());
      });

      // Clone and remap IDs
      const clonedNodes: LogicNode[] = clipboard.nodes.map((n) => {
        const newId = idMap.get(n.id)!;
        const newNode: LogicNode = {
          ...n,
          id: newId,
          data: {
            ...n.data,
            // Remap parentId if it's in the copied set
            parentId: n.data.parentId && idMap.has(n.data.parentId)
              ? idMap.get(n.data.parentId)
              : n.data.parentId,
          },
        };

        // Remap childIds for operator nodes
        if (newNode.data.type === 'operator') {
          const opData = newNode.data as OperatorNodeData;
          newNode.data = {
            ...opData,
            childIds: opData.childIds.map((id) => idMap.get(id) ?? id),
          };
        }

        // Remap cells for verticalCell nodes
        if (newNode.data.type === 'verticalCell') {
          const vcData = newNode.data as VerticalCellNodeData;
          newNode.data = {
            ...vcData,
            cells: vcData.cells.map((cell) => ({
              ...cell,
              branchId: cell.branchId && idMap.has(cell.branchId)
                ? idMap.get(cell.branchId)
                : cell.branchId,
              conditionBranchId: cell.conditionBranchId && idMap.has(cell.conditionBranchId)
                ? idMap.get(cell.conditionBranchId)
                : cell.conditionBranchId,
              thenBranchId: cell.thenBranchId && idMap.has(cell.thenBranchId)
                ? idMap.get(cell.thenBranchId)
                : cell.thenBranchId,
            })),
          };
        }

        return newNode;
      });

      const newRootId = idMap.get(clipboard.rootId)!;
      const clonedRoot = clonedNodes.find((n) => n.id === newRootId)!;

      // If there's a selected node that isn't the root, replace it
      if (selectedNode) {
        const targetNode = prev.find((n) => n.id === selectedNode.id);
        if (targetNode && targetNode.data.parentId) {
          // Replace the selected node with the pasted tree
          // Update the cloned root to have the same parent info
          clonedRoot.data = {
            ...clonedRoot.data,
            parentId: targetNode.data.parentId,
            argIndex: targetNode.data.argIndex,
          };

          // Remove the target node and its descendants
          const targetDescendants = getDescendants(targetNode.id, prev);
          const targetIds = new Set([targetNode.id, ...targetDescendants.map((d) => d.id)]);

          // Update parent's childIds if it's an operator
          let newNodes = prev
            .filter((n) => !targetIds.has(n.id))
            .map((n) => {
              if (n.id === targetNode.data.parentId && n.data.type === 'operator') {
                const opData = n.data as OperatorNodeData;
                return {
                  ...n,
                  data: {
                    ...opData,
                    childIds: opData.childIds.map((id) =>
                      id === targetNode.id ? newRootId : id
                    ),
                  },
                };
              }
              if (n.id === targetNode.data.parentId && n.data.type === 'verticalCell') {
                const vcData = n.data as VerticalCellNodeData;
                return {
                  ...n,
                  data: {
                    ...vcData,
                    cells: vcData.cells.map((cell) => ({
                      ...cell,
                      branchId: cell.branchId === targetNode.id ? newRootId : cell.branchId,
                      conditionBranchId: cell.conditionBranchId === targetNode.id ? newRootId : cell.conditionBranchId,
                      thenBranchId: cell.thenBranchId === targetNode.id ? newRootId : cell.thenBranchId,
                    })),
                  },
                };
              }
              return n;
            });

          newNodes = [...newNodes, ...clonedNodes];

          hasEditedRef.current = true;
          onNodesChange?.(newNodes);
          setSelectedNodeId(newRootId);
          setPanelValues({});
          return newNodes;
        }
      }

      // If no selection or selected is root, replace entire tree
      clonedRoot.data = {
        ...clonedRoot.data,
        parentId: undefined,
        argIndex: undefined,
      };

      const newNodes = clonedNodes;
      hasEditedRef.current = true;
      onNodesChange?.(newNodes);
      setSelectedNodeId(newRootId);
      setPanelValues({});
      return newNodes;
    });
  }, [selectedNode, getDescendants, pushToUndoStack, onNodesChange]);

  // Check if paste is available
  const canPaste = useMemo(
    () => clipboardRef.current !== null && clipboardRef.current.nodes.length > 0,
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [clipboardVersion]
  );

  // Wrap a node in an operator (makes the node a child of the new operator)
  const wrapNodeInOperator = useCallback(
    (nodeId: string, operator: string) => {
      setInternalNodes((prev) => {
        const targetNode = prev.find((n) => n.id === nodeId);
        if (!targetNode) return prev;

        // Save current state to undo stack
        pushToUndoStack(prev);

        const newOperatorId = uuidv4();
        const opConfig = getOperator(operator);

        // Create the wrapper operator node
        const wrapperNode: LogicNode = {
          id: newOperatorId,
          type: 'operator',
          position: { x: 0, y: 0 },
          data: {
            type: 'operator',
            operator,
            category: opConfig?.category || 'logical',
            label: opConfig?.label || operator,
            childIds: [nodeId],
            expression: { [operator]: [] },
            parentId: targetNode.data.parentId,
            argIndex: targetNode.data.argIndex,
          } as OperatorNodeData,
        };

        // Update the target node to be a child of the wrapper
        const updatedTarget: LogicNode = {
          ...targetNode,
          data: {
            ...targetNode.data,
            parentId: newOperatorId,
            argIndex: 0,
          },
        };

        // Update the parent's childIds if the target had a parent
        const updatedNodes = prev.map((n) => {
          if (n.id === nodeId) {
            return updatedTarget;
          }
          // Update parent's childIds
          if (n.id === targetNode.data.parentId && n.data.type === 'operator') {
            const opData = n.data as OperatorNodeData;
            return {
              ...n,
              data: {
                ...opData,
                childIds: opData.childIds.map((id) => (id === nodeId ? newOperatorId : id)),
              },
            };
          }
          // Update parent's cells if it's a verticalCell
          if (n.id === targetNode.data.parentId && n.data.type === 'verticalCell') {
            const vcData = n.data as VerticalCellNodeData;
            return {
              ...n,
              data: {
                ...vcData,
                cells: vcData.cells.map((cell) => ({
                  ...cell,
                  branchId: cell.branchId === nodeId ? newOperatorId : cell.branchId,
                  conditionBranchId: cell.conditionBranchId === nodeId ? newOperatorId : cell.conditionBranchId,
                  thenBranchId: cell.thenBranchId === nodeId ? newOperatorId : cell.thenBranchId,
                })),
              },
            };
          }
          return n;
        });

        // Add the wrapper node
        const newNodes = [...updatedNodes, wrapperNode];

        hasEditedRef.current = true;
        onNodesChange?.(newNodes);
        setSelectedNodeId(newOperatorId);
        setPanelValues({});
        return newNodes;
      });
    },
    [onNodesChange, pushToUndoStack]
  );

  // Duplicate a node and its descendants
  const duplicateNode = useCallback(
    (nodeId: string) => {
      const targetNode = internalNodes.find((n) => n.id === nodeId);
      if (!targetNode) return;

      // Get all descendants
      const descendants = getDescendants(nodeId, internalNodes);

      // Clone the nodes (deep copy)
      const nodesToClone = [targetNode, ...descendants];

      // Create ID mapping
      const idMap = new Map<string, string>();
      nodesToClone.forEach((n) => {
        idMap.set(n.id, uuidv4());
      });

      setInternalNodes((prev) => {
        // Save current state to undo stack
        pushToUndoStack(prev);

        // Clone and remap IDs
        const clonedNodes: LogicNode[] = nodesToClone.map((n) => {
          const newId = idMap.get(n.id)!;
          const newNode: LogicNode = {
            ...JSON.parse(JSON.stringify(n)),
            id: newId,
            data: {
              ...JSON.parse(JSON.stringify(n.data)),
              parentId: n.data.parentId && idMap.has(n.data.parentId)
                ? idMap.get(n.data.parentId)
                : n.data.parentId,
            },
          };

          // Remap childIds for operator nodes
          if (newNode.data.type === 'operator') {
            const opData = newNode.data as OperatorNodeData;
            newNode.data = {
              ...opData,
              childIds: opData.childIds.map((id) => idMap.get(id) ?? id),
            };
          }

          // Remap cells for verticalCell nodes
          if (newNode.data.type === 'verticalCell') {
            const vcData = newNode.data as VerticalCellNodeData;
            newNode.data = {
              ...vcData,
              cells: vcData.cells.map((cell) => ({
                ...cell,
                branchId: cell.branchId && idMap.has(cell.branchId)
                  ? idMap.get(cell.branchId)
                  : cell.branchId,
                conditionBranchId: cell.conditionBranchId && idMap.has(cell.conditionBranchId)
                  ? idMap.get(cell.conditionBranchId)
                  : cell.conditionBranchId,
                thenBranchId: cell.thenBranchId && idMap.has(cell.thenBranchId)
                  ? idMap.get(cell.thenBranchId)
                  : cell.thenBranchId,
              })),
            };
          }

          return newNode;
        });

        const newRootId = idMap.get(nodeId)!;
        const clonedRoot = clonedNodes.find((n) => n.id === newRootId)!;

        // If the original had a parent, add as sibling
        if (targetNode.data.parentId) {
          const parent = prev.find((n) => n.id === targetNode.data.parentId);
          if (parent && parent.data.type === 'operator') {
            const opData = parent.data as OperatorNodeData;
            const newArgIndex = opData.childIds.length;

            // Update cloned root with new argIndex
            clonedRoot.data = {
              ...clonedRoot.data,
              argIndex: newArgIndex,
            };

            // Update parent's childIds
            const newNodes = prev.map((n) => {
              if (n.id === targetNode.data.parentId) {
                return {
                  ...n,
                  data: {
                    ...opData,
                    childIds: [...opData.childIds, newRootId],
                  },
                };
              }
              return n;
            });

            const result = [...newNodes, ...clonedNodes];
            hasEditedRef.current = true;
            onNodesChange?.(result);
            setSelectedNodeId(newRootId);
            setPanelValues({});
            return result;
          }
        }

        // If no parent or parent isn't operator, just add as new tree (replaces)
        clonedRoot.data = {
          ...clonedRoot.data,
          parentId: undefined,
          argIndex: undefined,
        };

        const result = clonedNodes;
        hasEditedRef.current = true;
        onNodesChange?.(result);
        setSelectedNodeId(newRootId);
        setPanelValues({});
        return result;
      });
    },
    [internalNodes, getDescendants, pushToUndoStack, onNodesChange]
  );

  // Select all descendants of a node
  const selectChildren = useCallback(
    (nodeId: string) => {
      const descendants = getDescendants(nodeId, internalNodes);
      const descendantIds = new Set(descendants.map((n) => n.id));
      // Include the node itself
      descendantIds.add(nodeId);
      setSelectedNodeIds(descendantIds);
      setSelectedNodeId(nodeId);
      setPanelValues({});
    },
    [internalNodes, getDescendants]
  );

  // Focus the properties panel on a specific node and optionally a field
  const focusPropertyPanel = useCallback(
    (nodeId: string, fieldId?: string) => {
      // Select the node first
      setSelectedNodeId(nodeId);
      setSelectedNodeIds(new Set([nodeId]));
      setPanelValues({});

      // Focus the field after a short delay to allow panel to render
      setTimeout(() => {
        propertyPanelFocusRef.current?.focusField(fieldId);
      }, 100);
    },
    []
  );

  // Apply current panel values to the selected node
  const applyPanelChanges = useCallback(() => {
    if (!selectedNode || Object.keys(panelValues).length === 0) {
      return;
    }

    const updatedData = panelValuesToNodeData(selectedNode.data, panelValues);

    // Only update if data actually changed
    if (JSON.stringify(selectedNode.data) !== JSON.stringify(updatedData)) {
      updateNode(selectedNode.id, updatedData);
    }
  }, [selectedNode, panelValues, updateNode]);

  const value = useMemo<EditorContextValue>(
    () => ({
      selectedNodeId: effectiveSelectedNodeId,
      selectedNodeIds: effectiveSelectedNodeIds,
      isEditMode,
      panelValues,
      selectedNode,
      selectedNodes,
      nodes,
      selectNode,
      setSelection,
      toggleNodeSelection,
      addToSelection,
      clearSelection,
      selectAllNodes,
      isNodeSelected,
      setEditMode,
      updatePanelValue,
      resetPanelValues,
      updateNode,
      deleteNode,
      applyPanelChanges,
      addArgumentToNode,
      removeArgumentFromNode,
      getChildNodes,
      createNode,
      hasNodes,
      insertNodeOnEdge,
      undo,
      redo,
      canUndo,
      canRedo,
      copyNode,
      pasteNode,
      canPaste,
      wrapNodeInOperator,
      duplicateNode,
      selectChildren,
      focusPropertyPanel,
      propertyPanelFocusRef,
    }),
    [
      effectiveSelectedNodeId,
      effectiveSelectedNodeIds,
      isEditMode,
      panelValues,
      selectedNode,
      selectedNodes,
      nodes,
      selectNode,
      setSelection,
      toggleNodeSelection,
      addToSelection,
      clearSelection,
      selectAllNodes,
      isNodeSelected,
      setEditMode,
      updatePanelValue,
      resetPanelValues,
      updateNode,
      deleteNode,
      applyPanelChanges,
      addArgumentToNode,
      removeArgumentFromNode,
      getChildNodes,
      createNode,
      hasNodes,
      insertNodeOnEdge,
      undo,
      redo,
      canUndo,
      canRedo,
      copyNode,
      pasteNode,
      canPaste,
      wrapNodeInOperator,
      duplicateNode,
      selectChildren,
      focusPropertyPanel,
    ]
  );

  return (
    <EditorContext.Provider value={value}>
      {children}
    </EditorContext.Provider>
  );
}
