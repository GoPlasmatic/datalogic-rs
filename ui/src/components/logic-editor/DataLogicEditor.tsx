import { useEffect, useMemo, useCallback, useRef } from 'react';
import {
  ReactFlow,
  Background,
  Controls,
  useNodesState,
  useEdgesState,
  ReactFlowProvider,
} from '@xyflow/react';
import '@xyflow/react/dist/style.css';

import type { DataLogicEditorProps, LogicNode, LogicEdge } from './types';
import { nodeTypes } from './nodes';
import { edgeTypes } from './edges';
import { useLogicEditor, useWasmEvaluator, type EvaluationResultsMap } from './hooks';
import { useContextMenu } from './hooks/useContextMenu';
import { getHiddenNodeIds } from './utils/visibility';
import { buildEdgesFromNodes } from './utils/edge-builder';
import { nodesToJsonLogic } from './utils/nodes-to-jsonlogic';
import { EvaluationContext, DebuggerProvider, ConnectedHandlesProvider, EditorProvider } from './context';
import { useEditorContext } from './context/editor';
import { DebuggerControls } from './debugger-controls';
import { PropertiesPanel } from './properties-panel';
import { NodeSelectionHandler } from './NodeSelectionHandler';
import { KeyboardHandler } from './KeyboardHandler';
import { NodeContextMenu, CanvasContextMenu } from './context-menu';
import { AutoFitView } from './AutoFitView';
import { EditorToolbar } from './EditorToolbar';
import { REACT_FLOW_OPTIONS } from './constants/layout';
import { useSystemTheme } from '../../hooks';
import './styles/nodes.css';
import './LogicEditor.css';
import './properties-panel/properties-panel.css';
import './panel-inputs/panel-inputs.css';
import './edges/edges.css';

const emptyResults: EvaluationResultsMap = new Map();

/**
 * Read-only inner component - minimal, no EditorContext dependency.
 * Used when editable=false to avoid EditorProvider's state sync effects.
 */
function ReadOnlyEditorInner({
  initialNodes,
  initialEdges,
  theme,
  showDebugger,
}: {
  initialNodes: LogicNode[];
  initialEdges: LogicEdge[];
  theme: 'light' | 'dark';
  showDebugger: boolean;
}) {
  const bgColor = theme === 'dark' ? '#404040' : '#cccccc';

  const [nodes, , onNodesChange] = useNodesState<LogicNode>(initialNodes);
  const [, , onEdgesChange] = useEdgesState<LogicEdge>(initialEdges);

  // Compute hidden node IDs based on collapsed state
  const hiddenNodeIds = useMemo(() => getHiddenNodeIds(nodes), [nodes]);
  const nodeIds = useMemo(() => new Set(nodes.map((n) => n.id)), [nodes]);

  const visibleNodes = useMemo(
    () => nodes.filter((node) => !hiddenNodeIds.has(node.id)),
    [nodes, hiddenNodeIds]
  );

  const currentEdges = useMemo(() => buildEdgesFromNodes(nodes), [nodes]);

  const visibleEdges = useMemo(
    () =>
      currentEdges.filter(
        (edge) =>
          nodeIds.has(edge.source) &&
          nodeIds.has(edge.target) &&
          !hiddenNodeIds.has(edge.source) &&
          !hiddenNodeIds.has(edge.target)
      ),
    [currentEdges, nodeIds, hiddenNodeIds]
  );

  return (
    <EvaluationContext.Provider value={emptyResults}>
      <ConnectedHandlesProvider edges={visibleEdges}>
        <ReactFlowProvider>
          <ReactFlow
            nodes={visibleNodes}
            edges={visibleEdges}
            onNodesChange={onNodesChange}
            onEdgesChange={onEdgesChange}
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
          >
            <Background color={bgColor} gap={20} size={1} />
            <Controls showInteractive={false} />
            {showDebugger && <DebuggerControls />}
            <AutoFitView nodeCount={initialNodes.length} />
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

/**
 * Editable inner component - full EditorContext support with syncing.
 * Used when editable=true.
 */
function EditableEditorInner({
  initialNodes,
  initialEdges,
  evaluationResults,
  theme,
  showDebugger,
}: {
  initialNodes: LogicNode[];
  initialEdges: LogicEdge[];
  evaluationResults: EvaluationResultsMap;
  theme: 'light' | 'dark';
  showDebugger: boolean;
}) {
  // Background dot colors based on theme
  const bgColor = theme === 'dark' ? '#404040' : '#cccccc';

  // Context menu hook
  const {
    contextMenu,
    handleNodeContextMenu,
    handlePaneContextMenu,
    handleNodeDoubleClick,
    handleCloseContextMenu,
    handleEditProperties,
    contextMenuNode,
  } = useContextMenu(true);

  // Get editor context for syncing
  const { nodes: editorNodes } = useEditorContext();

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
  useEffect(() => {
    const currentIds = new Set(editorNodes.map((n) => n.id));
    const prevIds = prevNodeIdsRef.current;

    const structureChanged =
      currentIds.size !== prevIds.size ||
      [...currentIds].some((id) => !prevIds.has(id)) ||
      [...prevIds].some((id) => !currentIds.has(id));

    if (structureChanged) {
      setNodes(editorNodes);
      prevNodeIdsRef.current = currentIds;
    }
  }, [editorNodes, setNodes]);

  // Compute hidden node IDs based on collapsed state
  const hiddenNodeIds = useMemo(() => getHiddenNodeIds(nodes), [nodes]);
  const nodeIds = useMemo(() => new Set(nodes.map((n) => n.id)), [nodes]);

  const visibleNodes = useMemo(
    () => nodes.filter((node) => !hiddenNodeIds.has(node.id)),
    [nodes, hiddenNodeIds]
  );

  const currentEdges = useMemo(() => buildEdgesFromNodes(nodes), [nodes]);

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
        .map((edge) => ({ ...edge, type: 'editable' })),
    [currentEdges, nodeIds, hiddenNodeIds]
  );

  return (
    <EvaluationContext.Provider value={evaluationResults}>
      <ConnectedHandlesProvider edges={visibleEdges}>
        <ReactFlowProvider>
          <ReactFlow
            nodes={visibleNodes}
            edges={visibleEdges}
            onNodesChange={onNodesChange}
            onEdgesChange={onEdgesChange}
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
            <Controls showInteractive />
            {showDebugger && <DebuggerControls />}
            <NodeSelectionHandler />
            <AutoFitView nodeCount={initialNodes.length} />

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
  theme: themeProp,
  className = '',
  preserveStructure = false,
  onPreserveStructureChange,
  editable = false,
}: DataLogicEditorProps) {
  // Debounce timer ref for onChange
  const onChangeTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Determine if we're in edit mode
  const isEditMode = editable;

  // Theme handling - use prop override or system preference
  const systemTheme = useSystemTheme();
  const resolvedTheme = themeProp ?? systemTheme;

  // Internal WASM evaluator
  const {
    ready: wasmReady,
    evaluateWithTrace,
  } = useWasmEvaluator({ preserveStructure });

  // Evaluation is enabled whenever data is provided (unified mode - no mode switching needed)
  const evalEnabled = data !== undefined;

  // Use trace-based evaluation when data is available
  const editor = useLogicEditor({
    value,
    evaluateWithTrace: evalEnabled && wasmReady ? evaluateWithTrace : undefined,
    data: evalEnabled ? data : undefined,
    preserveStructure,
  });

  // Use a combination of node count, edge count, and root node ID as key
  // This ensures the component remounts when the expression structure changes
  const expressionKey = `${editor.nodes.length}-${editor.edges.length}-${editor.nodes[0]?.id ?? 'empty'}`;

  // Check if debugger should be active (trace mode with steps)
  const hasDebugger = evalEnabled && editor.usingTraceMode && editor.steps.length > 0;

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

  // Build the class name
  const editorClassName = ['logic-editor', className].filter(Boolean).join(' ');

  // --- Read-only mode: skip EditorProvider entirely ---
  if (!isEditMode) {
    const readOnlyInner = (
      <ReadOnlyEditorInner
        key={expressionKey}
        initialNodes={editor.nodes}
        initialEdges={editor.edges}
        theme={resolvedTheme}
        showDebugger={false}
      />
    );

    return (
      <div className={editorClassName} data-theme={resolvedTheme}>
        {hasDebugger ? (
          <DebuggerProvider
            steps={editor.steps}
            traceNodeMap={editor.traceNodeMap}
            nodes={editor.nodes}
          >
            <EditorToolbar
              isEditMode={false}
              hasDebugger={hasDebugger}
              preserveStructure={preserveStructure}
              onPreserveStructureChange={onPreserveStructureChange}
            />
            <div className="logic-editor-body">
              <div className="logic-editor-main">
                {readOnlyInner}
              </div>
            </div>
          </DebuggerProvider>
        ) : (
          <>
            <EditorToolbar
              isEditMode={false}
              hasDebugger={hasDebugger}
              preserveStructure={preserveStructure}
              onPreserveStructureChange={onPreserveStructureChange}
            />
            <div className="logic-editor-body">
              <div className="logic-editor-main">
                {readOnlyInner}
              </div>
            </div>
          </>
        )}
      </div>
    );
  }

  // --- Edit mode: full EditorProvider with all features ---
  const editableInner = (
    <EditableEditorInner
      key={expressionKey}
      initialNodes={editor.nodes}
      initialEdges={editor.edges}
      evaluationResults={emptyResults}
      theme={resolvedTheme}
      showDebugger={false}
    />
  );

  return (
    <EditorProvider
      nodes={editor.nodes}
      initialEditMode={isEditMode}
      onNodesChange={handleNodesChange}
    >
      <KeyboardHandler />
      <div className={editorClassName} data-theme={resolvedTheme}>
        {hasDebugger ? (
          <DebuggerProvider
            steps={editor.steps}
            traceNodeMap={editor.traceNodeMap}
            nodes={editor.nodes}
          >
            <EditorToolbar
              isEditMode={isEditMode}
              hasDebugger={hasDebugger}
              preserveStructure={preserveStructure}
              onPreserveStructureChange={onPreserveStructureChange}
            />
            <div className="logic-editor-body">
              <div className="logic-editor-main">
                {editableInner}
              </div>
              <PropertiesPanel />
            </div>
          </DebuggerProvider>
        ) : (
          <>
            <EditorToolbar
              isEditMode={isEditMode}
              hasDebugger={hasDebugger}
              preserveStructure={preserveStructure}
              onPreserveStructureChange={onPreserveStructureChange}
            />
            <div className="logic-editor-body">
              <div className="logic-editor-main">
                {editableInner}
              </div>
              <PropertiesPanel />
            </div>
          </>
        )}
      </div>
    </EditorProvider>
  );
}

export default DataLogicEditor;
