/**
 * Operator Configuration Types
 *
 * This file defines the TypeScript interfaces for the operator configuration
 * that serves as the single source of truth for all operator documentation
 * and UI rendering.
 */

// ============================================================================
// Category Types
// ============================================================================

export type OperatorCategory =
  | 'variable'
  | 'comparison'
  | 'logical'
  | 'arithmetic'
  | 'control'
  | 'string'
  | 'array'
  | 'datetime'
  | 'validation'
  | 'error'
  | 'utility';

// ============================================================================
// Arity Types
// ============================================================================

export type ArityType =
  | 'nullary'    // 0 args (e.g., now)
  | 'unary'      // 1 arg (e.g., !, abs)
  | 'binary'     // 2 args (e.g., /, %)
  | 'ternary'    // 3 args (e.g., ?:, reduce)
  | 'nary'       // 1+ args (e.g., +, cat)
  | 'variadic'   // 2+ args (e.g., *, and)
  | 'chainable'  // 2+ args with chaining (e.g., <, >)
  | 'range'      // min-max range (e.g., substr 2-3)
  | 'special';   // Custom structure (e.g., if, val)

export type ArgType =
  | 'any'
  | 'number'
  | 'string'
  | 'boolean'
  | 'array'
  | 'object'
  | 'expression'
  | 'path'
  | 'datetime'
  | 'duration';

export interface ArgSpec {
  name: string;
  label: string;
  description?: string;
  type?: ArgType;
  required?: boolean;
  repeatable?: boolean;
}

export interface AritySpec {
  type: ArityType;
  min?: number;
  max?: number;
  args?: ArgSpec[];
}

// ============================================================================
// Help Types
// ============================================================================

export type ReturnType =
  | 'any'
  | 'number'
  | 'string'
  | 'boolean'
  | 'array'
  | 'object'
  | 'null'
  | 'datetime'
  | 'duration'
  | 'number | string'
  | 'same'
  | 'never';

export interface OperatorExample {
  title: string;
  rule: unknown;
  data?: unknown;
  result?: unknown;
  error?: { type: string };
  note?: string;
}

export interface OperatorHelp {
  summary: string;
  details?: string;
  returnType: ReturnType;
  examples: OperatorExample[];
  notes?: string[];
  seeAlso?: string[];
}

// ============================================================================
// UI Hints Types
// ============================================================================

export type NodeType =
  | 'operator'
  | 'variable'
  | 'literal'
  | 'decision'
  | 'vertical'
  | 'iterator'
  | 'structure';

export interface OperatorUIHints {
  icon?: string;
  shortLabel?: string;
  nodeType?: NodeType;
  inlineEditable?: boolean;
  showArgLabels?: boolean;
  collapsible?: boolean;
  scopeJump?: boolean;
  metadata?: boolean;
  datetimeProps?: boolean;
  iteratorContext?: boolean;
  addArgumentLabel?: string; // Custom label for add argument button (e.g., "Add Else If", "Add Default")
}

// ============================================================================
// Main Operator Type
// ============================================================================

export interface Operator {
  name: string;
  label: string;
  category: OperatorCategory;
  description: string;
  arity: AritySpec;
  help: OperatorHelp;
  ui?: OperatorUIHints;
  panel?: PanelConfig;
}

// ============================================================================
// Config Types
// ============================================================================

export interface OperatorConfig {
  version: string;
  operators: Record<string, Operator>;
}

export interface CategoryMeta {
  name: OperatorCategory;
  label: string;
  description: string;
  color: string;
  icon: string;
}

// ============================================================================
// Panel Configuration Types
// ============================================================================

/**
 * Input widget types for panel fields
 */
export type PanelInputType =
  | 'text'
  | 'textarea'
  | 'number'
  | 'boolean'
  | 'select'
  | 'path'
  | 'pathArray'
  | 'expression'
  | 'json';

/**
 * Visibility condition for conditional fields
 */
export interface VisibilityCondition {
  field: string;
  operator: 'equals' | 'notEquals' | 'exists' | 'notExists';
  value?: unknown;
}

/**
 * Select dropdown option
 */
export interface SelectOption {
  value: string | number | boolean;
  label: string;
  description?: string;
}

/**
 * Panel field configuration
 */
export interface PanelField {
  id: string;
  label: string;
  inputType: PanelInputType;
  helpText?: string;
  placeholder?: string;
  required?: boolean;
  defaultValue?: unknown;
  options?: SelectOption[];
  showWhen?: VisibilityCondition[];
  min?: number;
  max?: number;
  repeatable?: boolean;
}

/**
 * Panel section grouping
 */
export interface PanelSection {
  id: string;
  title?: string;
  fields: PanelField[];
  defaultCollapsed?: boolean;
  showWhen?: VisibilityCondition[];
}

/**
 * Iterator context variable
 */
export interface ContextVariable {
  name: string;
  label: string;
  description: string;
  accessor: 'var' | 'val';
  example: string;
}

/**
 * Complete panel configuration
 */
export interface PanelConfig {
  sections: PanelSection[];
  contextVariables?: ContextVariable[];
  chainable?: boolean;
}
