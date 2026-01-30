import type { JsonLogicValue } from '../components/logic-editor';

export interface WidgetProps {
  logic: JsonLogicValue;
  data?: object;
  height?: string;
  theme?: 'light' | 'dark' | 'auto';
  /** Enable editing: node selection, properties panel, context menus */
  editable?: boolean;
  /** @deprecated Ignored. Kept for backward compat. */
  mode?: 'visualize' | 'debug';
  /** @deprecated Ignored. */
  componentMode?: 'debugger' | 'visualizer';
}

export interface PlaygroundProps {
  /** Enable editing: node selection, properties panel, context menus */
  editable?: boolean;
  /** @deprecated Ignored. */
  componentMode?: 'debugger' | 'visualizer';
}

/**
 * Detect the current theme from mdBook classes or system preference
 */
export function detectTheme(): 'light' | 'dark' {
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
export function parseDataAttributes(element: Element): WidgetProps {
  // Support both naming conventions
  const logicAttr = element.getAttribute('data-logic') || element.getAttribute('data-datalogic-logic');
  const dataAttr = element.getAttribute('data-data') || element.getAttribute('data-datalogic-data');
  const modeAttr = element.getAttribute('data-mode') || element.getAttribute('data-datalogic-mode');
  const heightAttr = element.getAttribute('data-height') || element.getAttribute('data-datalogic-height');
  const themeAttr = (element.getAttribute('data-theme') || element.getAttribute('data-datalogic-theme')) as 'light' | 'dark' | 'auto' | null;
  const editableAttr = element.getAttribute('data-editable');

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

  // Backward compat: data-mode="edit" -> editable=true
  const editable = editableAttr === 'true' || modeAttr === 'edit';

  return {
    logic,
    data,
    height: heightAttr || '400px',
    theme: themeAttr || 'auto',
    editable,
  };
}
