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

// Re-export OperatorCategory from the canonical source
// Note: Includes 'literal' for node styling but actual operator categories
// are defined in config/operators.types.ts
export type { OperatorCategory } from '../config/operators.types';

// Extended OperatorCategory that includes 'literal' for node styling
// (literal nodes aren't operators but need category-based styling)
export type NodeCategory =
  | import('../config/operators.types').OperatorCategory
  | 'literal';

// CATEGORY_COLORS moved to constants/colors.ts
