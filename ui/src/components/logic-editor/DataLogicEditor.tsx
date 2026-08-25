import { useEffect, useMemo, useCallback, useRef, useState } from 'react';
import {
  ReactFlow,
  Background,
  Controls,
  useNodesState,
  useEdgesState,
  ReactFlowProvider,
  MarkerType,
} from '@xyflow/react';
import { Workflow } from 'lucide-react';
import './styles/reactflow-base.css';

import type {
  DataLogicEditorProps,
  LogicNode,
  LogicEdge,
  TracedResult,
} from './types';
import { nodeTypes } from './nodes';
import { edgeTypes } from './edges';
import { useLogicEditor, useWasmEvaluator, customOperatorNamesKey } from './hooks';
import { normalizeEvaluationConfig, summarizeEvaluationConfig } from './hooks/useWasmEvaluator';
import { useContextMenu } from './hooks/useContextMenu';
import { getHiddenNodeIds } from './utils/visibility';
import { buildEdgesFromNodes } from './utils/edge-builder';
import { nodesToJsonLogic } from './utils/nodes-to-jsonlogic';
import { formatTraceFailure, traceFailureType, type TraceFailure } from './utils/trace';
import { DebuggerProvider, ConnectedHandlesProvider, EditorProvider, DirectionContext, useDirection, type FlowDirection } from './context';
import { useEditorContext } from './context/editor';
import { DebuggerControls } from './debugger-controls';
import { PropertiesPanel } from './properties-panel';
import { NodeSelectionHandler } from './NodeSelectionHandler';
import { KeyboardHandler } from './KeyboardHandler';
import { NodeContextMenu, CanvasContextMenu } from './context-menu';
import { AutoFitView } from './AutoFitView';
import { EditorToolbar } from './EditorToolbar';
import { REACT_FLOW_OPTIONS } from './constants/layout';
import { useSystemTheme } from './hooks/useSystemTheme';
import './styles/nodes.css';
import './LogicEditor.css';

// Producer(child)->consumer(parent) edges: the arrowhead sits at the target end
// and points right, toward the result. Shared by the read-only and editable canvases.
const DEFAULT_EDGE_MARKER = {
  type: MarkerType.ArrowClosed,
  width: 16,
  height: 16,
  color: '#8098b0',
} as const;

function EmptyState({
  exampleSuggestions,
  onSelectExample,
  configSummary,
}: {
  exampleSuggestions?: string[];
  onSelectExample?: (name: string) => void;
  configSummary?: string | null;
}) {
  const chips = exampleSuggestions && onSelectExample ? exampleSuggestions : [];
  return (
    <div className="logic-editor-empty">
      <div className="logic-editor-empty-icon">
        <Workflow size={28} strokeWidth={1.5} />
      </div>
      <p>No expression</p>
      <p className="logic-editor-empty-hint">
        Enter valid JSONLogic in the input panel to visualize it.
      </p>
      {chips.length > 0 && (
        <div className="logic-editor-empty-chips">
          <span className="logic-editor-empty-chips-label">Try</span>
          {chips.map((name) => (
            <button
              key={name}
              type="button"
              className="logic-editor-empty-chip"
              onClick={() => onSelectExample?.(name)}
            >
              {name}
            </button>
          ))}
        </div>
      )}
      {configSummary && (
        <p className="logic-editor-empty-config">Engine settings: {configSummary}</p>
      )}
    </div>
  );
}

/**
 * Banner for a trace-level failure the debugger cannot show on a node:
 * a compile-stage error (no steps, no expression tree) or a runtime error
 * with no resolvable breadcrumb. Runtime failures that do map onto a node
 * are shown there by the debugger instead.
 */
function TraceErrorBanner({ failure }: { failure: TraceFailure }) {
  const kind = traceFailureType(failure);
  return (
    <div className="logic-editor-trace-error" role="alert">
      {kind && <span className="logic-editor-trace-error-kind">{kind}</span>}
      <span className="logic-editor-trace-error-message">{formatTraceFailure(failure)}</span>
    </div>
  );
}

/**
 * Read-only inner component - minimal, no EditorContext dependency.
 * Used when editable=false to avoid EditorProvider's state sync effects.
 */
function ReadOnlyEditorInner({
  initialNodes,
  initialEdges,
  theme,
  showDebugger,
  exampleSuggestions,
  onSelectExample,
  configSummary,
}: {
  initialNodes: LogicNode[];
  initialEdges: LogicEdge[];
  theme: 'light' | 'dark';
  showDebugger: boolean;
  exampleSuggestions?: string[];
  onSelectExample?: (name: string) => void;
  configSummary?: string | null;
}) {
  const bgColor = theme === 'dark' ? '#404040' : '#cccccc';
  const direction = useDirection();

  const [nodes, , onNodesChange] = useNodesState<LogicNode>(initialNodes);
  const [, , onEdgesChange] = useEdgesState<LogicEdge>(initialEdges);

  // Compute hidden node IDs based on collapsed state
  const hiddenNodeIds = useMemo(() => getHiddenNodeIds(nodes), [nodes]);
  const nodeIds = useMemo(() => new Set(nodes.map((n) => n.id)), [nodes]);

  const visibleNodes = useMemo(
    () => nodes.filter((node) => !hiddenNodeIds.has(node.id)),
    [nodes, hiddenNodeIds]
  );

  const currentEdges = useMemo(() => buildEdgesFromNodes(nodes, direction), [nodes, direction]);

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
              markerEnd: DEFAULT_EDGE_MARKER,
            }}
          >
            <Background color={bgColor} gap={20} size={1} />
            <Controls showInteractive={false} />
            {showDebugger && <DebuggerControls />}
            <AutoFitView nodeCount={initialNodes.length} />
          </ReactFlow>
        </ReactFlowProvider>

        {visibleNodes.length === 0 && (
          <EmptyState
            exampleSuggestions={exampleSuggestions}
            onSelectExample={onSelectExample}
            configSummary={configSummary}
          />
        )}
    </ConnectedHandlesProvider>
  );
}

/**
 * Editable inner component - full EditorContext support with syncing.
 * Used when editable=true.
 */
function EditableEditorInner({
  initialNodes,
  initialEdges,
  theme,
  showDebugger,
  exampleSuggestions,
  onSelectExample,
  configSummary,
}: {
  initialNodes: LogicNode[];
  initialEdges: LogicEdge[];
  theme: 'light' | 'dark';
  showDebugger: boolean;
  exampleSuggestions?: string[];
  onSelectExample?: (name: string) => void;
  configSummary?: string | null;
}) {
  // Background dot colors based on theme
  const bgColor = theme === 'dark' ? '#404040' : '#cccccc';
  const direction = useDirection();

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

  const currentEdges = useMemo(() => buildEdgesFromNodes(nodes, direction), [nodes, direction]);

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
              markerEnd: DEFAULT_EDGE_MARKER,
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
          <EmptyState
            exampleSuggestions={exampleSuggestions}
            onSelectExample={onSelectExample}
            configSummary={configSummary}
          />
        )}
    </ConnectedHandlesProvider>
  );
}

interface EditorBodyProps {
  value: DataLogicEditorProps['value'];
  onChange?: DataLogicEditorProps['onChange'];
  data?: unknown;
  resolvedTheme: 'light' | 'dark';
  className: string;
  templating: boolean;
  onTemplatingChange?: (value: boolean) => void;
  editable: boolean;
  exampleSuggestions?: string[];
  onSelectExample?: (name: string) => void;
  direction: FlowDirection;
  onDirectionChange: (direction: FlowDirection) => void;
  configSummary: string | null;
  evaluateWithTrace?: (logic: unknown, data: unknown) => TracedResult;
}

/**
 * Everything below the engine boundary. Keyed by the engine identity
 * (config + custom operators) so a settings change re-runs the trace with
 * fresh results even though the expression and data are unchanged.
 */
function DataLogicEditorBody({
  value,
  onChange,
  data,
  resolvedTheme,
  className,
  templating,
  onTemplatingChange,
  editable,
  exampleSuggestions,
  onSelectExample,
  direction,
  onDirectionChange,
  configSummary,
  evaluateWithTrace,
}: EditorBodyProps) {
  // Debounce timer ref for onChange
  const onChangeTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Determine if we're in edit mode
  const isEditMode = editable;

  // Evaluation is enabled whenever data is provided (unified mode - no mode switching needed)
  const evalEnabled = data !== undefined;

  // Use trace-based evaluation when data is available
  const editor = useLogicEditor({
    value,
    evaluateWithTrace: evalEnabled ? evaluateWithTrace : undefined,
    data: evalEnabled ? data : undefined,
    templating,
    direction,
  });

  // Use a combination of node count, edge count, and root node ID as key
  // This ensures the component remounts when the expression structure changes
  const expressionKey = `${editor.nodes.length}-${editor.edges.length}-${editor.nodes[0]?.id ?? 'empty'}-${direction}`;

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
          {configSummary && (
            <p className="logic-editor-error-config">Engine settings: {configSummary}</p>
          )}
        </div>
      </div>
    );
  }

  // Build the class name
  const editorClassName = ['logic-editor', className].filter(Boolean).join(' ');

  const toolbar = (
    <EditorToolbar
      isEditMode={isEditMode}
      hasDebugger={hasDebugger}
      templating={templating}
      onTemplatingChange={onTemplatingChange}
      direction={direction}
      onDirectionChange={onDirectionChange}
      configSummary={configSummary}
    />
  );

  // --- Read-only mode: skip EditorProvider entirely ---
  if (!isEditMode) {
    const readOnlyInner = (
      <DirectionContext.Provider value={direction}>
        <ReadOnlyEditorInner
          key={expressionKey}
          initialNodes={editor.nodes}
          initialEdges={editor.edges}
          theme={resolvedTheme}
          showDebugger={false}
          exampleSuggestions={exampleSuggestions}
          onSelectExample={onSelectExample}
          configSummary={configSummary}
        />
      </DirectionContext.Provider>
    );

    return (
      <div className={editorClassName} data-theme={resolvedTheme} data-direction={direction}>
        {hasDebugger ? (
          <DebuggerProvider
            steps={editor.steps}
            traceNodeMap={editor.traceNodeMap}
            nodes={editor.nodes}
            failedNodeIds={editor.failedNodeIds}
            traceError={editor.traceError}
          >
            {toolbar}
            <div className="logic-editor-body">
              <div className="logic-editor-main">
                {readOnlyInner}
              </div>
            </div>
          </DebuggerProvider>
        ) : (
          <>
            {toolbar}
            {editor.traceError && <TraceErrorBanner failure={editor.traceError} />}
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
    <DirectionContext.Provider value={direction}>
      <EditableEditorInner
        key={expressionKey}
        initialNodes={editor.nodes}
        initialEdges={editor.edges}
        theme={resolvedTheme}
        showDebugger={false}
        exampleSuggestions={exampleSuggestions}
        onSelectExample={onSelectExample}
        configSummary={configSummary}
      />
    </DirectionContext.Provider>
  );

  return (
    <EditorProvider
      nodes={editor.nodes}
      initialEditMode={isEditMode}
      onNodesChange={handleNodesChange}
    >
      <KeyboardHandler />
      <div className={editorClassName} data-theme={resolvedTheme} data-direction={direction}>
        {hasDebugger ? (
          <DebuggerProvider
            steps={editor.steps}
            traceNodeMap={editor.traceNodeMap}
            nodes={editor.nodes}
            failedNodeIds={editor.failedNodeIds}
            traceError={editor.traceError}
          >
            {toolbar}
            <div className="logic-editor-body">
              <div className="logic-editor-main">
                {editableInner}
              </div>
              <PropertiesPanel />
            </div>
          </DebuggerProvider>
        ) : (
          <>
            {toolbar}
            {editor.traceError && <TraceErrorBanner failure={editor.traceError} />}
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

export function DataLogicEditor({
  value,
  onChange,
  data,
  theme: themeProp,
  className = '',
  templating = false,
  onTemplatingChange,
  config,
  customOperators,
  editable = false,
  exampleSuggestions,
  onSelectExample,
}: DataLogicEditorProps) {
  // Diagram direction: 'flow' (data flow, root on the right) by default, or
  // 'hierarchy' (root on the left, JSON nesting order). Toggled from the toolbar.
  const [direction, setDirection] = useState<FlowDirection>('flow');

  // Theme handling - use prop override or system preference
  const systemTheme = useSystemTheme();
  const resolvedTheme = themeProp ?? systemTheme;

  // Internal WASM evaluator: one Engine per (templating, config, customOperators)
  const {
    ready: wasmReady,
    evaluateWithTrace,
  } = useWasmEvaluator({ templating, config, customOperators });

  const configKey = useMemo(
    () => JSON.stringify(normalizeEvaluationConfig(config) ?? null),
    [config],
  );
  const configSummary = useMemo(() => summarizeEvaluationConfig(config), [config]);
  // Remount only when the vocabulary actually changes. Keying on the object
  // identity instead would remount on every parent render for the idiomatic
  // inline `customOperators={{ ... }}`, discarding selection, undo history
  // and debugger position each time.
  const engineKey = `${configKey}|${customOperatorNamesKey(customOperators)}`;

  return (
    <DataLogicEditorBody
      key={engineKey}
      value={value}
      onChange={onChange}
      data={data}
      resolvedTheme={resolvedTheme}
      className={className}
      templating={templating}
      onTemplatingChange={onTemplatingChange}
      editable={editable}
      exampleSuggestions={exampleSuggestions}
      onSelectExample={onSelectExample}
      direction={direction}
      onDirectionChange={setDirection}
      configSummary={configSummary}
      evaluateWithTrace={wasmReady ? evaluateWithTrace : undefined}
    />
  );
}

export default DataLogicEditor;
