/**
 * Editor Context Provider
 *
 * Provides state management for the visual editor including
 * node selection, edit mode, and panel field values.
 *
 * This context composes functionality from extracted services:
 * - HistoryService: undo/redo
 * - SelectionService: node selection (currently inlined for state access)
 * - ClipboardService: copy/paste
 * - NodeMutationService: pure functions for node mutations
 */

import { useState, useCallback, useMemo, useEffect, useRef, type ReactNode } from 'react';
import { v4 as uuidv4 } from 'uuid';
import type { LogicNode, LogicNodeData, OperatorNodeData, LiteralNodeData, VerticalCellNodeData, VariableNodeData } from '../../types';
import type { EditorContextValue, CreateNodeType } from './types';
import { EditorContext } from './context';
import { panelValuesToNodeData } from '../../utils/node-updaters';
import { deleteNodeAndDescendants } from '../../utils/node-deletion';
import { getOperator } from '../../config/operators';
import {
  cloneNodesWithIdMapping,
  getDescendants,
  updateParentChildReference,
} from '../../utils/node-cloning';
import {
  addArgument,
  removeArgument,
  wrapInOperator,
  duplicateNodeTree,
} from '../../services/node-mutation-service';

/** Clipboard data for copy/paste operations */
interface ClipboardData {
  nodes: LogicNode[];
  rootId: string;
}

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

  // Ref to track current nodes for undo/redo (avoids stale closures)
  const nodesRef = useRef<LogicNode[]>(propNodes);

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
      setInternalNodes(propNodes);
      hasEditedRef.current = false;
    }
  }, [propNodes]); // eslint-disable-line react-hooks/exhaustive-deps

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

  // Keep nodesRef in sync with internalNodes for undo/redo
  useEffect(() => {
    nodesRef.current = internalNodes;
  }, [internalNodes]);

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
    (nodeId: string, nodeType: 'literal' | 'variable' | 'operator' = 'literal', operatorName?: string) => {
      setInternalNodes((prev) => {
        // Save current state to undo stack
        pushToUndoStack(prev);

        // Use the pure function from node-mutation-service
        const result = addArgument(prev, nodeId, nodeType, operatorName);
        if (!result) return prev;

        hasEditedRef.current = true;
        onNodesChange?.(result.nodes);
        return result.nodes;
      });
    },
    [onNodesChange, pushToUndoStack]
  );

  // Remove an argument from an operator node
  const removeArgumentFromNode = useCallback(
    (nodeId: string, argIndex: number) => {
      setInternalNodes((prev) => {
        // Save current state to undo stack
        pushToUndoStack(prev);

        // Use the pure function from node-mutation-service
        const result = removeArgument(prev, nodeId, argIndex);
        if (!result) return prev;

        hasEditedRef.current = true;
        onNodesChange?.(result);
        return result;
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
    // Use nodesRef instead of stale internalNodes to avoid closure issues
    redoStackRef.current.push(JSON.parse(JSON.stringify(nodesRef.current)));
    setHistoryVersion((v) => v + 1);

    setInternalNodes(previousState);
    onNodesChange?.(previousState);

    // Clear selection to avoid stale references
    setSelectedNodeId(null);
    setSelectedNodeIds(new Set());
    setPanelValues({});
  }, [onNodesChange]);

  // Redo the last undone action
  const redo = useCallback(() => {
    if (redoStackRef.current.length === 0) return;

    const nextState = redoStackRef.current.pop()!;
    // Use nodesRef instead of stale internalNodes to avoid closure issues
    undoStackRef.current.push(JSON.parse(JSON.stringify(nodesRef.current)));
    setHistoryVersion((v) => v + 1);

    setInternalNodes(nextState);
    onNodesChange?.(nextState);

    // Clear selection to avoid stale references
    setSelectedNodeId(null);
    setSelectedNodeIds(new Set());
    setPanelValues({});
  }, [onNodesChange]);

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


  // Copy the selected node and its descendants to clipboard
  const copyNode = useCallback(() => {
    if (!selectedNode) return;

    // Get all descendants using the utility function
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
  }, [selectedNode, internalNodes]);

  // Paste clipboard contents
  const pasteNode = useCallback(() => {
    const clipboard = clipboardRef.current;
    if (!clipboard || clipboard.nodes.length === 0) return;

    setInternalNodes((prev) => {
      // Clone nodes with ID remapping using the shared utility
      const { nodes: clonedNodes, newRootId } = cloneNodesWithIdMapping(
        clipboard.nodes,
        clipboard.rootId
      );

      const clonedRoot = clonedNodes.find((n) => n.id === newRootId);

      // Defensive check: if cloning failed, return unchanged
      if (!clonedRoot || !newRootId) {
        console.warn('Paste failed: could not find cloned root node');
        return prev;
      }

      // Save current state to undo stack (only after validation)
      pushToUndoStack(prev);

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

          // Filter out removed nodes and update parent references
          let newNodes = prev.filter((n) => !targetIds.has(n.id));
          newNodes = updateParentChildReference(
            newNodes,
            targetNode.data.parentId,
            targetNode.id,
            newRootId
          );

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
  }, [selectedNode, pushToUndoStack, onNodesChange]);

  // Check if paste is available
  const canPaste = useMemo(
    () => clipboardRef.current !== null && clipboardRef.current.nodes.length > 0,
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [clipboardVersion]
  );

  // Wrap a node in an operator (makes the node a child of the new operator)
  const wrapNodeInOperatorFn = useCallback(
    (nodeId: string, operator: string) => {
      setInternalNodes((prev) => {
        // Save current state to undo stack
        pushToUndoStack(prev);

        // Use the pure function from node-mutation-service
        const result = wrapInOperator(prev, nodeId, operator);
        if (!result) return prev;

        // Find the new wrapper node (last node in the result)
        const wrapperNode = result[result.length - 1];
        const newOperatorId = wrapperNode.id;

        hasEditedRef.current = true;
        onNodesChange?.(result);
        setSelectedNodeId(newOperatorId);
        setPanelValues({});
        return result;
      });
    },
    [onNodesChange, pushToUndoStack]
  );

  // Duplicate a node and its descendants
  const duplicateNodeFn = useCallback(
    (nodeId: string) => {
      setInternalNodes((prev) => {
        // Save current state to undo stack
        pushToUndoStack(prev);

        // Use the pure function from node-mutation-service
        const result = duplicateNodeTree(prev, nodeId);
        if (!result) return prev;

        hasEditedRef.current = true;
        onNodesChange?.(result.nodes);
        setSelectedNodeId(result.newRootId);
        setPanelValues({});
        return result.nodes;
      });
    },
    [pushToUndoStack, onNodesChange]
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
    [internalNodes]
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
      wrapNodeInOperator: wrapNodeInOperatorFn,
      duplicateNode: duplicateNodeFn,
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
      wrapNodeInOperatorFn,
      duplicateNodeFn,
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
