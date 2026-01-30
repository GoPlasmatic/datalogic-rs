import type { JsonLogicValue } from '../components/logic-editor';

// Sample expressions for the embed playground
export const EMBED_SAMPLE_EXPRESSIONS: Record<string, { logic: JsonLogicValue; data: object }> = {
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
