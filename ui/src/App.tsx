import { useState, useCallback, useEffect, useRef } from "react";
import { Sun, Moon, Github, BookOpen, ChevronDown } from "lucide-react";
import {
  DataLogicEditor,
  type JsonLogicValue,
  type DataLogicEditorMode,
} from "./components/logic-editor";
import { DebugPanel } from "./components/debug-panel";
import { ModeSelector } from "./components/mode-selector";
import { useWasmEvaluator } from "./components/logic-editor/hooks";
import { useTheme } from "./hooks";
import "./App.css";

// Sample JSONLogic expressions for testing - organized by visual complexity
const SAMPLE_EXPRESSIONS: Record<
  string,
  { logic: JsonLogicValue; data: object }
> = {
  // ============================================
  // Tier 1: Medium Complexity (4-8 nodes)
  // ============================================

  // String concatenation with conditionals
  "Greeting Builder": {
    logic: {
      cat: [
        { if: [{ var: "formal" }, "Dear ", "Hi "] },
        { var: "title" },
        " ",
        { var: "name" },
        { if: [{ var: "formal" }, ",", "!"] },
      ],
    },
    data: { name: "Smith", title: "Dr.", formal: true },
  },

  // Arithmetic chain
  "Discount Price": {
    logic: {
      "*": [
        { var: "price" },
        { "-": [1, { "/": [{ var: "discountPercent" }, 100] }] },
      ],
    },
    data: { price: 150, discountPercent: 25 },
  },

  // And/Or logic branching
  "Age Validation": {
    logic: {
      and: [
        { ">=": [{ var: "age" }, 18] },
        { "<=": [{ var: "age" }, 65] },
        {
          or: [
            { "==": [{ var: "hasID" }, true] },
            { "==": [{ var: "hasPassport" }, true] },
          ],
        },
      ],
    },
    data: { age: 30, hasID: true, hasPassport: false },
  },

  // Basic conditional branching
  "Pass or Fail": {
    logic: {
      if: [
        { ">=": [{ var: "score" }, 60] },
        { cat: ["Passed with score: ", { var: "score" }] },
        {
          cat: [
            "Failed. Need ",
            { "-": [60, { var: "score" }] },
            " more points",
          ],
        },
      ],
    },
    data: { score: 45 },
  },

  // ============================================
  // Tier 2: High Complexity (8-15 nodes)
  // ============================================

  // Multi-branch if/else
  "Grade Calculator": {
    logic: {
      if: [
        { ">=": [{ var: "score" }, 90] },
        "A - Excellent",
        { ">=": [{ var: "score" }, 80] },
        "B - Good",
        { ">=": [{ var: "score" }, 70] },
        "C - Average",
        { ">=": [{ var: "score" }, 60] },
        "D - Below Average",
        "F - Fail",
      ],
    },
    data: { score: 78 },
  },

  // Array iteration
  "Map - Double": {
    logic: {
      map: [{ var: "numbers" }, { "*": [{ var: "" }, 2] }],
    },
    data: { numbers: [1, 2, 3, 4, 5] },
  },

  // Array filtering
  "Filter - Above Threshold": {
    logic: {
      filter: [
        { var: "numbers" },
        { ">": [{ var: "" }, { val: [[-1], "threshold"] }] },
      ],
    },
    data: { numbers: [10, 25, 5, 30, 15, 8], threshold: 12 },
  },

  // Array aggregation
  "Reduce - Sum": {
    logic: {
      reduce: [
        { var: "items" },
        { "+": [{ var: "accumulator" }, { var: "current" }] },
        0,
      ],
    },
    data: { items: [10, 20, 30, 40] },
  },

  // ============================================
  // Tier 3: Very High Complexity (15+ nodes)
  // ============================================

  // Multi-branch conditionals
  "Shipping Calculator": {
    logic: {
      if: [
        { ">=": [{ var: "order.total" }, 100] },
        0,
        { "==": [{ var: "order.shipping" }, "express"] },
        { "+": [10, { "*": [{ var: "order.weight" }, 2] }] },
        { "==": [{ var: "order.shipping" }, "standard"] },
        { "+": [5, { "*": [{ var: "order.weight" }, 0.5] }] },
        { "*": [{ var: "order.weight" }, 0.25] },
      ],
    },
    data: { order: { total: 75, shipping: "express", weight: 5 } },
  },

  // Nested reduce + arithmetic
  "Order Total": {
    logic: {
      "*": [
        {
          reduce: [
            { var: "cart.items" },
            {
              "+": [
                { var: "accumulator" },
                {
                  "*": [{ var: "current.price" }, { var: "current.quantity" }],
                },
              ],
            },
            0,
          ],
        },
        { "-": [1, { "/": [{ var: "cart.discountPercent" }, 100] }] },
      ],
    },
    data: {
      cart: {
        items: [
          { name: "Widget", price: 25, quantity: 2 },
          { name: "Gadget", price: 50, quantity: 1 },
          { name: "Gizmo", price: 15, quantity: 3 },
        ],
        discountPercent: 10,
      },
    },
  },

  // Deep nested and/or logic
  "Loan Eligibility": {
    logic: {
      and: [
        { ">=": [{ var: "applicant.age" }, 21] },
        { "<=": [{ var: "applicant.age" }, 65] },
        {
          or: [
            {
              and: [
                { ">=": [{ var: "applicant.income" }, 50000] },
                { ">=": [{ var: "applicant.creditScore" }, 700] },
              ],
            },
            {
              and: [
                { ">=": [{ var: "applicant.income" }, 100000] },
                { ">=": [{ var: "applicant.creditScore" }, 600] },
                { "==": [{ var: "applicant.hasCollateral" }, true] },
              ],
            },
          ],
        },
        {
          "<": [
            {
              "/": [
                { var: "applicant.existingDebt" },
                { var: "applicant.income" },
              ],
            },
            0.4,
          ],
        },
      ],
    },
    data: {
      applicant: {
        age: 35,
        income: 75000,
        creditScore: 720,
        existingDebt: 20000,
        hasCollateral: false,
      },
    },
  },

  // Parallel array predicates
  "Inventory Check": {
    logic: {
      and: [
        { all: [{ var: "products" }, { ">": [{ var: "stock" }, 0] }] },
        { some: [{ var: "products" }, { ">=": [{ var: "stock" }, 100] }] },
        { none: [{ var: "products" }, { "<": [{ var: "price" }, 0] }] },
      ],
    },
    data: {
      products: [
        { name: "A", stock: 50, price: 10 },
        { name: "B", stock: 150, price: 25 },
        { name: "C", stock: 5, price: 100 },
      ],
    },
  },

  // ============================================
  // Special: Structure Mode
  // ============================================

  // preserveStructure mode - JSON template output (requires "Preserve Structure" checkbox)
  "Party Template (Structure)": {
    logic: {
      if: [
        { and: [{ "!": { var: "BICFI" } }, { var: "ClrSysMmbId.MmbId" }] },
        {
          party_identifier: {
            cat: [
              "//",
              {
                if: [
                  { var: "ClrSysMmbId.ClrSysId.Cd" },
                  { var: "ClrSysMmbId.ClrSysId.Cd" },
                  "",
                ],
              },
              { var: "ClrSysMmbId.MmbId" },
            ],
          },
          name_and_address: [],
        },
        null,
      ],
    },
    data: {
      BICFI: "",
      ClrSysMmbId: {
        MmbId: "12345",
        ClrSysId: { Cd: "USABA" },
      },
    },
  },
};

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

  // Editor mode state
  const [editorMode, setEditorMode] = useState<DataLogicEditorMode>("debug");

  // Preserve structure mode state
  const [preserveStructure, setPreserveStructure] = useState<boolean>(false);

  // Examples dropdown state
  const [selectedExample, setSelectedExample] = useState<string>(
    Object.keys(SAMPLE_EXPRESSIONS)[0],
  );
  const [examplesDropdownOpen, setExamplesDropdownOpen] = useState(false);
  const examplesDropdownRef = useRef<HTMLDivElement>(null);

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

  // Load first sample on mount
  useEffect(() => {
    const firstSample = Object.keys(SAMPLE_EXPRESSIONS)[0];
    // eslint-disable-next-line react-hooks/set-state-in-effect -- Initialization on mount is intentional
    loadSample(firstSample);
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
          <h1>DataLogic Debugger</h1>
          <span className="header-subtitle">Visual JSONLogic Debugger</span>
        </div>
        <div className="header-controls">
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
          <div className="panel-header">
            <h2>Visual Flow</h2>
            <div className="panel-header-controls">
              <label className="preserve-structure-toggle">
                <input
                  type="checkbox"
                  checked={preserveStructure}
                  onChange={(e) => setPreserveStructure(e.target.checked)}
                />
                <span>Preserve Structure</span>
              </label>
              <ModeSelector mode={editorMode} onChange={setEditorMode} />
            </div>
          </div>
          <div className="panel-content">
            <DataLogicEditor
              value={expression}
              onChange={handleExpressionChange}
              data={data}
              mode={editorMode}
              theme={theme}
              preserveStructure={preserveStructure}
            />
          </div>
        </div>
      </main>
    </div>
  );
}

export default App;
