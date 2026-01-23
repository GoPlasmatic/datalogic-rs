/**
 * DataLogic Embed - Standalone bundle for embedding in static HTML pages
 *
 * This module exposes a global `DataLogicEmbed` object that can be used to render
 * DataLogic visual widgets and playgrounds in mdBook or any static HTML page.
 *
 * Usage:
 * 1. Load React and ReactDOM from CDN
 * 2. Load this bundle (datalogic-embed.js)
 * 3. Call DataLogicEmbed.init() to auto-render all widgets
 *
 * Or manually:
 * - DataLogicEmbed.renderWidget(element, { logic, data, mode })
 * - DataLogicEmbed.renderPlayground(element)
 */

import React, { useState, useCallback, useEffect } from 'react';
import ReactDOM from 'react-dom/client';
import { DataLogicEditor, type JsonLogicValue } from './components/logic-editor';
import { ModeSelector } from './components/mode-selector';
import { useWasmEvaluator } from './components/logic-editor/hooks';

// Import styles - include all component CSS (but NOT index.css which has global styles)
import '@xyflow/react/dist/style.css';
// NOTE: index.css is NOT imported here because it has global styles that affect the entire page
import './components/logic-editor/styles/nodes.css';
import './components/logic-editor/LogicEditor.css';
import './components/logic-editor/debugger-controls/DebuggerControls.css';
import './components/mode-selector/ModeSelector.css';
import './components/debug-panel/DebugPanel.css';
import './embed.css';

// Track mounted roots for cleanup
const mountedRoots = new Map<Element, ReactDOM.Root>();

// ============================================
// JSON Syntax Highlighter
// ============================================

/**
 * Highlight JSON text with syntax coloring
 */
function highlightJsonText(text: string): React.ReactNode[] {
  if (!text) return [];

  const result: React.ReactNode[] = [];
  let i = 0;
  let keyIndex = 0;

  while (i < text.length) {
    const char = text[i];

    // Whitespace
    if (/\s/.test(char)) {
      let ws = '';
      while (i < text.length && /\s/.test(text[i])) {
        ws += text[i];
        i++;
      }
      result.push(ws);
      continue;
    }

    // String (key or value)
    if (char === '"') {
      let str = '"';
      i++;
      while (i < text.length && text[i] !== '"') {
        if (text[i] === '\\' && i + 1 < text.length) {
          str += text[i] + text[i + 1];
          i += 2;
        } else {
          str += text[i];
          i++;
        }
      }
      if (i < text.length) {
        str += '"';
        i++;
      }

      // Check if this is a key (followed by :)
      let j = i;
      while (j < text.length && /\s/.test(text[j])) j++;
      const isKey = text[j] === ':';

      result.push(
        <span key={`str-${keyIndex++}`} className={isKey ? 'json-key' : 'json-string'}>
          {str}
        </span>
      );
      continue;
    }

    // Number
    if (/[-\d]/.test(char)) {
      let num = '';
      while (i < text.length && /[-\d.eE+]/.test(text[i])) {
        num += text[i];
        i++;
      }
      result.push(<span key={`num-${keyIndex++}`} className="json-number">{num}</span>);
      continue;
    }

    // Boolean or null
    if (text.slice(i, i + 4) === 'true') {
      result.push(<span key={`bool-${keyIndex++}`} className="json-boolean">true</span>);
      i += 4;
      continue;
    }
    if (text.slice(i, i + 5) === 'false') {
      result.push(<span key={`bool-${keyIndex++}`} className="json-boolean">false</span>);
      i += 5;
      continue;
    }
    if (text.slice(i, i + 4) === 'null') {
      result.push(<span key={`null-${keyIndex++}`} className="json-null">null</span>);
      i += 4;
      continue;
    }

    // Punctuation
    if (/[{}\[\]:,]/.test(char)) {
      result.push(<span key={`punc-${keyIndex++}`} className="json-punctuation">{char}</span>);
      i++;
      continue;
    }

    // Other characters (invalid JSON, but still render)
    result.push(char);
    i++;
  }

  return result;
}

interface JsonHighlightProps {
  value: unknown;
  placeholder?: string;
}

function JsonHighlight({ value, placeholder = '' }: JsonHighlightProps) {
  if (value === undefined) {
    return <pre className="json-highlight">{placeholder}</pre>;
  }

  const text = JSON.stringify(value, null, 2);
  return <pre className="json-highlight">{highlightJsonText(text)}</pre>;
}

interface JsonEditorProps {
  value: string;
  onChange: (e: React.ChangeEvent<HTMLTextAreaElement>) => void;
  hasError?: boolean;
  placeholder?: string;
  className?: string;
}

function JsonEditor({ value, onChange, hasError, placeholder, className }: JsonEditorProps) {
  const textareaRef = React.useRef<HTMLTextAreaElement>(null);
  const highlightRef = React.useRef<HTMLPreElement>(null);

  // Sync scroll between textarea and highlight overlay
  const handleScroll = useCallback(() => {
    if (textareaRef.current && highlightRef.current) {
      highlightRef.current.scrollTop = textareaRef.current.scrollTop;
      highlightRef.current.scrollLeft = textareaRef.current.scrollLeft;
    }
  }, []);

  return (
    <div className={`json-editor ${className || ''} ${hasError ? 'has-error' : ''}`}>
      <pre
        ref={highlightRef}
        className="json-editor-highlight json-highlight"
        aria-hidden="true"
      >
        {value ? highlightJsonText(value) : <span className="json-placeholder">{placeholder}</span>}
      </pre>
      <textarea
        ref={textareaRef}
        className="json-editor-input"
        value={value}
        onChange={onChange}
        onScroll={handleScroll}
        spellCheck={false}
        placeholder=""
      />
    </div>
  );
}

// ============================================
// Widget Component - Two-row layout with inputs and diagram
// ============================================

interface WidgetProps {
  logic: JsonLogicValue;
  data?: object;
  mode?: 'visualize' | 'debug';
  height?: string;
  theme?: 'light' | 'dark' | 'auto';
}

function Widget({ logic: initialLogic, data: initialData = {}, mode = 'visualize', height = '500px', theme = 'auto' }: WidgetProps) {
  // Detect theme from mdBook or system
  const resolvedTheme = theme === 'auto' ? detectTheme() : theme;

  // State for editable inputs
  const [logicText, setLogicText] = useState<string>(JSON.stringify(initialLogic, null, 2));
  const [logic, setLogic] = useState<JsonLogicValue>(initialLogic);
  const [logicError, setLogicError] = useState<string | null>(null);

  const [dataText, setDataText] = useState<string>(JSON.stringify(initialData, null, 2));
  const [data, setData] = useState<object>(initialData);
  const [dataError, setDataError] = useState<string | null>(null);

  const [result, setResult] = useState<unknown>(undefined);
  const [resultError, setResultError] = useState<string | null>(null);

  const { ready: wasmReady, evaluate } = useWasmEvaluator({});

  // Handle logic text changes
  const handleLogicChange = useCallback((e: React.ChangeEvent<HTMLTextAreaElement>) => {
    const text = e.target.value;
    setLogicText(text);

    if (!text.trim()) {
      setLogic({});
      setLogicError(null);
      return;
    }

    try {
      const parsed = JSON.parse(text);
      setLogic(parsed);
      setLogicError(null);
    } catch (err) {
      setLogicError(err instanceof Error ? err.message : 'Invalid JSON');
    }
  }, []);

  // Handle data text changes
  const handleDataChange = useCallback((e: React.ChangeEvent<HTMLTextAreaElement>) => {
    const text = e.target.value;
    setDataText(text);

    if (!text.trim()) {
      setData({});
      setDataError(null);
      return;
    }

    try {
      const parsed = JSON.parse(text);
      if (typeof parsed !== 'object' || parsed === null || Array.isArray(parsed)) {
        setDataError('Data must be a JSON object');
        return;
      }
      setData(parsed);
      setDataError(null);
    } catch (err) {
      setDataError(err instanceof Error ? err.message : 'Invalid JSON');
    }
  }, []);

  // Evaluate expression when inputs change
  useEffect(() => {
    if (!wasmReady || logicError || dataError) {
      setResult(undefined);
      setResultError(null);
      return;
    }

    try {
      const evalResult = evaluate(logic, data);
      setResult(evalResult);
      setResultError(null);
    } catch (err) {
      setResult(undefined);
      setResultError(err instanceof Error ? err.message : 'Evaluation failed');
    }
  }, [wasmReady, logic, data, logicError, dataError, evaluate]);

  return (
    <div className="datalogic-widget" style={{ height }} data-theme={resolvedTheme}>
      {/* Row 1: Logic, Data, Result columns */}
      <div className="widget-input-row">
        <div className="widget-column">
          <div className="widget-column-header">Logic</div>
          <JsonEditor
            value={logicText}
            onChange={handleLogicChange}
            hasError={!!logicError}
            placeholder="Enter JSONLogic expression..."
            className="widget-json-editor"
          />
          {logicError && <div className="widget-error">{logicError}</div>}
        </div>

        <div className="widget-column">
          <div className="widget-column-header">Data</div>
          <JsonEditor
            value={dataText}
            onChange={handleDataChange}
            hasError={!!dataError}
            placeholder="Enter JSON data..."
            className="widget-json-editor"
          />
          {dataError && <div className="widget-error">{dataError}</div>}
        </div>

        <div className="widget-column">
          <div className="widget-column-header">Result</div>
          <div className={`widget-result ${resultError ? 'has-error' : ''}`}>
            {resultError ? (
              <span className="widget-result-error">{resultError}</span>
            ) : (
              <JsonHighlight value={result} placeholder="" />
            )}
          </div>
        </div>
      </div>

      {/* Row 2: Diagram */}
      <div className="widget-diagram-row">
        <DataLogicEditor
          value={logic}
          data={data}
          mode={mode}
          theme={resolvedTheme}
          className="datalogic-widget-editor"
        />
      </div>
    </div>
  );
}

// ============================================
// Playground Component - Full interactive editor
// ============================================

// Sample expressions for the playground
const SAMPLE_EXPRESSIONS: Record<string, { logic: JsonLogicValue; data: object }> = {
  'Simple Comparison': {
    logic: { '==': [1, 1] },
    data: {},
  },
  'Variable Access': {
    logic: { var: 'user.name' },
    data: { user: { name: 'Alice', age: 30 } },
  },
  'Conditional': {
    logic: { if: [{ '>=': [{ var: 'age' }, 18] }, 'adult', 'minor'] },
    data: { age: 21 },
  },
  'Array Filter': {
    logic: { filter: [{ var: 'numbers' }, { '>': [{ var: '' }, 5] }] },
    data: { numbers: [1, 3, 5, 7, 9, 11] },
  },
  'Array Map': {
    logic: { map: [{ var: 'items' }, { '*': [{ var: '' }, 2] }] },
    data: { items: [1, 2, 3, 4, 5] },
  },
  'Grade Calculator': {
    logic: {
      if: [
        { '>=': [{ var: 'score' }, 90] }, 'A - Excellent',
        { '>=': [{ var: 'score' }, 80] }, 'B - Good',
        { '>=': [{ var: 'score' }, 70] }, 'C - Average',
        { '>=': [{ var: 'score' }, 60] }, 'D - Below Average',
        'F - Fail',
      ],
    },
    data: { score: 78 },
  },
  'Reduce - Sum': {
    logic: {
      reduce: [
        { var: 'items' },
        { '+': [{ var: 'accumulator' }, { var: 'current' }] },
        0,
      ],
    },
    data: { items: [10, 20, 30, 40] },
  },
  'Discount Price': {
    logic: {
      '*': [
        { var: 'price' },
        { '-': [1, { '/': [{ var: 'discountPercent' }, 100] }] },
      ],
    },
    data: { price: 150, discountPercent: 25 },
  },
};

function Playground() {
  
  const [logicText, setLogicText] = useState<string>('');
  const [expression, setExpression] = useState<JsonLogicValue | null>(null);
  const [logicError, setLogicError] = useState<string | null>(null);

  const [dataText, setDataText] = useState<string>('{}');
  const [data, setData] = useState<object>({});
  const [dataError, setDataError] = useState<string | null>(null);

  const [result, setResult] = useState<unknown>(undefined);
  const [resultError, setResultError] = useState<string | null>(null);

  const [editorMode, setEditorMode] = useState<'visualize' | 'debug'>('debug');
  const [selectedExample, setSelectedExample] = useState<string>('');

  // Detect theme
  const theme = detectTheme();
  
  const { ready: wasmReady, loading: wasmLoading, evaluate } = useWasmEvaluator({});

  // Handle logic text changes
  const handleLogicChange = useCallback((e: React.ChangeEvent<HTMLTextAreaElement>) => {
    const text = e.target.value;
    setLogicText(text);

    if (!text.trim()) {
      setExpression(null);
      setLogicError(null);
      return;
    }

    try {
      const parsed = JSON.parse(text);
      setExpression(parsed);
      setLogicError(null);
    } catch (err) {
      setLogicError(err instanceof Error ? err.message : 'Invalid JSON');
    }
  }, []);

  // Handle data text changes
  const handleDataChange = useCallback((e: React.ChangeEvent<HTMLTextAreaElement>) => {
    const text = e.target.value;
    setDataText(text);

    if (!text.trim()) {
      setData({});
      setDataError(null);
      return;
    }

    try {
      const parsed = JSON.parse(text);
      if (typeof parsed !== 'object' || parsed === null || Array.isArray(parsed)) {
        setDataError('Data must be a JSON object');
        return;
      }
      setData(parsed);
      setDataError(null);
    } catch (err) {
      setDataError(err instanceof Error ? err.message : 'Invalid JSON');
    }
  }, []);

  // Load sample expression
  const loadSample = useCallback((name: string) => {
    const sample = SAMPLE_EXPRESSIONS[name];
    if (sample) {
      setSelectedExample(name);
      setExpression(sample.logic);
      setLogicText(JSON.stringify(sample.logic, null, 2));
      setLogicError(null);
      setData(sample.data);
      setDataText(JSON.stringify(sample.data, null, 2));
      setDataError(null);
    }
  }, []);

  // Load first sample on mount
  useEffect(() => {
    const firstName = Object.keys(SAMPLE_EXPRESSIONS)[0];
    // eslint-disable-next-line react-hooks/set-state-in-effect -- Initialization on mount is intentional
    loadSample(firstName);
  }, [loadSample]);

  // Evaluate expression when inputs change
  /* eslint-disable react-hooks/set-state-in-effect -- Derived state computation from expression/data changes */
  useEffect(() => {
    if (!wasmReady || !expression || logicError || dataError) {
      setResult(undefined);
      setResultError(null);
      return;
    }

    try {
      const evalResult = evaluate(expression, data);
      setResult(evalResult);
      setResultError(null);
    } catch (err) {
      setResult(undefined);
      setResultError(err instanceof Error ? err.message : 'Evaluation failed');
    }
  }, [wasmReady, expression, data, logicError, dataError, evaluate]);
  /* eslint-enable react-hooks/set-state-in-effect */

  return (
    <div className="datalogic-playground" data-theme={theme}>
      {/* Header */}
      <div className="playground-header">
        <span className="playground-title">JSONLogic Playground</span>
        <div className="playground-controls">
          <select
            className="playground-examples"
            value={selectedExample}
            onChange={(e) => loadSample(e.target.value)}
          >
            <option value="" disabled>
              Load Example...
            </option>
            {Object.keys(SAMPLE_EXPRESSIONS).map((name) => (
              <option key={name} value={name}>
                {name}
              </option>
            ))}
          </select>
          <ModeSelector mode={editorMode} onChange={setEditorMode} />
        </div>
      </div>

      {/* Row 1: Logic, Data, Result columns */}
      <div className="playground-input-row">
        <div className="playground-column">
          <div className="playground-column-header">Logic</div>
          <JsonEditor
            value={logicText}
            onChange={handleLogicChange}
            hasError={!!logicError}
            placeholder="Enter JSONLogic expression..."
            className="playground-json-editor"
          />
          {logicError && <div className="playground-error">{logicError}</div>}
        </div>

        <div className="playground-column">
          <div className="playground-column-header">Data</div>
          <JsonEditor
            value={dataText}
            onChange={handleDataChange}
            hasError={!!dataError}
            placeholder="Enter JSON data..."
            className="playground-json-editor"
          />
          {dataError && <div className="playground-error">{dataError}</div>}
        </div>

        <div className="playground-column">
          <div className="playground-column-header">Result</div>
          <div className={`playground-result ${resultError ? 'has-error' : ''}`}>
            {resultError ? (
              <span className="playground-result-error">{resultError}</span>
            ) : wasmLoading ? (
              <span className="json-highlight json-null">Loading WASM...</span>
            ) : (
              <JsonHighlight value={result} placeholder="" />
            )}
          </div>
        </div>
      </div>

      {/* Row 2: Diagram */}
      <div className="playground-diagram-row">
        <DataLogicEditor
          value={expression}
          data={data}
          mode={editorMode}
          theme={theme}
        />
      </div>
    </div>
  );
}

// ============================================
// Utility Functions
// ============================================

/**
 * Detect the current theme from mdBook classes or system preference
 */
function detectTheme(): 'light' | 'dark' {
  // Check mdBook theme classes
  const htmlClasses = document.documentElement.classList;
  if (htmlClasses.contains('coal') || htmlClasses.contains('navy') || htmlClasses.contains('ayu')) {
    return 'dark';
  }
  if (htmlClasses.contains('light') || htmlClasses.contains('rust')) {
    return 'light';
  }

  // Fall back to system preference
  if (typeof window !== 'undefined' && window.matchMedia) {
    return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
  }

  return 'light';
}

/**
 * Parse data attributes from an element
 * Supports both data-logic/data-data and data-datalogic-logic/data-datalogic-data formats
 */
function parseDataAttributes(element: Element): WidgetProps {
  // Support both naming conventions
  const logicAttr = element.getAttribute('data-logic') || element.getAttribute('data-datalogic-logic');
  const dataAttr = element.getAttribute('data-data') || element.getAttribute('data-datalogic-data');
  const modeAttr = (element.getAttribute('data-mode') || element.getAttribute('data-datalogic-mode')) as 'visualize' | 'debug' | null;
  const heightAttr = element.getAttribute('data-height') || element.getAttribute('data-datalogic-height');
  const themeAttr = (element.getAttribute('data-theme') || element.getAttribute('data-datalogic-theme')) as 'light' | 'dark' | 'auto' | null;

  let logic: JsonLogicValue = {};
  if (logicAttr) {
    try {
      logic = JSON.parse(logicAttr);
    } catch {
      console.error('Invalid JSON in data-logic attribute:', logicAttr);
    }
  }

  let data: object = {};
  if (dataAttr) {
    try {
      data = JSON.parse(dataAttr);
    } catch {
      console.error('Invalid JSON in data-data attribute:', dataAttr);
    }
  }

  return {
    logic,
    data,
    mode: modeAttr || 'visualize',
    height: heightAttr || '400px',
    theme: themeAttr || 'auto',
  };
}

/**
 * Unmount a React root from an element
 */
function unmountElement(element: Element) {
  const root = mountedRoots.get(element);
  if (root) {
    root.unmount();
    mountedRoots.delete(element);
  }
}

// ============================================
// Public API
// ============================================

const DataLogicEmbed = {
  /**
   * Render a widget into an element
   */
  renderWidget(element: Element, props: Partial<WidgetProps> = {}) {
    // Unmount existing root if present
    unmountElement(element);

    // Parse attributes and merge with props
    const parsedProps = parseDataAttributes(element);
    const finalProps = { ...parsedProps, ...props };

    // Create root and render
    const root = ReactDOM.createRoot(element);
    mountedRoots.set(element, root);
    root.render(
      <React.StrictMode>
        <Widget {...finalProps} />
      </React.StrictMode>
    );
  },

  /**
   * Render the full playground into an element
   */
  renderPlayground(element: Element) {
    // Unmount existing root if present
    unmountElement(element);

    // Create root and render
    const root = ReactDOM.createRoot(element);
    mountedRoots.set(element, root);
    root.render(
      <React.StrictMode>
        <Playground />
      </React.StrictMode>
    );
  },

  /**
   * Auto-render all widgets on the page
   * Widgets: elements with [data-datalogic], [data-logic], or .playground-widget class
   * Playground: element with #datalogic-playground or [data-datalogic-playground]
   */
  renderWidgets() {
    // Render playground if present
    const playground = document.querySelector('#datalogic-playground, [data-datalogic-playground]');
    if (playground) {
      this.renderPlayground(playground);
    }

    // Render widgets - support multiple selector patterns
    const widgets = document.querySelectorAll('[data-datalogic]:not([data-datalogic-playground]), .playground-widget, [data-logic]:not([data-datalogic-playground])');
    widgets.forEach((widget) => {
      if (!widget.classList.contains('datalogic-initialized')) {
        this.renderWidget(widget);
      }
    });
  },

  /**
   * Initialize - render widgets and set up observers for page changes
   * Call this once after loading the script
   */
  init() {
    // Render existing widgets
    this.renderWidgets();

    // Watch for page changes (mdBook navigation)
    const content = document.getElementById('content');
    if (content) {
      const observer = new MutationObserver(() => {
        // Clean up unmounted widgets
        mountedRoots.forEach((root, element) => {
          if (!document.body.contains(element)) {
            root.unmount();
            mountedRoots.delete(element);
          }
        });

        // Render new widgets
        this.renderWidgets();
      });

      observer.observe(content, { childList: true, subtree: true });
    }
  },

  /**
   * Cleanup all mounted widgets
   */
  cleanup() {
    mountedRoots.forEach((root) => root.unmount());
    mountedRoots.clear();
  },
};

// Expose globally
declare global {
  interface Window {
    DataLogicEmbed: typeof DataLogicEmbed;
  }
}

window.DataLogicEmbed = DataLogicEmbed;

export default DataLogicEmbed;
