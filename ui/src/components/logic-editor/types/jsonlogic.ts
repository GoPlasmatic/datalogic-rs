// JSONLogic type definitions

// Primitive values that can appear in JSONLogic
export type JsonLogicPrimitive = string | number | boolean | null;

// Variable reference: { "var": "path.to.value" } or { "var": ["path", "default"] }
export interface JsonLogicVar {
  var: string | [string, JsonLogicValue];
}

// Val reference: { "val": "path.to.value" }
export interface JsonLogicVal {
  val: string;
}

// Generic JSONLogic expression
export type JsonLogicExpression = {
  [operator: string]: JsonLogicValue | JsonLogicValue[];
};

// Any valid JSONLogic value
export type JsonLogicValue =
  | JsonLogicPrimitive
  | JsonLogicPrimitive[]
  | JsonLogicExpression
  | JsonLogicValue[];

// Operator categories for styling
export type OperatorCategory =
  | 'variable'
  | 'comparison'
  | 'logical'
  | 'arithmetic'
  | 'string'
  | 'array'
  | 'control'
  | 'datetime'
  | 'error'
  | 'literal';

// Operator metadata for the registry
export interface OperatorMeta {
  name: string;
  category: OperatorCategory;
  label: string;
  description: string;
  minArgs?: number;
  maxArgs?: number;
  argLabels?: string[];
}

// CATEGORY_COLORS moved to constants/colors.ts
