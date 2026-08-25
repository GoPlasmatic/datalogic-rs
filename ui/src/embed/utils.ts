import type { JsonLogicValue } from '../components/logic-editor';

export interface WidgetProps {
  logic: JsonLogicValue;
  /** Root evaluation context: any JSON value (object, array, or scalar). */
  data?: unknown;
  height?: string;
  theme?: 'light' | 'dark' | 'auto';
  /** Enable editing: node selection, properties panel, context menus */
  editable?: boolean;
  /** Start in templating mode (multi-key objects compile to output templates) */
  templating?: boolean;
}

export interface PlaygroundProps {
  /** Enable editing: node selection, properties panel, context menus */
  editable?: boolean;
  /** Start in templating mode (multi-key objects compile to output templates) */
  templating?: boolean;
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

function readAttribute(element: Element, ...names: string[]): string | null {
  for (const name of names) {
    const value = element.getAttribute(name);
    if (value !== null) return value;
  }
  return null;
}

/**
 * Parse data attributes from an element
 * Supports both data-logic/data-data and data-datalogic-logic/data-datalogic-data formats
 */
export function parseDataAttributes(element: Element): WidgetProps {
  // Support both naming conventions
  const logicAttr = readAttribute(element, 'data-logic', 'data-datalogic-logic');
  const dataAttr = readAttribute(element, 'data-data', 'data-datalogic-data');
  const heightAttr = readAttribute(element, 'data-height', 'data-datalogic-height');
  const themeAttr = readAttribute(element, 'data-theme', 'data-datalogic-theme') as
    | 'light'
    | 'dark'
    | 'auto'
    | null;
  const editableAttr = readAttribute(element, 'data-editable', 'data-datalogic-editable');
  const templatingAttr = readAttribute(element, 'data-templating', 'data-datalogic-templating');

  let logic: JsonLogicValue = {};
  if (logicAttr) {
    try {
      logic = JSON.parse(logicAttr);
    } catch {
      console.error('Invalid JSON in data-logic attribute:', logicAttr);
    }
  }

  let data: unknown = {};
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
    height: heightAttr || '400px',
    theme: themeAttr || 'auto',
    editable: editableAttr === 'true',
    templating: templatingAttr === 'true',
  };
}
