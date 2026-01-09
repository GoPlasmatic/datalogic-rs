# Use Cases & Examples

Real-world examples of using datalogic-rs for common scenarios.

## Feature Flags

Control feature availability based on user attributes.

### Basic Feature Flag

```rust
use datalogic_rs::DataLogic;
use serde_json::json;

let engine = DataLogic::new();

// Feature available to premium users in US
let rule = json!({
    "and": [
        { "==": [{ "var": "user.plan" }, "premium"] },
        { "==": [{ "var": "user.country" }, "US"] }
    ]
});

let compiled = engine.compile(&rule).unwrap();

let user_data = json!({
    "user": {
        "plan": "premium",
        "country": "US"
    }
});

let enabled = engine.evaluate_owned(&compiled, user_data).unwrap();
assert_eq!(enabled, json!(true));
```

### Percentage Rollout

```rust
// Enable for 20% of users (based on user ID hash)
let rule = json!({
    "<": [
        { "%": [{ "var": "user.id" }, 100] },
        20
    ]
});
```

### Beta Access

```rust
// Enable for beta testers OR employees OR users who signed up before a date
let rule = json!({
    "or": [
        { "==": [{ "var": "user.role" }, "beta_tester"] },
        { "ends_with": [{ "var": "user.email" }, "@company.com"] },
        { "<": [{ "var": "user.signup_date" }, "2024-01-01"] }
    ]
});
```

---

## Dynamic Pricing

Calculate prices based on rules.

### Discount by Quantity

```rust
let rule = json!({
    "if": [
        { ">=": [{ "var": "quantity" }, 100] },
        { "*": [{ "var": "base_price" }, 0.8] },  // 20% off
        { "if": [
            { ">=": [{ "var": "quantity" }, 50] },
            { "*": [{ "var": "base_price" }, 0.9] },  // 10% off
            { "var": "base_price" }
        ]}
    ]
});

let data = json!({
    "quantity": 75,
    "base_price": 100
});

let price = engine.evaluate_owned(&compiled, data).unwrap();
// Result: 90 (10% discount)
```

### Tiered Pricing

```rust
let rule = json!({
    "+": [
        // First 10 units at $10
        { "*": [{ "min": [{ "var": "quantity" }, 10] }, 10] },
        // Next 40 units at $8
        { "*": [
            { "max": [{ "-": [{ "min": [{ "var": "quantity" }, 50] }, 10] }, 0] },
            8
        ]},
        // Remaining units at $6
        { "*": [
            { "max": [{ "-": [{ "var": "quantity" }, 50] }, 0] },
            6
        ]}
    ]
});
```

### Member Pricing

```rust
let rule = json!({
    "if": [
        { "var": "user.is_member" },
        { "*": [
            { "var": "product.price" },
            { "-": [1, { "/": [{ "var": "user.member_discount" }, 100] }] }
        ]},
        { "var": "product.price" }
    ]
});

let data = json!({
    "user": { "is_member": true, "member_discount": 15 },
    "product": { "price": 200 }
});
// Result: 170 (15% member discount)
```

---

## Form Validation

Validate user input with complex rules.

### Required Fields

```rust
let rule = json!({
    "if": [
        { "missing": ["name", "email", "password"] },
        {
            "valid": false,
            "errors": { "missing": ["name", "email", "password"] }
        },
        { "valid": true }
    ]
});
```

### Field Constraints

```rust
let engine = DataLogic::with_preserve_structure();

let rule = json!({
    "valid": { "and": [
        // Email format
        { "in": ["@", { "var": "email" }] },
        // Password length
        { ">=": [{ "length": { "var": "password" } }, 8] },
        // Age range
        { "and": [
            { ">=": [{ "var": "age" }, 18] },
            { "<=": [{ "var": "age" }, 120] }
        ]}
    ]},
    "errors": { "filter": [
        [
            { "if": [
                { "!": { "in": ["@", { "var": "email" }] } },
                "Invalid email format",
                null
            ]},
            { "if": [
                { "<": [{ "length": { "var": "password" } }, 8] },
                "Password must be at least 8 characters",
                null
            ]},
            { "if": [
                { "or": [
                    { "<": [{ "var": "age" }, 18] },
                    { ">": [{ "var": "age" }, 120] }
                ]},
                "Age must be between 18 and 120",
                null
            ]}
        ],
        { "!==": [{ "var": "" }, null] }
    ]}
});
```

### Conditional Validation

```rust
// If business account, require company name
let rule = json!({
    "if": [
        { "and": [
            { "==": [{ "var": "account_type" }, "business"] },
            { "missing": ["company_name"] }
        ]},
        { "error": "Company name required for business accounts" },
        { "valid": true }
    ]
});
```

---

## Access Control

Determine user permissions.

### Role-Based Access

```rust
let rule = json!({
    "or": [
        { "==": [{ "var": "user.role" }, "admin"] },
        { "and": [
            { "==": [{ "var": "user.role" }, "editor"] },
            { "==": [{ "var": "resource.owner_id" }, { "var": "user.id" }] }
        ]}
    ]
});
```

### Permission Checking

```rust
let rule = json!({
    "in": [
        { "var": "required_permission" },
        { "var": "user.permissions" }
    ]
});

let data = json!({
    "user": {
        "permissions": ["read", "write", "delete"]
    },
    "required_permission": "write"
});
// Result: true
```

### Time-Based Access

```rust
let rule = json!({
    "and": [
        // Has permission
        { "in": ["access_data", { "var": "user.permissions" }] },
        // Within allowed hours (9 AM - 6 PM)
        { "and": [
            { ">=": [{ "var": "current_hour" }, 9] },
            { "<": [{ "var": "current_hour" }, 18] }
        ]},
        // On a weekday
        { "in": [{ "var": "current_day" }, [1, 2, 3, 4, 5]] }
    ]
});
```

---

## Fraud Detection

Score and flag potentially fraudulent transactions.

### Risk Scoring

```rust
let rule = json!({
    "+": [
        // High amount
        { "if": [{ ">": [{ "var": "amount" }, 1000] }, 30, 0] },
        // New account
        { "if": [{ "<": [{ "var": "account_age_days" }, 7] }, 25, 0] },
        // Different country
        { "if": [
            { "!=": [{ "var": "billing_country" }, { "var": "shipping_country" }] },
            20,
            0
        ]},
        // Multiple attempts
        { "if": [{ ">": [{ "var": "attempts_last_hour" }, 3] }, 25, 0] },
        // Unusual time
        { "if": [
            { "or": [
                { "<": [{ "var": "hour" }, 6] },
                { ">": [{ "var": "hour" }, 23] }
            ]},
            15,
            0
        ]}
    ]
});

// Score > 50 = flag for review
let data = json!({
    "amount": 1500,
    "account_age_days": 3,
    "billing_country": "US",
    "shipping_country": "CA",
    "attempts_last_hour": 1,
    "hour": 14
});
// Result: 75 (high amount + new account + different country)
```

### Velocity Checks

```rust
let rule = json!({
    "or": [
        // Too many transactions in short time
        { ">": [{ "var": "transactions_last_hour" }, 10] },
        // Too much total amount
        { ">": [{ "var": "total_amount_last_hour" }, 5000] },
        // Same card used from multiple IPs
        { ">": [{ "var": "unique_ips_last_day" }, 3] }
    ]
});
```

---

## Data Transformation

Transform and reshape data.

### API Response Mapping

```rust
let engine = DataLogic::with_preserve_structure();

let template = json!({
    "users": {
        "map": [
            { "var": "raw_users" },
            {
                "id": { "var": "user_id" },
                "fullName": { "cat": [{ "var": "first_name" }, " ", { "var": "last_name" }] },
                "email": { "lower": { "var": "email" } },
                "isActive": { "==": [{ "var": "status" }, "active"] }
            }
        ]
    },
    "total": { "length": { "var": "raw_users" } },
    "activeCount": { "length": {
        "filter": [
            { "var": "raw_users" },
            { "==": [{ "var": "status" }, "active"] }
        ]
    }}
});
```

### Report Generation

```rust
let template = json!({
    "report": {
        "title": { "cat": ["Sales Report - ", { "var": "period" }] },
        "generated": { "format_date": [{ "now": [] }, "%Y-%m-%d %H:%M"] },
        "summary": {
            "totalSales": { "reduce": [
                { "var": "transactions" },
                { "+": [{ "var": "accumulator" }, { "var": "current.amount" }] },
                0
            ]},
            "avgTransaction": { "/": [
                { "reduce": [
                    { "var": "transactions" },
                    { "+": [{ "var": "accumulator" }, { "var": "current.amount" }] },
                    0
                ]},
                { "length": { "var": "transactions" } }
            ]},
            "topCategory": { "var": "top_category" }
        }
    }
});
```

---

## Notification Rules

Determine when and how to send notifications.

### Alert Conditions

```rust
let rule = json!({
    "if": [
        // Critical: immediate
        { ">": [{ "var": "error_rate" }, 10] },
        { "channel": "pager", "priority": "critical" },
        // Warning: Slack
        { "if": [
            { ">": [{ "var": "error_rate" }, 5] },
            { "channel": "slack", "priority": "warning" },
            // Info: email digest
            { "if": [
                { ">": [{ "var": "error_rate" }, 1] },
                { "channel": "email", "priority": "info" },
                null
            ]}
        ]}
    ]
});
```

### User Preferences

```rust
let rule = json!({
    "and": [
        // User has enabled notifications
        { "var": "user.notifications_enabled" },
        // Notification type is in user's preferences
        { "in": [
            { "var": "notification.type" },
            { "var": "user.enabled_types" }
        ]},
        // Within user's quiet hours
        { "!": { "and": [
            { ">=": [{ "var": "current_hour" }, { "var": "user.quiet_start" }] },
            { "<": [{ "var": "current_hour" }, { "var": "user.quiet_end" }] }
        ]}}
    ]
});
```
