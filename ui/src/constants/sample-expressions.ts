import type { JsonLogicValue } from '../components/logic-editor';

// Sample JSONLogic expressions for testing - organized by visual complexity
export const SAMPLE_EXPRESSIONS: Record<
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
