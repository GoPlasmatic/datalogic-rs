/**
 * Editor Context Types
 *
 * Types for the visual editor state management.
 */

import type { LogicNode, LogicNodeData } from '../../types';

/**
 * Editor state
 */
export interface EditorState {
  /** Currently selected node ID (primary selection) */
  selectedNodeId: string | null;
  /** All selected node IDs (for multi-select) */
  selectedNodeIds: Set<string>;
  /** Whether edit mode is active */
  isEditMode: boolean;
  /** Current field values for the selected node's panel */
  panelValues: Record<string, unknown>;
}

/** Node creation types */
export type CreateNodeType = 'variable' | 'operator' | 'literal' | 'condition';

/** Node types that can be added as an argument */
export type AddArgumentNodeType = 'literal' | 'variable' | 'operator';

/**
 * Clipboard data for copy/paste
 */
export interface ClipboardData {
  /** The copied nodes (root node and all descendants) */
  nodes: LogicNode[];
  /** The ID of the root copied node */
  rootId: string;
}

/**
 * Editor actions
 */
export interface EditorActions {
  /** Select a node by ID (clears multi-selection) */
  selectNode: (nodeId: string | null) => void;
  /** Set selection from ReactFlow (supports multi-select) */
  setSelection: (nodeIds: string[]) => void;
  /** Toggle a node in multi-selection (Cmd/Ctrl+Click) */
  toggleNodeSelection: (nodeId: string) => void;
  /** Add a node to multi-selection (Shift+Click) */
  addToSelection: (nodeId: string) => void;
  /** Clear all selections */
  clearSelection: () => void;
  /** Select all nodes */
  selectAllNodes: () => void;
  /** Check if a node is selected */
  isNodeSelected: (nodeId: string) => boolean;
  /** Toggle edit mode */
  setEditMode: (enabled: boolean) => void;
  /** Update a panel field value */
  updatePanelValue: (fieldId: string, value: unknown) => void;
  /** Reset panel values (when selection changes) */
  resetPanelValues: (values?: Record<string, unknown>) => void;
  /** Update a node's data */
  updateNode: (nodeId: string, newData: Partial<LogicNodeData>) => void;
  /** Delete a node and its descendants */
  deleteNode: (nodeId: string) => void;
  /** Apply current panel values to the selected node */
  applyPanelChanges: () => void;
  /** Add a new argument to an N-ary operator node */
  addArgumentToNode: (nodeId: string, nodeType?: AddArgumentNodeType, operatorName?: string) => void;
  /** Remove an argument from an operator node by index */
  removeArgumentFromNode: (nodeId: string, argIndex: number) => void;
  /** Get child nodes for a given parent node */
  getChildNodes: (parentId: string) => LogicNode[];
  /** Create a new node (as root if canvas is empty, or wrap existing root) */
  createNode: (type: CreateNodeType, operatorName?: string) => void;
  /** Check if canvas has any nodes */
  hasNodes: () => boolean;
  /** Insert a new node on an edge (between source and target) */
  insertNodeOnEdge: (
    sourceId: string,
    targetId: string,
    operatorName: string
  ) => void;
  /** Undo the last action */
  undo: () => void;
  /** Redo the last undone action */
  redo: () => void;
  /** Check if undo is available */
  canUndo: boolean;
  /** Check if redo is available */
  canRedo: boolean;
  /** Copy the selected node and its descendants to clipboard */
  copyNode: () => void;
  /** Paste clipboard contents, replacing selected node or as new root */
  pasteNode: () => void;
  /** Check if paste is available */
  canPaste: boolean;
  /** Wrap a node in an operator (makes the node a child of the new operator) */
  wrapNodeInOperator: (nodeId: string, operator: string) => void;
  /** Duplicate a node and its descendants */
  duplicateNode: (nodeId: string) => void;
  /** Select all descendants of a node */
  selectChildren: (nodeId: string) => void;
  /** Focus the properties panel on a specific node and optionally a field */
  focusPropertyPanel: (nodeId: string, fieldId?: string) => void;
  /** Ref for focusing property panel fields */
  propertyPanelFocusRef: React.RefObject<{ focusField: (fieldId?: string) => void } | null>;
}

/**
 * Full editor context value
 */
export interface EditorContextValue extends EditorState, EditorActions {
  /** The currently selected node (computed from selectedNodeId) */
  selectedNode: LogicNode | null;
  /** All selected nodes (computed from selectedNodeIds) */
  selectedNodes: LogicNode[];
  /** All nodes in the editor (internal state, may differ from props during editing) */
  nodes: LogicNode[];
}
