import type { JsonLogicValue } from '../components/logic-editor';

/**
 * A Studio example. `expected` is the engine's result for `logic` against
 * `data` (verified with the all-features CLI); `tests/samples.test.ts`
 * asserts the vendored WASM engine still agrees. Samples flagged with
 * `templating: true` only parse in templating mode, and `loadSample`
 * switches the Templating toggle on or off to match.
 */
export interface SampleExpression {
  logic: JsonLogicValue;
  data: unknown;
  expected: unknown;
  templating?: boolean;
}

// Sample JSONLogic expressions for testing - organized by visual complexity
export const SAMPLE_EXPRESSIONS: Record<string, SampleExpression> = {
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
    expected: "Dear Dr. Smith,",
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
    expected: 112.5,
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
    expected: true,
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
    expected: "Failed. Need 15 more points",
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
    expected: "C - Average",
  },

  // Array iteration
  "Map - Double": {
    logic: {
      map: [{ var: "numbers" }, { "*": [{ var: "" }, 2] }],
    },
    data: { numbers: [1, 2, 3, 4, 5] },
    expected: [2, 4, 6, 8, 10],
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
    expected: [25, 30, 15],
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
    expected: 100,
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
    expected: 20,
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
    expected: 130.5,
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
    expected: true,
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
    expected: true,
  },

  // ============================================
  // Tier 4: v5 operators
  // ============================================

  // switch: value routing with a default arm
  "Status Router": {
    logic: {
      switch: [
        { var: "status" },
        [[200, "OK"], [404, "Not Found"]],
        "Unknown",
      ],
    },
    data: { status: 404 },
    expected: "Not Found",
  },

  // ??: first non-null value wins (variadic)
  "Nickname Fallback": {
    logic: { "??": [{ var: "nickname" }, { var: "name" }, "anonymous"] },
    data: { name: "Ada", nickname: null },
    expected: "Ada",
  },

  // try: the catch arm sees the error as its context ({"type": ...})
  "Guarded Division": {
    logic: {
      try: [
        { "/": [{ var: "a" }, { var: "b" }] },
        { cat: ["Error: ", { var: "type" }] },
      ],
    },
    data: { a: 1, b: 0 },
    expected: "Error: NaN",
  },

  // throw + try: reject invalid input with a named error
  "Validate or Reject": {
    logic: {
      try: [
        {
          if: [
            { "<": [{ var: "qty" }, 1] },
            { throw: "InvalidQuantity" },
            { "*": [{ var: "qty" }, { var: "price" }] },
          ],
        },
        { cat: ["Rejected: ", { var: "type" }] },
      ],
    },
    data: { qty: 0, price: 5 },
    expected: "Rejected: InvalidQuantity",
  },

  // type: runtime type names
  "Type Guard": {
    logic: {
      if: [
        { "==": [{ type: { var: "value" } }, "number"] },
        { "*": [{ var: "value" }, 2] },
        { cat: ["not a number: ", { type: { var: "value" } }] },
      ],
    },
    data: { value: "abc" },
    expected: "not a number: string",
  },

  // ===, !==, !!: strict comparison and boolean coercion
  "Strict vs Loose": {
    logic: {
      if: [
        { "===": [{ var: "a" }, { var: "b" }] },
        "identical",
        { "==": [{ var: "a" }, { var: "b" }] },
        "loosely equal",
        "different",
      ],
    },
    data: { a: 1, b: "1" },
    expected: "loosely equal",
  },

  // !==, !!: strict inequality and truthiness ([] and {} are falsy)
  "Truthiness Check": {
    logic: {
      and: [
        { "!==": [{ var: "a" }, "1"] },
        { "!!": { var: "tags" } },
        { "!": { "!!": { var: "empty" } } },
      ],
    },
    data: { a: 1, tags: ["x"], empty: [] },
    expected: true,
  },

  // entries: iterate an object as {key, value} rows
  "Config Entries": {
    logic: {
      map: [
        { entries: { var: "config" } },
        { cat: [{ var: "key" }, "=", { var: "value" }] },
      ],
    },
    data: { config: { retries: 3, timeout: 30 } },
    expected: ["retries=3", "timeout=30"],
  },

  // keys + values + merge
  "Keys and Values": {
    logic: {
      merge: [{ keys: { var: "config" } }, { values: { var: "config" } }],
    },
    data: { config: { retries: 3, timeout: 30 } },
    expected: ["retries", "timeout", 3, 30],
  },

  // group_by: bucket rows by a key expression
  "Group Orders": {
    logic: { group_by: [{ var: "orders" }, { var: "status" }] },
    data: {
      orders: [
        { id: 1, status: "paid" },
        { id: 2, status: "open" },
        { id: 3, status: "paid" },
      ],
    },
    expected: [
      { key: "paid", items: [{ id: 1, status: "paid" }, { id: 3, status: "paid" }] },
      { key: "open", items: [{ id: 2, status: "open" }] },
    ],
  },

  // distinct + merge: union of two tag lists
  "Unique Tags": {
    logic: { distinct: { merge: [{ var: "a" }, { var: "b" }] } },
    data: { a: ["x", "y"], b: ["y", "z"] },
    expected: ["x", "y", "z"],
  },

  // sort with a key extractor (descending)
  "Sort by Price": {
    logic: {
      map: [
        { sort: [{ var: "items" }, false, { var: "price" }] },
        { var: "name" },
      ],
    },
    data: {
      items: [
        { name: "A", price: 5 },
        { name: "B", price: 20 },
        { name: "C", price: 10 },
      ],
    },
    expected: ["B", "C", "A"],
  },

  // slice with a step: [array, start, end, step]
  "Every Other": {
    logic: { slice: [{ var: "numbers" }, 1, -1, 2] },
    data: { numbers: [0, 1, 2, 3, 4, 5, 6, 7] },
    expected: [1, 3, 5],
  },

  // val scope metadata: [[1], "index"] reads the iteration index
  "Numbered List": {
    logic: {
      map: [
        { var: "names" },
        { cat: [{ "+": [{ val: [[1], "index"] }, 1] }, ". ", { var: "" }] },
      ],
    },
    data: { names: ["Ada", "Grace", "Linus"] },
    expected: ["1. Ada", "2. Grace", "3. Linus"],
  },

  // %, filter: even numbers only
  "Even Numbers": {
    logic: {
      filter: [{ var: "numbers" }, { "==": [{ "%": [{ var: "" }, 2] }, 0] }],
    },
    data: { numbers: [1, 2, 3, 4, 5, 6] },
    expected: [2, 4, 6],
  },

  // max/min clamp and ceil rounding
  "Clamp and Round": {
    logic: {
      cat: [
        { max: [0, { min: [100, { var: "value" }] }] },
        "% of ",
        { "/": [{ ceil: { "*": [{ var: "price" }, 100] } }, 100] },
      ],
    },
    data: { value: 140, price: 12.345 },
    expected: "100% of 12.35",
  },

  // floor + abs
  "Whole Distance": {
    logic: { floor: { abs: { var: "delta" } } },
    data: { delta: -3.7 },
    expected: 3,
  },

  // String transforms: trim, substr, upper, lower
  "Name Normalizer": {
    logic: {
      cat: [
        { upper: { substr: [{ trim: { var: "name" } }, 0, 1] } },
        { lower: { substr: [{ trim: { var: "name" } }, 1] } },
      ],
    },
    data: { name: "  aLICE " },
    expected: "Alice",
  },

  // in, starts_with, ends_with
  "Email Check": {
    logic: {
      and: [
        { in: ["@", { var: "email" }] },
        { ends_with: [{ var: "email" }, ".com"] },
        { "!": { starts_with: [{ var: "email" }, "@"] } },
      ],
    },
    data: { email: "ada@example.com" },
    expected: true,
  },

  // split + trim + length
  "CSV Fields": {
    logic: {
      if: [
        { ">=": [{ length: { split: [{ var: "csv" }, ","] } }, 3] },
        { map: [{ split: [{ var: "csv" }, ","] }, { trim: { var: "" } }] },
        "too few fields",
      ],
    },
    data: { csv: "a, b ,c" },
    expected: ["a", "b", "c"],
  },

  // missing / missing_some: required-field validation
  "Required Fields": {
    logic: {
      if: [
        { missing: ["name", "email"] },
        { cat: ["Missing: ", { missing: ["name", "email"] }] },
        { missing_some: [1, ["phone", "mobile"]] },
      ],
    },
    data: { name: "Ada" },
    expected: "Missing: email",
  },

  // exists: nested path check via the array form
  "Exists Check": {
    logic: {
      and: [{ exists: ["user", "email"] }, { "!": { exists: "phone" } }],
    },
    data: { user: { email: "a@b.com" } },
    expected: true,
  },

  // datetime + format_date with an IANA timezone
  "Local Time": {
    logic: {
      format_date: [
        { datetime: { var: "launch" } },
        "EEE dd MMM yyyy HH:mm",
        { var: "tz" },
      ],
    },
    data: { launch: "2026-08-17T18:30:00Z", tz: "Asia/Kolkata" },
    expected: "Tue 18 Aug 2026 00:00",
  },

  // date_diff in days
  "Days Until Deadline": {
    logic: {
      cat: [
        {
          date_diff: [
            { datetime: { var: "deadline" } },
            { datetime: { var: "today" } },
            "days",
          ],
        },
        " days left",
      ],
    },
    data: { deadline: "2026-09-01T00:00:00Z", today: "2026-08-25T00:00:00Z" },
    expected: "7 days left",
  },

  // parse_date + datetime arithmetic with a timestamp duration
  "Reschedule": {
    logic: {
      format_date: [
        {
          "+": [
            { parse_date: [{ var: "start" }, "dd/MM/yyyy HH:mm"] },
            { timestamp: "1d:2h:30m" },
          ],
        },
        "yyyy-MM-dd HH:mm",
      ],
    },
    data: { start: "25/08/2026 09:00" },
    expected: "2026-08-26 11:30",
  },

  // sem_ver (flagd): version gate
  "Version Gate": {
    logic: {
      if: [
        { sem_ver: [{ var: "app.version" }, ">=", "2.1.0"] },
        "new-checkout",
        "legacy-checkout",
      ],
    },
    data: { app: { version: "v2.3.1" } },
    expected: "new-checkout",
  },

  // fractional (flagd): sticky percentage rollout keyed by flagKey + email
  "Rollout Bucket": {
    logic: {
      if: [
        { in: ["@faas.com", { var: "email" }] },
        {
          fractional: [
            { cat: [{ var: "$flagd.flagKey" }, { var: "email" }] },
            ["red", 25],
            ["blue", 25],
            ["green", 25],
            ["yellow", 25],
          ],
        },
        "default",
      ],
    },
    data: { email: "rachel@faas.com", $flagd: { flagKey: "headerColor" } },
    expected: "blue",
  },

  // The data root can be an array: {"var": ""} is the whole context
  "Top-Level Array": {
    logic: {
      map: [
        { filter: [{ var: "" }, { ">=": [{ var: "score" }, 50] }] },
        { var: "name" },
      ],
    },
    data: [
      { name: "Ada", score: 91 },
      { name: "Bob", score: 42 },
      { name: "Cy", score: 75 },
    ],
    expected: ["Ada", "Cy"],
  },

  // ============================================
  // Tensor: marshalling JSON into a model's inputs and back out
  // ============================================

  // Encode a request into the shape a model expects: normalize the raw
  // features, then stack them into one batch tensor. `shape` is read back
  // rather than the buffer, which is base64 on the wire.
  "Model Input Batch": {
    logic: {
      shape: [
        {
          stack: [
            {
              map: [
                { var: "requests" },
                { normalize: [{ tensor: [{ var: "features" }, "f32"] }, 0.5, 2] },
              ],
            },
            0,
          ],
        },
      ],
    },
    data: {
      requests: [
        { features: [0.1, 0.7, 0.4, 0.9] },
        { features: [0.3, 0.2, 0.8, 0.6] },
        { features: [0.5, 0.5, 0.5, 0.5] },
      ],
    },
    expected: [3, 4],
  },

  // Read a model's output back into JSON: pick the winning class per row
  // and label it. `argmax` collapses an axis and hands back a plain list,
  // so ordinary array operators take over from there.
  "Model Output Labels": {
    logic: {
      map: [
        { argmax: [{ tensor: [{ var: "logits" }, "f32"] }, 1] },
        { val: [[2], "labels", { val: [] }] },
      ],
    },
    data: {
      logits: [
        [0.1, 0.8, 0.1],
        [0.7, 0.2, 0.1],
        [0.2, 0.3, 0.5],
      ],
      labels: ["cat", "dog", "bird"],
    },
    expected: ["dog", "cat", "bird"],
  },

  // One-hot a categorical field into the dense row a model wants.
  "One-Hot Encode": {
    logic: {
      to_list: [{ one_hot: [{ var: "category_ids" }, 4, "u8"] }],
    },
    data: { category_ids: [0, 3, 1] },
    expected: [
      [1, 0, 0, 0],
      [0, 0, 0, 1],
      [0, 1, 0, 0],
    ],
  },

  // ============================================
  // Templating mode (multi-key objects are output templates)
  // ============================================

  "Order Summary (Template)": {
    logic: {
      order_id: { var: "id" },
      total: { "*": [{ var: "qty" }, { var: "price" }] },
      skus: { map: [{ var: "lines" }, { var: "sku" }] },
    },
    data: { id: "A1", qty: 2, price: 9.5, lines: [{ sku: "X" }, { sku: "Y" }] },
    expected: { order_id: "A1", total: 19, skus: ["X", "Y"] },
    templating: true,
  },

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
    expected: { name_and_address: [], party_identifier: "//USABA12345" },
    templating: true,
  },
};
