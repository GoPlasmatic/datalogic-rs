import { useState, useCallback, useEffect, useRef } from "react";
import { Sun, Moon, Github, BookOpen, ChevronDown, Link2, Check, Plus } from "lucide-react";
import { generateShareableUrl, parseShareableUrl } from "./utils/url-share";
import {
  DataLogicEditor,
  type JsonLogicValue,
} from "./components/logic-editor";
import { DebugPanel } from "./components/debug-panel";
import { useWasmEvaluator } from "./components/logic-editor/hooks";
import { useTheme } from "./hooks";
import { SAMPLE_EXPRESSIONS } from "./constants/sample-expressions";
import "./App.css";

function App() {
  const { theme, toggleTheme } = useTheme();

  const [logicText, setLogicText] = useState<string>("");
  const [expression, setExpression] = useState<JsonLogicValue | null>(null);
  const [logicError, setLogicError] = useState<string | null>(null);

  const [dataText, setDataText] = useState<string>("{}");
  const [data, setData] = useState<object>({});
  const [dataError, setDataError] = useState<string | null>(null);

  const [result, setResult] = useState<unknown>(undefined);
  const [resultError, setResultError] = useState<string | null>(null);

  // Preserve structure mode state
  const [preserveStructure, setPreserveStructure] = useState<boolean>(false);

  // Examples dropdown state
  const [selectedExample, setSelectedExample] = useState<string>(
    Object.keys(SAMPLE_EXPRESSIONS)[0],
  );
  const [examplesDropdownOpen, setExamplesDropdownOpen] = useState(false);
  const examplesDropdownRef = useRef<HTMLDivElement>(null);

  // URL sharing state
  const [copied, setCopied] = useState(false);
  const initializedRef = useRef(false);

  // Resizable panel state
  const [panelWidth, setPanelWidth] = useState<number>(350);
  const [isDragging, setIsDragging] = useState(false);
  const containerRef = useRef<HTMLElement>(null);

  const {
    ready: wasmReady,
    loading: wasmLoading,
    evaluate,
  } = useWasmEvaluator({ preserveStructure });

  // Update expression when logic text changes
  const handleLogicChange = useCallback((text: string) => {
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
      setLogicError(err instanceof Error ? err.message : "Invalid JSON");
    }
  }, []);

  // Update data when data text changes
  const handleDataChange = useCallback((text: string) => {
    setDataText(text);

    if (!text.trim()) {
      setData({});
      setDataError(null);
      return;
    }

    try {
      const parsed = JSON.parse(text);
      if (
        typeof parsed !== "object" ||
        parsed === null ||
        Array.isArray(parsed)
      ) {
        setDataError("Data must be a JSON object");
        return;
      }
      setData(parsed);
      setDataError(null);
    } catch (err) {
      setDataError(err instanceof Error ? err.message : "Invalid JSON");
    }
  }, []);

  // Update expression state (for onChange callback)
  const handleExpressionChange = useCallback(
    (newExpr: JsonLogicValue | null) => {
      setExpression(newExpr);
      setLogicText(newExpr !== null ? JSON.stringify(newExpr, null, 2) : "");
      setLogicError(null);
    },
    [],
  );

  // Load a sample expression
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
      setExamplesDropdownOpen(false);
    }
  }, []);

  // Create a new empty project
  const handleNew = useCallback(() => {
    setLogicText('');
    setExpression(null);
    setLogicError(null);
    setDataText('{}');
    setData({});
    setDataError(null);
    setSelectedExample('');
    // Clear URL params
    window.history.replaceState({}, '', window.location.pathname);
  }, []);

  // Share current state via URL
  const handleShare = useCallback(async () => {
    if (!expression) return;
    try {
      const url = generateShareableUrl(expression, data, preserveStructure);
      await navigator.clipboard.writeText(url);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      console.error('Failed to copy shareable URL:', err);
    }
  }, [expression, data, preserveStructure]);

  // Load from URL or first sample on mount
  useEffect(() => {
    // Prevent double initialization in StrictMode
    if (initializedRef.current) return;
    initializedRef.current = true;

    const shared = parseShareableUrl();
    if (shared) {
      // eslint-disable-next-line react-hooks/set-state-in-effect -- Initialization on mount is intentional
      setExpression(shared.logic as JsonLogicValue);
      setLogicText(JSON.stringify(shared.logic, null, 2));
      setData(shared.data as object);
      setDataText(JSON.stringify(shared.data, null, 2));
      if (shared.preserveStructure) setPreserveStructure(true);
      // Clear the URL parameter after loading
      window.history.replaceState({}, '', window.location.pathname);
    } else {
      const firstSample = Object.keys(SAMPLE_EXPRESSIONS)[0];
      loadSample(firstSample);
    }
  }, [loadSample]);

  // Close dropdown when clicking outside
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (
        examplesDropdownRef.current &&
        !examplesDropdownRef.current.contains(event.target as Node)
      ) {
        setExamplesDropdownOpen(false);
      }
    };

    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, []);

  // Evaluate the expression when inputs change
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
      setResultError(err instanceof Error ? err.message : "Evaluation failed");
    }
  }, [wasmReady, expression, data, logicError, dataError, evaluate]);
  /* eslint-enable react-hooks/set-state-in-effect */

  // Handle divider dragging
  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    setIsDragging(true);
  }, []);

  useEffect(() => {
    if (!isDragging) return;

    const handleMouseMove = (e: MouseEvent) => {
      if (containerRef.current) {
        const containerRect = containerRef.current.getBoundingClientRect();
        const newWidth = e.clientX - containerRect.left;
        // Constrain between 200px and 600px
        setPanelWidth(Math.max(200, Math.min(600, newWidth)));
      }
    };

    const handleMouseUp = () => {
      setIsDragging(false);
    };

    document.addEventListener("mousemove", handleMouseMove);
    document.addEventListener("mouseup", handleMouseUp);

    return () => {
      document.removeEventListener("mousemove", handleMouseMove);
      document.removeEventListener("mouseup", handleMouseUp);
    };
  }, [isDragging]);

  return (
    <div className="app">
      <header className="app-header">
        <div className="header-title">
          <h1>DataLogic Studio</h1>
          <span className="header-subtitle">Visual JSONLogic Editor & Debugger</span>
        </div>
        <div className="header-controls">
          <button
            className="new-button"
            onClick={handleNew}
            title="Start a new project"
          >
            <Plus size={16} />
            <span>New</span>
          </button>
          <div className="examples-dropdown" ref={examplesDropdownRef}>
            <button
              className="examples-dropdown-trigger"
              onClick={() => setExamplesDropdownOpen(!examplesDropdownOpen)}
              aria-expanded={examplesDropdownOpen}
              aria-haspopup="listbox"
            >
              <span className="examples-dropdown-label">Examples</span>
              <span className="examples-dropdown-value">{selectedExample}</span>
              <ChevronDown
                size={14}
                className={`examples-dropdown-icon ${examplesDropdownOpen ? "open" : ""}`}
              />
            </button>
            {examplesDropdownOpen && (
              <div className="examples-dropdown-menu" role="listbox">
                {Object.keys(SAMPLE_EXPRESSIONS).map((name) => (
                  <button
                    key={name}
                    className={`examples-dropdown-item ${name === selectedExample ? "selected" : ""}`}
                    onClick={() => loadSample(name)}
                    role="option"
                    aria-selected={name === selectedExample}
                  >
                    {name}
                  </button>
                ))}
              </div>
            )}
          </div>
          <div className="header-divider" />
          <div className="header-links">
            <a
              href="https://github.com/GoPlasmatic/datalogic-rs"
              target="_blank"
              rel="noopener noreferrer"
              className="header-link"
              title="DataLogic GitHub Repository"
            >
              <Github size={16} />
              <span>GitHub</span>
            </a>
            <a
              href="https://goplasmatic.github.io/datalogic-rs/"
              target="_blank"
              rel="noopener noreferrer"
              className="header-link"
              title="DataLogic Documentation"
            >
              <BookOpen size={16} />
              <span>Docs</span>
            </a>
          </div>
          <div className="header-divider" />
          <button
            className="share-button"
            onClick={handleShare}
            disabled={!expression || !!logicError}
            title="Copy shareable link"
          >
            {copied ? <Check size={16} /> : <Link2 size={16} />}
            <span>{copied ? 'Copied!' : 'Share'}</span>
          </button>
          <button
            className="theme-toggle"
            onClick={toggleTheme}
            title={`Switch to ${theme === "light" ? "dark" : "light"} mode`}
          >
            {theme === "light" ? <Moon size={18} /> : <Sun size={18} />}
          </button>
        </div>
      </header>

      <main className="app-main" ref={containerRef}>
        {/* Left Panel - Debug Panel with Logic/Data inputs */}
        <div className="panel debug-input-panel" style={{ width: panelWidth }}>
          <DebugPanel
            logic={expression}
            logicText={logicText}
            onLogicChange={handleLogicChange}
            logicError={logicError}
            data={data}
            dataText={dataText}
            onDataChange={handleDataChange}
            dataError={dataError}
            result={result}
            resultError={resultError}
            wasmReady={wasmReady}
            wasmLoading={wasmLoading}
          />
        </div>

        {/* Divider */}
        <div
          className={`divider ${isDragging ? "dragging" : ""}`}
          onMouseDown={handleMouseDown}
        />

        {/* Right Panel - Visual Flow */}
        <div className="panel visual-panel">
          <DataLogicEditor
            value={expression}
            onChange={handleExpressionChange}
            data={data}
            theme={theme}
            preserveStructure={preserveStructure}
            onPreserveStructureChange={setPreserveStructure}
            editable
          />
        </div>
      </main>
    </div>
  );
}

export default App;
