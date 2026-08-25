import type { JsonLogicValue } from '../components/logic-editor';

/** An embed playground example; see `SampleExpression` for the field semantics. */
export interface EmbedSampleExpression {
  logic: JsonLogicValue;
  data: unknown;
  expected: unknown;
  templating?: boolean;
}

// Sample expressions for the embed playground
export const EMBED_SAMPLE_EXPRESSIONS: Record<string, EmbedSampleExpression> = {
  'Simple Comparison': {
    logic: { '==': [1, 1] },
    data: {},
    expected: true,
  },
  'Variable Access': {
    logic: { var: 'user.name' },
    data: { user: { name: 'Alice', age: 30 } },
    expected: 'Alice',
  },
  'Conditional': {
    logic: { if: [{ '>=': [{ var: 'age' }, 18] }, 'adult', 'minor'] },
    data: { age: 21 },
    expected: 'adult',
  },
  'Array Filter': {
    logic: { filter: [{ var: 'numbers' }, { '>': [{ var: '' }, 5] }] },
    data: { numbers: [1, 3, 5, 7, 9, 11] },
    expected: [7, 9, 11],
  },
  'Array Map': {
    logic: { map: [{ var: 'items' }, { '*': [{ var: '' }, 2] }] },
    data: { items: [1, 2, 3, 4, 5] },
    expected: [2, 4, 6, 8, 10],
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
    expected: 'C - Average',
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
    expected: 100,
  },
  'Discount Price': {
    logic: {
      '*': [
        { var: 'price' },
        { '-': [1, { '/': [{ var: 'discountPercent' }, 100] }] },
      ],
    },
    data: { price: 150, discountPercent: 25 },
    expected: 112.5,
  },
  'Status Router': {
    logic: {
      switch: [{ var: 'status' }, [[200, 'OK'], [404, 'Not Found']], 'Unknown'],
    },
    data: { status: 404 },
    expected: 'Not Found',
  },
  'Nickname Fallback': {
    logic: { '??': [{ var: 'nickname' }, { var: 'name' }, 'anonymous'] },
    data: { name: 'Ada', nickname: null },
    expected: 'Ada',
  },
  'Guarded Division': {
    logic: {
      try: [
        { '/': [{ var: 'a' }, { var: 'b' }] },
        { cat: ['Error: ', { var: 'type' }] },
      ],
    },
    data: { a: 1, b: 0 },
    expected: 'Error: NaN',
  },
  'Group Orders': {
    logic: { group_by: [{ var: 'orders' }, { var: 'status' }] },
    data: {
      orders: [
        { id: 1, status: 'paid' },
        { id: 2, status: 'open' },
        { id: 3, status: 'paid' },
      ],
    },
    expected: [
      { key: 'paid', items: [{ id: 1, status: 'paid' }, { id: 3, status: 'paid' }] },
      { key: 'open', items: [{ id: 2, status: 'open' }] },
    ],
  },
  'Local Time': {
    logic: {
      format_date: [
        { datetime: '2026-08-17T18:30:00Z' },
        'dd MMM yyyy HH:mm',
        'Asia/Kolkata',
      ],
    },
    data: {},
    expected: '18 Aug 2026 00:00',
  },
  'Order Summary (Template)': {
    logic: {
      order_id: { var: 'id' },
      total: { '*': [{ var: 'qty' }, { var: 'price' }] },
      skus: { map: [{ var: 'lines' }, { var: 'sku' }] },
    },
    data: { id: 'A1', qty: 2, price: 9.5, lines: [{ sku: 'X' }, { sku: 'Y' }] },
    expected: { order_id: 'A1', total: 19, skus: ['X', 'Y'] },
    templating: true,
  },
};
