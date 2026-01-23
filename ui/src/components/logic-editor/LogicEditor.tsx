import { useEffect, useMemo } from 'react';
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

import { useTheme } from '../../hooks';
import type { LogicEditorProps, LogicNode, LogicEdge } from './types';
import { nodeTypes } from './nodes';
import { useLogicEditor, useDebugEvaluation, type EvaluationResultsMap } from './hooks';
import { getHiddenNodeIds } from './utils/visibility';
import { buildEdgesFromNodes } from './utils/edge-builder';
import { EvaluationContext, DebuggerProvider, ConnectedHandlesProvider } from './context';
import { DebuggerControls } from './debugger-controls';
import './styles/nodes.css';
import './LogicEditor.css';

const emptyResults: EvaluationResultsMap = new Map();

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
// This is keyed by expression to force remount on expression change
function LogicEditorInner({
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
  const [nodes, setNodes, onNodesChange] = useNodesState<LogicNode>(initialNodes);
  // Note: We don't use edges state directly - edges are rebuilt from nodes
  const [, , onEdgesChange] = useEdgesState<LogicEdge>(initialEdges);

  // Update when initial values change (shouldn't happen due to key, but just in case)
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

export function LogicEditor({
  value,
  onChange: _onChange, // Accepted for interface compatibility, but not used (read-only visualizer)
  readOnly = false,
  className = '',
  debugData,
  evaluate,
  evaluateWithTrace,
}: LogicEditorProps) {
  void _onChange; // Suppress unused variable warning
  const { theme } = useTheme();
  const editor = useLogicEditor({
    value,
    evaluateWithTrace,
    data: debugData,
  });

  // Use root node ID as key - it's a UUID that changes with each new expression
  const expressionKey = editor.nodes.length > 0 ? editor.nodes[0].id : 'empty';

  // Fallback: Compute evaluation results using multiple evaluate calls (only when not in trace mode)
  const fallbackResults = useDebugEvaluation({
    nodes: editor.nodes,
    data: debugData,
    evaluate: evaluate ?? null,
    // Only enable fallback when trace mode is not active
    enabled: !editor.usingTraceMode && !!evaluate && debugData !== undefined,
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
      <div className={`logic-editor ${className}`}>
        <div className="logic-editor-error">
          <p className="logic-editor-error-title">Error rendering expression</p>
          <p className="logic-editor-error-message">{editor.error}</p>
        </div>
      </div>
    );
  }

  // Check if debugger should be active (trace mode with steps)
  const showDebugger = editor.usingTraceMode && editor.steps.length > 0;

  const editorInner = (
    <LogicEditorInner
      key={expressionKey}
      initialNodes={editor.nodes}
      initialEdges={editor.edges}
      readOnly={readOnly}
      evaluationResults={results}
      theme={theme}
      showDebugger={showDebugger}
    />
  );

  return (
    <div className={`logic-editor ${className}`}>
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

export default LogicEditor;
