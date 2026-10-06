import { useEffect, useMemo, useCallback, useRef, useState, type ReactNode } from 'react';
import {
  ReactFlow,
  Background,
  Controls,
  useNodesState,
  useEdgesState,
  ReactFlowProvider,
  MarkerType,
  type OnEdgesChange,
  type OnNodesChange,
  type ReactFlowProps,
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
import { useLogicEditor, useWasmEvaluator } from './hooks';
import { engineSettingsKey, summarizeEvaluationConfig } from './hooks/useWasmEvaluator';
import { useContextMenu } from './hooks/useContextMenu';
import { getHiddenNodeIds } from './utils/visibility';
import { buildEdgesFromNodes } from './utils/edge-builder';
import { nodesToJsonLogic } from './utils/nodes-to-jsonlogic';
import { formatTraceFailure, traceFailureType, type TraceFailure } from './utils/trace';
import { DebuggerProvider, ConnectedHandlesProvider, EditorProvider, DirectionContext, useDirection, type FlowDirection } from './context';
import { useEditorContext } from './context/editor';
import { EditorRootContext } from './context/EditorRootContext';
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
  return <ErrorBanner kind={traceFailureType(failure)} message={formatTraceFailure(failure)} />;
}

function ErrorBanner({ kind, message }: { kind?: string | null; message: string }) {
  return (
    <div className="logic-editor-trace-error" role="alert">
      {kind && <span className="logic-editor-trace-error-kind">{kind}</span>}
      <span className="logic-editor-trace-error-message">{message}</span>
    </div>
  );
}

/**
 * Banner for an engine that failed to load. Without it the editor fell back
 * to the static diagram and the debugger silently never appeared.
 */
function EngineErrorBanner({ error }: { error: string }) {
  return <ErrorBanner kind="Engine" message={`The evaluation engine failed to load: ${error}`} />;
}

interface CanvasProps {
  initialNodes: LogicNode[];
  initialEdges: LogicEdge[];
  theme: 'light' | 'dark';
  exampleSuggestions?: string[];
  onSelectExample?: (name: string) => void;
  configSummary?: string | null;
}

type EditableFlowHandlers = Pick<
  ReactFlowProps<LogicNode, LogicEdge>,
  'onNodeContextMenu' | 'onPaneContextMenu' | 'onNodeDoubleClick'
>;

interface FlowCanvasProps extends CanvasProps {
  nodes: LogicNode[];
  onNodesChange: OnNodesChange<LogicNode>;
  onEdgesChange: OnEdgesChange<LogicEdge>;
  editable: boolean;
  flowHandlers?: EditableFlowHandlers;
  /** Extra overlays rendered inside the ReactFlow canvas. */
  children?: ReactNode;
}

/**
 * The diagram, shared by the read-only and editable canvases: hides
 * collapsed subtrees, derives the edges from the nodes and renders the
 * ReactFlow board (or the empty state).
 */
function FlowCanvas({
  nodes,
  onNodesChange,
  onEdgesChange,
  editable,
  flowHandlers,
  children,
  initialNodes,
  theme,
  exampleSuggestions,
  onSelectExample,
  configSummary,
}: FlowCanvasProps) {
  // Background dot colors based on theme
  const bgColor = theme === 'dark' ? '#404040' : '#cccccc';
  const direction = useDirection();

  // Compute hidden node IDs based on collapsed state
  const hiddenNodeIds = useMemo(() => getHiddenNodeIds(nodes), [nodes]);
  const nodeIds = useMemo(() => new Set(nodes.map((n) => n.id)), [nodes]);

  const visibleNodes = useMemo(
    () => nodes.filter((node) => !hiddenNodeIds.has(node.id)),
    [nodes, hiddenNodeIds]
  );

  const currentEdges = useMemo(() => buildEdgesFromNodes(nodes, direction), [nodes, direction]);

  const visibleEdges = useMemo(() => {
    const visible = currentEdges.filter(
      (edge) =>
        nodeIds.has(edge.source) &&
        nodeIds.has(edge.target) &&
        !hiddenNodeIds.has(edge.source) &&
        !hiddenNodeIds.has(edge.target)
    );
    return editable ? visible.map((edge) => ({ ...edge, type: 'editable' })) : visible;
  }, [currentEdges, nodeIds, hiddenNodeIds, editable]);

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
          {...flowHandlers}
        >
          <Background color={bgColor} gap={20} size={1} />
          <Controls showInteractive={editable} />
          <AutoFitView nodeCount={initialNodes.length} />
          {children}
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
 * Read-only canvas: no EditorContext dependency, so editable=false skips
 * EditorProvider's state sync effects.
 */
function ReadOnlyCanvas(props: CanvasProps) {
  const [nodes, , onNodesChange] = useNodesState<LogicNode>(props.initialNodes);
  const [, , onEdgesChange] = useEdgesState<LogicEdge>(props.initialEdges);

  return (
    <FlowCanvas
      {...props}
      nodes={nodes}
      onNodesChange={onNodesChange}
      onEdgesChange={onEdgesChange}
      editable={false}
    />
  );
}

function markSelected(nodes: LogicNode[], selected: ReadonlySet<string>): LogicNode[] {
  if (selected.size === 0) return nodes;
  return nodes.map((n) => (selected.has(n.id) ? { ...n, selected: true } : n));
}

/**
 * Editable canvas: keeps the ReactFlow nodes in step with EditorContext and
 * adds selection, context menus and double-click editing.
 */
function EditableCanvas(props: CanvasProps) {
  const { initialNodes, initialEdges } = props;

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
  const { nodes: editorNodes, selectedNodeIds } = useEditorContext();

  // Initialize state directly from props - component remounts via key when
  // the expression's structure changes. A selection the editor carried over
  // starts out marked, so ReactFlow does not report it as cleared.
  const [mountNodes] = useState(() => markSelected(initialNodes, selectedNodeIds));
  const [nodes, setNodes, onNodesChange] = useNodesState<LogicNode>(mountNodes);
  // Note: We don't use edges state directly - edges are rebuilt from nodes
  const [, , onEdgesChange] = useEdgesState<LogicEdge>(initialEdges);

  // Sync state when props change (handles cases where key doesn't trigger
  // remount). Node ids survive a re-conversion, so keep ReactFlow's
  // selection marks: dropping them made NodeSelectionHandler clear the
  // editor's selection.
  useEffect(() => {
    setNodes((current) => {
      const selected = new Set(current.filter((n) => n.selected).map((n) => n.id));
      return markSelected(initialNodes, selected);
    });
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

  const flowHandlers = useMemo<EditableFlowHandlers>(
    () => ({
      onNodeContextMenu: handleNodeContextMenu,
      onPaneContextMenu: handlePaneContextMenu,
      onNodeDoubleClick: handleNodeDoubleClick,
    }),
    [handleNodeContextMenu, handlePaneContextMenu, handleNodeDoubleClick]
  );

  return (
    <FlowCanvas
      {...props}
      nodes={nodes}
      onNodesChange={onNodesChange}
      onEdgesChange={onEdgesChange}
      editable
      flowHandlers={flowHandlers}
    >
      <NodeSelectionHandler />
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
    </FlowCanvas>
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
  /** The WASM engine's load error, if it failed to start. */
  engineError: string | null;
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
  engineError,
}: EditorBodyProps) {
  // Debounce timer ref for onChange
  const onChangeTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // The root element: keyboard shortcuts listen on it (EditorRootContext).
  const [rootElement, setRootElement] = useState<HTMLDivElement | null>(null);

  // The last expression this editor reported through onChange, serialized.
  const [lastEmitted, setLastEmitted] = useState<string | null>(null);

  // Counts values that came from outside, as opposed to the echo of this
  // editor's own edit. Part of the canvas key: a new rule from the host
  // remounts (and refits) the canvas even when its node count matches the
  // old one, while an echo keeps the canvas, its viewport and selection.
  const [valueRevision, setValueRevision] = useState({ value, revision: 0 });
  if (value !== valueRevision.value) {
    const isEcho = lastEmitted !== null && JSON.stringify(value) === lastEmitted;
    setValueRevision({
      value,
      revision: isEcho ? valueRevision.revision : valueRevision.revision + 1,
    });
  }

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

  // Remount the canvas for a value from outside, or when the expression's
  // structure (node count, edge count, root) or the direction changes.
  const expressionKey = `${valueRevision.revision}-${editor.nodes.length}-${editor.edges.length}-${editor.nodes[0]?.id ?? 'empty'}-${direction}`;

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
        setLastEmitted(JSON.stringify(newExpr));
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
      isEditMode={editable}
      hasDebugger={hasDebugger}
      templating={templating}
      onTemplatingChange={onTemplatingChange}
      direction={direction}
      onDirectionChange={onDirectionChange}
      configSummary={configSummary}
    />
  );

  const canvasProps: CanvasProps = {
    initialNodes: editor.nodes,
    initialEdges: editor.edges,
    theme: resolvedTheme,
    exampleSuggestions,
    onSelectExample,
    configSummary,
  };

  const body = (
    <div className="logic-editor-body">
      <div className="logic-editor-main">
        <DirectionContext.Provider value={direction}>
          {editable ? (
            <EditableCanvas key={expressionKey} {...canvasProps} />
          ) : (
            <ReadOnlyCanvas key={expressionKey} {...canvasProps} />
          )}
        </DirectionContext.Provider>
      </div>
      {editable && <PropertiesPanel />}
    </div>
  );

  // tabIndex -1 makes a click anywhere in the editor (the canvas included)
  // focus it, so its shortcuts work after pointing at it, without adding a
  // tab stop.
  const shell = (
    <div
      ref={setRootElement}
      tabIndex={-1}
      className={editorClassName}
      data-theme={resolvedTheme}
      data-direction={direction}
    >
      {hasDebugger ? (
        <DebuggerProvider
          steps={editor.steps}
          traceNodeMap={editor.traceNodeMap}
          nodes={editor.nodes}
          failedNodeIds={editor.failedNodeIds}
          traceError={editor.traceError}
        >
          {toolbar}
          {body}
        </DebuggerProvider>
      ) : (
        <>
          {toolbar}
          {evalEnabled && engineError && <EngineErrorBanner error={engineError} />}
          {editor.traceError && <TraceErrorBanner failure={editor.traceError} />}
          {body}
        </>
      )}
    </div>
  );

  return (
    <EditorRootContext.Provider value={rootElement}>
      {editable ? (
        <EditorProvider
          nodes={editor.nodes}
          initialEditMode={editable}
          onNodesChange={handleNodesChange}
        >
          <KeyboardHandler />
          {shell}
        </EditorProvider>
      ) : (
        // Read-only mode skips EditorProvider entirely.
        shell
      )}
    </EditorRootContext.Provider>
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
    error: wasmError,
    evaluateWithTrace,
  } = useWasmEvaluator({ templating, config, customOperators });

  const configSummary = useMemo(() => summarizeEvaluationConfig(config), [config]);
  // Remount only when the settings or the vocabulary actually change. Keying
  // on the object identity instead would remount on every parent render for
  // the idiomatic inline `customOperators={{ ... }}`, discarding selection,
  // undo history and debugger position each time.
  const engineKey = engineSettingsKey(config, customOperators);

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
      engineError={wasmError}
    />
  );
}

export default DataLogicEditor;
