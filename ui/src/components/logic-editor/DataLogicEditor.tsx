import { useEffect, useMemo, useCallback, useRef, useState } from 'react';
import {
  ReactFlow,
  Background,
  Controls,
  useNodesState,
  useEdgesState,
  useReactFlow,
  ReactFlowProvider,
  type NodeMouseHandler,
} from '@xyflow/react';
import '@xyflow/react/dist/style.css';

import type { DataLogicEditorProps, LogicNode, LogicEdge } from './types';
import { nodeTypes } from './nodes';
import { edgeTypes } from './edges';
import { useLogicEditor, useWasmEvaluator, useDebugEvaluation, type EvaluationResultsMap } from './hooks';
import { getHiddenNodeIds } from './utils/visibility';
import { buildEdgesFromNodes } from './utils/edge-builder';
import { nodesToJsonLogic } from './utils/nodes-to-jsonlogic';
import { EvaluationContext, DebuggerProvider, ConnectedHandlesProvider, EditorProvider } from './context';
import { useEditorContext } from './context/editor';
import { DebuggerControls } from './debugger-controls';
import { PropertiesPanel } from './properties-panel';
import { NodeSelectionHandler } from './NodeSelectionHandler';
import { KeyboardHandler } from './KeyboardHandler';
import { UndoRedoToolbar } from './UndoRedoToolbar';
import { NodeContextMenu, CanvasContextMenu } from './context-menu';
import { REACT_FLOW_OPTIONS } from './constants/layout';
import { useSystemTheme } from '../../hooks';
import './styles/nodes.css';
import './LogicEditor.css';
import './properties-panel/properties-panel.css';
import './panel-inputs/panel-inputs.css';
import './edges/edges.css';

const emptyResults: EvaluationResultsMap = new Map();

// Helper component to auto-fit view (must be inside ReactFlow)
function AutoFitView({ nodeCount }: { nodeCount: number }) {
  const { fitView } = useReactFlow();

  useEffect(() => {
    const timer = setTimeout(() => {
      fitView({
        padding: REACT_FLOW_OPTIONS.fitViewPadding,
        maxZoom: REACT_FLOW_OPTIONS.maxZoom,
      });
    }, 50);
    return () => clearTimeout(timer);
  }, [nodeCount, fitView]);

  return null;
}

// Context menu state type
interface ContextMenuState {
  type: 'node' | 'canvas';
  x: number;
  y: number;
  nodeId?: string;
}

// Inner component that handles ReactFlow state
function DataLogicEditorInner({
  initialNodes,
  initialEdges,
  readOnly,
  evaluationResults,
  theme,
  showDebugger,
  isEditMode,
}: {
  initialNodes: LogicNode[];
  initialEdges: LogicEdge[];
  readOnly: boolean;
  evaluationResults: EvaluationResultsMap;
  theme: 'light' | 'dark';
  showDebugger: boolean;
  isEditMode: boolean;
}) {
  // Background dot colors based on theme
  const bgColor = theme === 'dark' ? '#404040' : '#cccccc';

  // Context menu state
  const [contextMenu, setContextMenu] = useState<ContextMenuState | null>(null);

  // Get editor context for double-click handling
  const { focusPropertyPanel, nodes: editorNodes } = useEditorContext();

  // Initialize state directly from props - component remounts via key when expression changes
  const [nodes, setNodes, onNodesChange] = useNodesState<LogicNode>(initialNodes);
  // Note: We don't use edges state directly - edges are rebuilt from nodes
  const [, , onEdgesChange] = useEdgesState<LogicEdge>(initialEdges);

  // Sync state when props change (handles cases where key doesn't trigger remount)
  useEffect(() => {
    setNodes(initialNodes);
  }, [initialNodes, setNodes]);

  // Track previous node IDs to detect structural changes
  const prevNodeIdsRef = useRef<Set<string>>(new Set(initialNodes.map((n) => n.id)));

  // Sync ReactFlow state with EditorContext nodes only on structural changes (add/delete)
  // This prevents focus loss when editing node data (like literal values)
  useEffect(() => {
    if (!isEditMode) return;

    const currentIds = new Set(editorNodes.map((n) => n.id));
    const prevIds = prevNodeIdsRef.current;

    // Check if node structure changed (different IDs or count)
    const structureChanged =
      currentIds.size !== prevIds.size ||
      [...currentIds].some((id) => !prevIds.has(id)) ||
      [...prevIds].some((id) => !currentIds.has(id));

    if (structureChanged) {
      setNodes(editorNodes);
      prevNodeIdsRef.current = currentIds;
    }
  }, [editorNodes, isEditMode, setNodes]);

  // Compute hidden node IDs based on collapsed state
  const hiddenNodeIds = useMemo(() => getHiddenNodeIds(nodes), [nodes]);

  // Build set of all node IDs for edge validation
  const nodeIds = useMemo(() => new Set(nodes.map((n) => n.id)), [nodes]);

  // Filter visible nodes (exclude hidden descendants of collapsed nodes)
  const visibleNodes = useMemo(
    () => nodes.filter((node) => !hiddenNodeIds.has(node.id)),
    [nodes, hiddenNodeIds]
  );

  // Rebuild edges from current node state - this respects collapse state
  // Edges are rebuilt whenever nodes change (including collapse state changes)
  const currentEdges = useMemo(() => buildEdgesFromNodes(nodes), [nodes]);

  // Filter visible edges (exclude edges connected to hidden or non-existent nodes)
  // In edit mode, use the editable edge type for interactive [+] buttons
  const visibleEdges = useMemo(
    () =>
      currentEdges
        .filter(
          (edge) =>
            nodeIds.has(edge.source) &&
            nodeIds.has(edge.target) &&
            !hiddenNodeIds.has(edge.source) &&
            !hiddenNodeIds.has(edge.target)
        )
        .map((edge) =>
          isEditMode ? { ...edge, type: 'editable' } : edge
        ),
    [currentEdges, nodeIds, hiddenNodeIds, isEditMode]
  );

  // Handle node context menu (right-click)
  const handleNodeContextMenu: NodeMouseHandler<LogicNode> = useCallback(
    (event, node) => {
      if (!isEditMode) return;
      event.preventDefault();
      setContextMenu({
        type: 'node',
        x: event.clientX,
        y: event.clientY,
        nodeId: node.id,
      });
    },
    [isEditMode]
  );

  // Handle pane (canvas) context menu
  const handlePaneContextMenu = useCallback(
    (event: React.MouseEvent | MouseEvent) => {
      if (!isEditMode) return;
      event.preventDefault();
      setContextMenu({
        type: 'canvas',
        x: event.clientX,
        y: event.clientY,
      });
    },
    [isEditMode]
  );

  // Handle node double-click
  const handleNodeDoubleClick: NodeMouseHandler<LogicNode> = useCallback(
    (_event, node) => {
      if (!isEditMode) return;

      // Determine which field to focus based on node type
      let fieldId: string | undefined;
      switch (node.data.type) {
        case 'literal':
          fieldId = 'value';
          break;
        case 'variable':
          fieldId = 'path';
          break;
        default:
          fieldId = undefined;
      }

      focusPropertyPanel(node.id, fieldId);
    },
    [isEditMode, focusPropertyPanel]
  );

  // Close context menu
  const handleCloseContextMenu = useCallback(() => {
    setContextMenu(null);
  }, []);

  // Handle "Edit Properties" from context menu
  const handleEditProperties = useCallback(() => {
    if (contextMenu?.nodeId) {
      const node = editorNodes.find((n) => n.id === contextMenu.nodeId);
      if (node) {
        let fieldId: string | undefined;
        switch (node.data.type) {
          case 'literal':
            fieldId = 'value';
            break;
          case 'variable':
            fieldId = 'path';
            break;
          default:
            fieldId = undefined;
        }
        focusPropertyPanel(contextMenu.nodeId, fieldId);
      }
    }
    handleCloseContextMenu();
  }, [contextMenu, editorNodes, focusPropertyPanel, handleCloseContextMenu]);

  // Get the node for context menu
  const contextMenuNode = useMemo(() => {
    if (contextMenu?.type === 'node' && contextMenu.nodeId) {
      return editorNodes.find((n) => n.id === contextMenu.nodeId);
    }
    return undefined;
  }, [contextMenu, editorNodes]);

  return (
    <EvaluationContext.Provider value={evaluationResults}>
      <ConnectedHandlesProvider edges={visibleEdges}>
        <ReactFlowProvider>
          <ReactFlow
            nodes={visibleNodes}
            edges={visibleEdges}
            onNodesChange={readOnly ? undefined : onNodesChange}
            onEdgesChange={readOnly ? undefined : onEdgesChange}
            nodeTypes={nodeTypes}
            edgeTypes={edgeTypes}
            fitView
            fitViewOptions={{
              padding: REACT_FLOW_OPTIONS.fitViewPadding,
              maxZoom: REACT_FLOW_OPTIONS.maxZoom,
            }}
            minZoom={0.1}
            maxZoom={2}
            defaultEdgeOptions={{
              type: 'default',
              animated: false,
            }}
            onNodeContextMenu={handleNodeContextMenu}
            onPaneContextMenu={handlePaneContextMenu}
            onNodeDoubleClick={handleNodeDoubleClick}
          >
            <Background color={bgColor} gap={20} size={1} />
            <Controls showInteractive={!readOnly} />
            {showDebugger && <DebuggerControls />}
            <NodeSelectionHandler />
            <AutoFitView nodeCount={initialNodes.length} />

            {/* Context Menus - must be inside ReactFlowProvider for useReactFlow hook */}
            {contextMenu?.type === 'node' && contextMenuNode && (
              <NodeContextMenu
                x={contextMenu.x}
                y={contextMenu.y}
                node={contextMenuNode}
                onClose={handleCloseContextMenu}
                onEditProperties={handleEditProperties}
              />
            )}
            {contextMenu?.type === 'canvas' && (
              <CanvasContextMenu
                x={contextMenu.x}
                y={contextMenu.y}
                onClose={handleCloseContextMenu}
              />
            )}
          </ReactFlow>
        </ReactFlowProvider>

        {visibleNodes.length === 0 && (
          <div className="logic-editor-empty">
            <p>No expression</p>
            <p className="logic-editor-empty-hint">
              Enter valid JSONLogic in the input panel to visualize it
            </p>
          </div>
        )}
      </ConnectedHandlesProvider>
    </EvaluationContext.Provider>
  );
}

export function DataLogicEditor({
  value,
  onChange,
  data,
  mode = 'visualize',
  theme: themeProp,
  className = '',
  preserveStructure = false,
  componentMode: _componentMode = 'debugger',
}: DataLogicEditorProps) {
  void _componentMode; // Component mode is handled by parent components (App.tsx, embed.tsx)

  // Debounce timer ref for onChange
  const onChangeTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Determine if we're in edit mode
  const isEditMode = mode === 'edit';

  // Theme handling - use prop override or system preference
  const systemTheme = useSystemTheme();
  const resolvedTheme = themeProp ?? systemTheme;

  // Internal WASM evaluator
  const {
    ready: wasmReady,
    evaluate,
    evaluateWithTrace,
  } = useWasmEvaluator({ preserveStructure });

  // Determine if debugging is enabled
  const debugEnabled = mode === 'debug' && data !== undefined;

  // Use trace-based evaluation when in debug mode
  const editor = useLogicEditor({
    value,
    evaluateWithTrace: debugEnabled && wasmReady ? evaluateWithTrace : undefined,
    data: debugEnabled ? data : undefined,
    preserveStructure,
  });

  // Use a combination of node count, edge count, and root node ID as key
  // This ensures the component remounts when the expression structure changes
  const expressionKey = `${editor.nodes.length}-${editor.edges.length}-${editor.nodes[0]?.id ?? 'empty'}`;

  // Fallback: Compute evaluation results using multiple evaluate calls (only when not in trace mode)
  const fallbackResults = useDebugEvaluation({
    nodes: editor.nodes,
    data,
    evaluate: wasmReady ? evaluate : null,
    enabled: debugEnabled && !editor.usingTraceMode && wasmReady,
  });

  // Use trace-based results when available, otherwise fall back to multi-evaluation
  const results = editor.usingTraceMode
    ? editor.evaluationResults
    : fallbackResults.size > 0
      ? fallbackResults
      : emptyResults;

  // Check if debugger should be active (trace mode with steps)
  const showDebugger = debugEnabled && editor.usingTraceMode && editor.steps.length > 0;

  // Allow node interactions (collapse/expand) in all modes
  // Full editing (adding/removing nodes) is still a future feature
  const readOnly = false;

  // Handle nodes change from editor context - convert to JSONLogic and call onChange
  const handleNodesChange = useCallback(
    (nodes: LogicNode[]) => {
      if (!onChange) return;

      // Clear any pending timer
      if (onChangeTimerRef.current) {
        clearTimeout(onChangeTimerRef.current);
      }

      // Debounce the onChange call (300ms)
      onChangeTimerRef.current = setTimeout(() => {
        const newExpr = nodesToJsonLogic(nodes);
        onChange(newExpr);
        onChangeTimerRef.current = null;
      }, 300);
    },
    [onChange]
  );

  // Cleanup timer on unmount
  useEffect(() => {
    return () => {
      if (onChangeTimerRef.current) {
        clearTimeout(onChangeTimerRef.current);
      }
    };
  }, []);

  // Handle error state
  if (editor.error) {
    return (
      <div className={`logic-editor ${className}`} data-theme={resolvedTheme}>
        <div className="logic-editor-error">
          <p className="logic-editor-error-title">Error rendering expression</p>
          <p className="logic-editor-error-message">{editor.error}</p>
        </div>
      </div>
    );
  }

  const editorInner = (
    <DataLogicEditorInner
      key={expressionKey}
      initialNodes={editor.nodes}
      initialEdges={editor.edges}
      readOnly={readOnly}
      evaluationResults={results}
      theme={resolvedTheme}
      showDebugger={showDebugger}
      isEditMode={isEditMode}
    />
  );

  // Build the class name with edit mode modifier
  const editorClassName = [
    'logic-editor',
    isEditMode ? 'logic-editor--with-panel' : '',
    className,
  ].filter(Boolean).join(' ');

  return (
    <EditorProvider
      nodes={editor.nodes}
      initialEditMode={isEditMode}
      onNodesChange={handleNodesChange}
    >
      {isEditMode && <KeyboardHandler />}
      <div className={editorClassName} data-theme={resolvedTheme}>
        {isEditMode && <UndoRedoToolbar />}
        <div className="logic-editor-main">
          {showDebugger ? (
            <DebuggerProvider
              steps={editor.steps}
              traceNodeMap={editor.traceNodeMap}
              nodes={editor.nodes}
            >
              {editorInner}
            </DebuggerProvider>
          ) : (
            editorInner
          )}
        </div>
        {isEditMode && <PropertiesPanel />}
      </div>
    </EditorProvider>
  );
}

export default DataLogicEditor;
