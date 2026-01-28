import { useEffect, useMemo, useState } from 'react';
import {
  ReactFlow,
  Background,
  Controls,
  useNodesState,
  useEdgesState,
  useReactFlow,
  ReactFlowProvider,
} from '@xyflow/react';
import '@xyflow/react/dist/style.css';

import type { DataLogicEditorProps, LogicNode, LogicEdge } from './types';
import { nodeTypes } from './nodes';
import { useLogicEditor, useWasmEvaluator, useDebugEvaluation, type EvaluationResultsMap } from './hooks';
import { getHiddenNodeIds } from './utils/visibility';
import { buildEdgesFromNodes } from './utils/edge-builder';
import { EvaluationContext, DebuggerProvider, ConnectedHandlesProvider } from './context';
import { DebuggerControls } from './debugger-controls';
import './styles/nodes.css';
import './LogicEditor.css';

const emptyResults: EvaluationResultsMap = new Map();

// Hook to detect system theme
function useSystemTheme(): 'light' | 'dark' {
  const [systemTheme, setSystemTheme] = useState<'light' | 'dark'>(() => {
    if (typeof window === 'undefined') return 'light';
    return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
  });

  useEffect(() => {
    if (typeof window === 'undefined') return;

    const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
    const handler = (e: MediaQueryListEvent) => {
      setSystemTheme(e.matches ? 'dark' : 'light');
    };

    mediaQuery.addEventListener('change', handler);
    return () => mediaQuery.removeEventListener('change', handler);
  }, []);

  return systemTheme;
}

// Helper component to auto-fit view (must be inside ReactFlow)
function AutoFitView({ nodeCount }: { nodeCount: number }) {
  const { fitView } = useReactFlow();

  useEffect(() => {
    const timer = setTimeout(() => {
      fitView({ padding: 0.2, maxZoom: 0.75 });
    }, 50);
    return () => clearTimeout(timer);
  }, [nodeCount, fitView]);

  return null;
}

// Inner component that handles ReactFlow state
function DataLogicEditorInner({
  initialNodes,
  initialEdges,
  readOnly,
  evaluationResults,
  theme,
  showDebugger,
}: {
  initialNodes: LogicNode[];
  initialEdges: LogicEdge[];
  readOnly: boolean;
  evaluationResults: EvaluationResultsMap;
  theme: 'light' | 'dark';
  showDebugger: boolean;
}) {
  // Background dot colors based on theme
  const bgColor = theme === 'dark' ? '#404040' : '#cccccc';

  // Initialize state directly from props - component remounts via key when expression changes
  const [nodes, setNodes, onNodesChange] = useNodesState<LogicNode>(initialNodes);
  // Note: We don't use edges state directly - edges are rebuilt from nodes
  const [, , onEdgesChange] = useEdgesState<LogicEdge>(initialEdges);

  // Sync state when props change (handles cases where key doesn't trigger remount)
  useEffect(() => {
    setNodes(initialNodes);
  }, [initialNodes, setNodes]);

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
    <EvaluationContext.Provider value={evaluationResults}>
      <ConnectedHandlesProvider edges={visibleEdges}>
        <ReactFlowProvider>
          <ReactFlow
            nodes={visibleNodes}
            edges={visibleEdges}
            onNodesChange={readOnly ? undefined : onNodesChange}
            onEdgesChange={readOnly ? undefined : onEdgesChange}
            nodeTypes={nodeTypes}
            fitView
            fitViewOptions={{ padding: 0.2, maxZoom: 0.75 }}
            minZoom={0.1}
            maxZoom={2}
            defaultEdgeOptions={{
              type: 'default',
              animated: false,
            }}
          >
            <Background color={bgColor} gap={20} size={1} />
            <Controls showInteractive={!readOnly} />
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

export function DataLogicEditor({
  value,
  onChange: _onChange,
  data,
  mode = 'visualize',
  theme: themeProp,
  className = '',
  preserveStructure = false,
  componentMode: _componentMode = 'debugger',
}: DataLogicEditorProps) {
  // Warn about edit mode not being implemented
  if (mode === 'edit') {
    console.warn('[DataLogicEditor] Edit mode is not yet implemented. Component will render in read-only mode with debug evaluation if data is provided.');
  }
  void _onChange; // Editor mode not yet implemented
  void _componentMode; // Component mode is handled by parent components (App.tsx, embed.tsx)

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

  // Check if debugger should be active (trace mode with steps)
  const showDebugger = debugEnabled && editor.usingTraceMode && editor.steps.length > 0;

  // Allow node interactions (collapse/expand) in all modes
  // Full editing (adding/removing nodes) is still a future feature
  const readOnly = false;

  const editorInner = (
    <DataLogicEditorInner
      key={expressionKey}
      initialNodes={editor.nodes}
      initialEdges={editor.edges}
      readOnly={readOnly}
      evaluationResults={results}
      theme={resolvedTheme}
      showDebugger={showDebugger}
    />
  );

  return (
    <div className={`logic-editor ${className}`} data-theme={resolvedTheme}>
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
  );
}

export default DataLogicEditor;
