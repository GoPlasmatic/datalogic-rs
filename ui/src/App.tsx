import { useState, useCallback, useEffect, useRef, useMemo } from "react";
import { Sun, Moon, Monitor, BookOpen, ChevronDown, Link2, Check, Plus, Menu, X, Settings2 } from "lucide-react";
import { Tooltip } from "./components/Tooltip";

// GitHub icon (removed from lucide-react as a brand icon)
function GithubIcon({ size = 16 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="currentColor">
      <path d="M12 0C5.37 0 0 5.37 0 12c0 5.31 3.435 9.795 8.205 11.385.6.105.825-.255.825-.57 0-.285-.015-1.23-.015-2.235-3.015.555-3.795-.735-4.035-1.41-.135-.345-.72-1.41-1.23-1.695-.42-.225-1.02-.78-.015-.795.945-.015 1.62.87 1.845 1.23 1.08 1.815 2.805 1.305 3.495.99.105-.78.42-1.305.765-1.605-2.67-.3-5.46-1.335-5.46-5.925 0-1.305.465-2.385 1.23-3.225-.12-.3-.54-1.53.12-3.18 0 0 1.005-.315 3.3 1.23.96-.27 1.98-.405 3-.405s2.04.135 3 .405c2.295-1.56 3.3-1.23 3.3-1.23.66 1.65.24 2.88.12 3.18.765.84 1.23 1.905 1.23 3.225 0 4.605-2.805 5.625-5.475 5.925.435.375.81 1.095.81 2.22 0 1.605-.015 2.895-.015 3.3 0 .315.225.69.825.57A12.02 12.02 0 0 0 24 12c0-6.63-5.37-12-12-12z" />
    </svg>
  );
}
import { generateShareableUrl, parseShareableUrl } from "./utils/url-share";
import {
  DataLogicEditor,
  type JsonLogicValue,
  type DataLogicEvaluationConfig,
} from "./components/logic-editor";
import { DebugPanel, EngineSettingsPanel } from "./components/debug-panel";
import type { DebugError } from "./components/debug-panel";
import { MobileNav, type MobileTab } from "./components/mobile-nav/MobileNav";
import {
  useWasmEvaluator,
  DataLogicEvaluationError,
  summarizeEvaluationConfig,
  normalizeEvaluationConfig,
} from "./components/logic-editor/hooks";
import { useTheme, useIsMobile } from "./hooks";
import { SAMPLE_EXPRESSIONS } from "./constants/sample-expressions";
import "./App.css";

function App() {
  const { resolvedTheme, themePreference, setThemePreference, toggleTheme } = useTheme();

  const [logicText, setLogicText] = useState<string>("");
  const [expression, setExpression] = useState<JsonLogicValue | null>(null);
  const [logicError, setLogicError] = useState<string | null>(null);

  const [dataText, setDataText] = useState<string>("{}");
  // Any JSON value is a valid root context (object, array or scalar)
  const [data, setData] = useState<unknown>({});
  const [dataError, setDataError] = useState<string | null>(null);

  const [result, setResult] = useState<unknown>(undefined);
  const [resultError, setResultError] = useState<DebugError>(null);

  // Templating mode state: multi-key objects compile to output-shaping
  // templates with embedded JSONLogic. Matches the v5 core API
  // (`Engine::builder().with_templating(true)`).
  const [templating, setTemplating] = useState<boolean>(false);

  // Engine evaluation settings (presets, NaN / division-by-zero handling,
  // truthiness, numeric coercion, recursion cap). `{}` means engine defaults.
  const [engineConfig, setEngineConfig] = useState<DataLogicEvaluationConfig>({});
  const [engineSettingsOpen, setEngineSettingsOpen] = useState(false);
  const engineSettingsRef = useRef<HTMLDivElement>(null);
  const configSummary = useMemo(() => summarizeEvaluationConfig(engineConfig), [engineConfig]);

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

  // Mobile state
  const isMobile = useIsMobile();
  const [mobileTab, setMobileTab] = useState<MobileTab>('visual');
  const [overflowMenuOpen, setOverflowMenuOpen] = useState(false);
  const overflowMenuRef = useRef<HTMLDivElement>(null);

  const {
    ready: wasmReady,
    loading: wasmLoading,
    evaluate,
  } = useWasmEvaluator({ templating, config: engineConfig });

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

  // Update data when data text changes (any JSON value is accepted)
  const handleDataChange = useCallback((text: string) => {
    setDataText(text);

    if (!text.trim()) {
      setData({});
      setDataError(null);
      return;
    }

    try {
      setData(JSON.parse(text));
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

  // Load a sample expression (switching templating to match the sample)
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
      setTemplating(sample.templating ?? false);
      setExamplesDropdownOpen(false);
    }
  }, []);

  // Create a new empty project (also resets templating and engine settings)
  const handleNew = useCallback(() => {
    setLogicText('');
    setExpression(null);
    setLogicError(null);
    setDataText('{}');
    setData({});
    setDataError(null);
    setSelectedExample('');
    setTemplating(false);
    setEngineConfig({});
    // Clear URL params
    window.history.replaceState({}, '', window.location.pathname);
  }, []);

  // Share current state via URL (logic, data, templating, engine settings)
  const handleShare = useCallback(async () => {
    if (!expression) return;
    try {
      const url = generateShareableUrl(expression, data, {
        templating,
        config: normalizeEvaluationConfig(engineConfig),
      });
      await navigator.clipboard.writeText(url);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (err) {
      console.error('Failed to copy shareable URL:', err);
    }
  }, [expression, data, templating, engineConfig]);

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
      setData(shared.data);
      setDataText(JSON.stringify(shared.data, null, 2));
      if (shared.templating) setTemplating(true);
      if (shared.config) setEngineConfig(shared.config);
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

  // Close overflow menu when clicking outside
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (
        overflowMenuRef.current &&
        !overflowMenuRef.current.contains(event.target as Node)
      ) {
        setOverflowMenuOpen(false);
      }
    };

    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, []);

  // Close the engine settings popover when clicking outside (desktop anchor)
  useEffect(() => {
    if (!engineSettingsOpen || isMobile) return;
    const handleClickOutside = (event: MouseEvent) => {
      if (
        engineSettingsRef.current &&
        !engineSettingsRef.current.contains(event.target as Node)
      ) {
        setEngineSettingsOpen(false);
      }
    };
    document.addEventListener("mousedown", handleClickOutside);
    return () => document.removeEventListener("mousedown", handleClickOutside);
  }, [engineSettingsOpen, isMobile]);

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
      if (err instanceof DataLogicEvaluationError) {
        setResultError(err.structured);
      } else {
        setResultError(err instanceof Error ? err.message : typeof err === 'string' ? err : "Evaluation failed");
      }
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

  // Surface a few examples as quick-action chips in the empty state.
  const exampleSuggestions = useMemo(
    () => Object.keys(SAMPLE_EXPRESSIONS).slice(0, 4),
    [],
  );

  const openEngineSettings = useCallback(() => {
    setOverflowMenuOpen(false);
    setEngineSettingsOpen(true);
  }, []);

  const closeEngineSettings = useCallback(() => setEngineSettingsOpen(false), []);

  const debugPanelElement = (
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
      accordion={isMobile}
      configSummary={configSummary}
      onOpenEngineSettings={openEngineSettings}
    />
  );

  const visualEditorElement = (
    <DataLogicEditor
      value={expression}
      onChange={handleExpressionChange}
      data={data}
      theme={resolvedTheme}
      templating={templating}
      onTemplatingChange={setTemplating}
      config={engineConfig}
      editable
      exampleSuggestions={exampleSuggestions}
      onSelectExample={loadSample}
    />
  );

  const themeOptions = [
    { value: 'light' as const, label: 'Light', Icon: Sun },
    { value: 'system' as const, label: 'System', Icon: Monitor },
    { value: 'dark' as const, label: 'Dark', Icon: Moon },
  ];

  return (
    <div className="app">
      <header className="app-header">
        <div className="header-title">
          <span className="brand-mark" aria-hidden="true" />
          <div className="brand-text">
            <h1>DataLogic Studio</h1>
            <span className="brand-eyebrow">JSONLogic · Visual Debugger</span>
          </div>
        </div>
        <div className="header-controls">
          <Tooltip label="Start a new project">
            <button
              className="new-button header-desktop-only"
              onClick={handleNew}
            >
              <Plus size={16} />
              <span>New</span>
            </button>
          </Tooltip>
          <div className="examples-dropdown header-desktop-only" ref={examplesDropdownRef}>
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
                    {SAMPLE_EXPRESSIONS[name].templating && (
                      <span className="examples-dropdown-tag">templating</span>
                    )}
                  </button>
                ))}
              </div>
            )}
          </div>
          <div className="header-divider" />
          <div className="header-links">
            <Tooltip label="View on GitHub">
              <a
                href="https://github.com/GoPlasmatic/datalogic-rs"
                target="_blank"
                rel="noopener noreferrer"
                className="header-link"
                aria-label="GitHub repository"
              >
                <GithubIcon size={16} />
                <span>GitHub</span>
              </a>
            </Tooltip>
            <Tooltip label="Documentation">
              <a
                href="https://goplasmatic.github.io/datalogic-rs/"
                target="_blank"
                rel="noopener noreferrer"
                className="header-link"
                aria-label="DataLogic documentation"
              >
                <BookOpen size={16} />
                <span>Docs</span>
              </a>
            </Tooltip>
          </div>
          <div className="header-divider" />

          {/* Engine settings (desktop anchor; on mobile the sheet is opened from the overflow menu) */}
          <div className="engine-settings-anchor header-desktop-only" ref={engineSettingsRef}>
            <Tooltip label={configSummary ? `Engine settings: ${configSummary}` : 'Engine settings (evaluation semantics)'}>
              <button
                type="button"
                className={`engine-settings-trigger ${configSummary ? 'is-active' : ''}`}
                onClick={() => setEngineSettingsOpen((open) => !open)}
                aria-expanded={engineSettingsOpen}
                aria-haspopup="dialog"
              >
                <Settings2 size={16} />
                <span>Engine</span>
                {configSummary && <span className="engine-settings-dot" aria-hidden="true" />}
              </button>
            </Tooltip>
            {engineSettingsOpen && !isMobile && (
              <EngineSettingsPanel
                config={engineConfig}
                onChange={setEngineConfig}
                onClose={closeEngineSettings}
                variant="popover"
              />
            )}
          </div>

          <Tooltip label="Copy shareable link (includes data, templating and engine settings)">
            <button
              className="share-button header-desktop-only"
              onClick={handleShare}
              disabled={!expression || !!logicError}
            >
              {copied ? <Check size={16} /> : <Link2 size={16} />}
              <span>{copied ? 'Copied!' : 'Share'}</span>
            </button>
          </Tooltip>

          {/* Three-way theme switch (desktop) */}
          <div className="theme-switch header-desktop-only" role="radiogroup" aria-label="Theme">
            {themeOptions.map(({ value, label, Icon }) => (
              <Tooltip key={value} label={label}>
                <button
                  type="button"
                  role="radio"
                  aria-checked={themePreference === value}
                  className={`theme-switch-option ${themePreference === value ? 'is-active' : ''}`}
                  onClick={() => setThemePreference(value)}
                >
                  <Icon size={15} />
                </button>
              </Tooltip>
            ))}
          </div>

          {/* Mobile binary fallback (hidden on desktop via .header-desktop-only inversion) */}
          <button
            className="theme-toggle theme-toggle--mobile-only"
            onClick={toggleTheme}
            aria-label={`Switch to ${resolvedTheme === "light" ? "dark" : "light"} mode`}
          >
            {resolvedTheme === "light" ? <Moon size={18} /> : <Sun size={18} />}
          </button>
          {/* Mobile overflow menu: holds actions that don't fit in the compact header */}
          <div className="overflow-menu" ref={overflowMenuRef}>
            <button
              className="overflow-menu-trigger"
              onClick={() => setOverflowMenuOpen(!overflowMenuOpen)}
              aria-label="More options"
            >
              {overflowMenuOpen ? <X size={20} /> : <Menu size={20} />}
            </button>
            {overflowMenuOpen && (
              <div className="overflow-menu-dropdown">
                <button
                  className="overflow-menu-item"
                  onClick={() => { handleNew(); setOverflowMenuOpen(false); }}
                >
                  <Plus size={16} />
                  <span>New Project</span>
                </button>
                <button
                  className="overflow-menu-item"
                  onClick={() => { handleShare(); setOverflowMenuOpen(false); }}
                  disabled={!expression || !!logicError}
                >
                  {copied ? <Check size={16} /> : <Link2 size={16} />}
                  <span>{copied ? 'Copied!' : 'Share Link'}</span>
                </button>
                <button
                  className={`overflow-menu-item ${configSummary ? 'overflow-menu-item--active' : ''}`}
                  onClick={openEngineSettings}
                >
                  <Settings2 size={16} />
                  <span>Engine Settings{configSummary ? ' (custom)' : ''}</span>
                </button>
                <div className="overflow-menu-divider" />
                <div className="overflow-menu-label">Theme</div>
                <div className="overflow-menu-theme" role="radiogroup" aria-label="Theme">
                  {themeOptions.map(({ value, label, Icon }) => (
                    <button
                      key={value}
                      type="button"
                      role="radio"
                      aria-checked={themePreference === value}
                      className={`overflow-menu-theme-option ${themePreference === value ? 'is-active' : ''}`}
                      onClick={() => setThemePreference(value)}
                    >
                      <Icon size={15} />
                      <span>{label}</span>
                    </button>
                  ))}
                </div>
                <div className="overflow-menu-divider" />
                <div className="overflow-menu-label">Examples</div>
                {Object.keys(SAMPLE_EXPRESSIONS).map((name) => (
                  <button
                    key={name}
                    className={`overflow-menu-item ${name === selectedExample ? 'overflow-menu-item--active' : ''}`}
                    onClick={() => { loadSample(name); setOverflowMenuOpen(false); }}
                  >
                    <span>{name}</span>
                  </button>
                ))}
                <div className="overflow-menu-divider" />
                <a
                  href="https://github.com/GoPlasmatic/datalogic-rs"
                  target="_blank"
                  rel="noopener noreferrer"
                  className="overflow-menu-item"
                  onClick={() => setOverflowMenuOpen(false)}
                >
                  <GithubIcon size={16} />
                  <span>GitHub</span>
                </a>
                <a
                  href="https://goplasmatic.github.io/datalogic-rs/"
                  target="_blank"
                  rel="noopener noreferrer"
                  className="overflow-menu-item"
                  onClick={() => setOverflowMenuOpen(false)}
                >
                  <BookOpen size={16} />
                  <span>Docs</span>
                </a>
              </div>
            )}
          </div>
        </div>
      </header>

      {/* Mobile: engine settings as a centered sheet with a backdrop */}
      {engineSettingsOpen && isMobile && (
        <>
          <div className="engine-settings-backdrop" onClick={closeEngineSettings} />
          <EngineSettingsPanel
            config={engineConfig}
            onChange={setEngineConfig}
            onClose={closeEngineSettings}
            variant="sheet"
          />
        </>
      )}

      {isMobile ? (
        <>
          <main className="app-main app-main--mobile">
            {mobileTab === 'visual' && (
              <div className="panel visual-panel mobile-panel">
                {visualEditorElement}
              </div>
            )}
            {mobileTab === 'code' && (
              <div className="panel debug-input-panel mobile-panel">
                {debugPanelElement}
              </div>
            )}
          </main>
          <MobileNav activeTab={mobileTab} onTabChange={setMobileTab} />
        </>
      ) : (
        <main className="app-main" ref={containerRef}>
          <div className="panel debug-input-panel" style={{ width: panelWidth }}>
            {debugPanelElement}
          </div>
          <div
            className={`divider ${isDragging ? "dragging" : ""}`}
            onMouseDown={handleMouseDown}
          />
          <div className="panel visual-panel">
            {visualEditorElement}
          </div>
        </main>
      )}
    </div>
  );
}

export default App;
