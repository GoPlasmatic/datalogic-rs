# Use Cases & Examples

Real-world JSONLogic recipes for common scenarios. Every rule on this page is plain JSON: author it once, store it where you store data (a database row, a config file, an API payload), and evaluate it unchanged from any language datalogic-rs ships bindings for. Each recipe below is the rule, a sample data payload, and the result; standard-mode recipes also embed a live widget so you can run them right here. A few recipes use the engine's templating mode to build output objects; those are flagged inline.

## Run any of these in your language

The pattern is identical everywhere: compile the rule once, then evaluate it against as many data payloads as you like.

<div class="codetabs">

```rust
use datalogic_rs::Engine;

let engine = Engine::new();
let rule = engine
    .compile(r#"{"==": [{"var": "user.plan"}, "premium"]}"#)
    .unwrap();

let mut session = engine.session();
for payload in payloads {
    println!("{}", session.eval_str(&rule, payload).unwrap());
    session.reset(); // reset between evaluations to keep memory flat
}
```

```javascript
import { Engine } from '@goplasmatic/datalogic-node';

const engine = new Engine();
const rule = engine.compile({ '==': [{ var: 'user.plan' }, 'premium'] });

for (const payload of payloads) {
  console.log(rule.evaluate(payload));
}
```

```python
from datalogic_py import Engine

engine = Engine()
rule = engine.compile({"==": [{"var": "user.plan"}, "premium"]})

for payload in payloads:
    print(rule.evaluate(payload))
```

```go
import datalogic "github.com/GoPlasmatic/datalogic-rs/bindings/go/v5"

engine := datalogic.NewEngine()
defer engine.Close()

rule, _ := engine.Compile(`{"==": [{"var": "user.plan"}, "premium"]}`)
defer rule.Close()

for _, payload := range payloads {
    out, _ := rule.Evaluate(payload)
    fmt.Println(out)
}
```

```java
import com.goplasmatic.datalogic.Engine;
import com.goplasmatic.datalogic.Rule;

try (Engine engine = new Engine();
     Rule rule = engine.compile("{\"==\": [{\"var\": \"user.plan\"}, \"premium\"]}")) {
    for (String payload : payloads) {
        System.out.println(rule.evaluate(payload));
    }
}
```

```csharp
using Goplasmatic.Datalogic;

using var engine = new Engine();
using var rule = engine.Compile("""{"==": [{"var": "user.plan"}, "premium"]}""");

foreach (var payload in payloads)
{
    Console.WriteLine(rule.Evaluate(payload));
}
```

```php
use Goplasmatic\Datalogic\Engine;

$engine = new Engine();
$rule = $engine->compile('{"==": [{"var": "user.plan"}, "premium"]}');

foreach ($payloads as $payload) {
    echo $rule->evaluate($payload), "\n";
}
```

</div>

A few operators sit behind Cargo features in the Rust crate (`ext-string`, `datetime`); recipes that use them say so. Every language binding ships with all operator features enabled, so outside Rust there is nothing to switch on.

## Feature Flags

Control feature availability based on user attributes.

### Basic Feature Flag

Feature available to premium users in the US:

```json
{
    "and": [
        {"==": [{"var": "user.plan"}, "premium"]},
        {"==": [{"var": "user.country"}, "US"]}
    ]
}
```

Data:

```json
{
    "user": {"plan": "premium", "country": "US"}
}
```

Result: `true`

**Try it:**

<div class="playground-widget" data-logic='{"and":[{"==":[{"var":"user.plan"},"premium"]},{"==":[{"var":"user.country"},"US"]}]}' data-data='{"user":{"plan":"premium","country":"US"}}'>
</div>

### Percentage Rollout

Enable for 20% of users, bucketing on a hash of the user ID:

```json
{
    "<": [
        { "%": [{ "var": "user.id" }, 100] },
        20
    ]
}
```

Data:

```json
{"user": {"id": 12345}}
```

Result: `false` (12345 % 100 = 45, and 45 is not below the 20 cutoff)

**Try it:**

<div class="playground-widget" data-logic='{"<":[{"%":[{"var":"user.id"},100]},20]}' data-data='{"user":{"id":12345}}'>
</div>

### Beta Access

Enable for beta testers OR employees OR users who signed up before a date. The `ends_with` operator requires the `ext-string` feature in Rust; enabled by default in every binding.

```json
{
    "or": [
        { "==": [{ "var": "user.role" }, "beta_tester"] },
        { "ends_with": [{ "var": "user.email" }, "@company.com"] },
        { "<": [{ "var": "user.signup_date" }, "2024-01-01"] }
    ]
}
```

Data:

```json
{
    "user": {"role": "customer", "email": "sam@company.com", "signup_date": "2024-03-15"}
}
```

Result: `true` (the email marks this user as an employee)

**Try it:**

<div class="playground-widget" data-logic='{"or":[{"==":[{"var":"user.role"},"beta_tester"]},{"ends_with":[{"var":"user.email"},"@company.com"]},{"<":[{"var":"user.signup_date"},"2024-01-01"]}]}' data-data='{"user":{"role":"customer","email":"sam@company.com","signup_date":"2024-03-15"}}'>
</div>

---

## Dynamic Pricing

Calculate prices based on rules.

### Discount by Quantity

20% off from 100 units, 10% off from 50 units, list price below that:

```json
{
    "if": [
        { ">=": [{ "var": "quantity" }, 100] },
        { "*": [{ "var": "base_price" }, 0.8] },
        { "if": [
            { ">=": [{ "var": "quantity" }, 50] },
            { "*": [{ "var": "base_price" }, 0.9] },
            { "var": "base_price" }
        ]}
    ]
}
```

Data:

```json
{"quantity": 75, "base_price": 100}
```

Result: `90` (10% discount)

**Try it:**

<div class="playground-widget" data-logic='{"if":[{">=":[{"var":"quantity"},100]},{"*":[{"var":"base_price"},0.8]},{"if":[{">=":[{"var":"quantity"},50]},{"*":[{"var":"base_price"},0.9]},{"var":"base_price"}]}]}' data-data='{"quantity":75,"base_price":100}'>
</div>

### Tiered Pricing

The first 10 units cost $10, the next 40 cost $8, and every unit past 50 costs $6:

```json
{
    "+": [
        { "*": [{ "min": [{ "var": "quantity" }, 10] }, 10] },
        { "*": [
            { "max": [{ "-": [{ "min": [{ "var": "quantity" }, 50] }, 10] }, 0] },
            8
        ]},
        { "*": [
            { "max": [{ "-": [{ "var": "quantity" }, 50] }, 0] },
            6
        ]}
    ]
}
```

Data:

```json
{"quantity": 75}
```

Result: `570` (10 units at $10, 40 at $8, 25 at $6)

**Try it:**

<div class="playground-widget" data-logic='{"+":[{"*":[{"min":[{"var":"quantity"},10]},10]},{"*":[{"max":[{"-":[{"min":[{"var":"quantity"},50]},10]},0]},8]},{"*":[{"max":[{"-":[{"var":"quantity"},50]},0]},6]}]}' data-data='{"quantity":75}'>
</div>

### Member Pricing

Members pay the product price minus their personal discount percentage:

```json
{
    "if": [
        { "var": "user.is_member" },
        { "*": [
            { "var": "product.price" },
            { "-": [1, { "/": [{ "var": "user.member_discount" }, 100] }] }
        ]},
        { "var": "product.price" }
    ]
}
```

Data:

```json
{
    "user": { "is_member": true, "member_discount": 15 },
    "product": { "price": 200 }
}
```

Result: `170` (15% member discount)

**Try it:**

<div class="playground-widget" data-logic='{"if":[{"var":"user.is_member"},{"*":[{"var":"product.price"},{"-":[1,{"/":[{"var":"user.member_discount"},100]}]}]},{"var":"product.price"}]}' data-data='{"user":{"is_member":true,"member_discount":15},"product":{"price":200}}'>
</div>

---

## Form Validation

Validate user input with complex rules.

### Required Fields

Report which required fields are absent; the `missing` operator inside the template evaluates to exactly that list:

```json
{
    "if": [
        { "missing": ["name", "email", "password"] },
        {
            "valid": false,
            "errors": { "missing": ["name", "email", "password"] }
        },
        { "valid": true }
    ]
}
```

Data:

```json
{"name": "Ada Lovelace"}
```

Result: `{"valid": false, "errors": ["email", "password"]}`

> **Templating recipe.** Multi-key objects like the `valid`/`errors` branch need templating mode: in Rust, the `templating` Cargo feature plus `Engine::builder().with_templating(true)`; in every binding, the `templating` flag when constructing the engine. The inline widgets on this page run in standard mode, so paste this pair into the [playground](https://goplasmatic.github.io/datalogic-rs/playground/) and switch on **Templating** to run it.

### Field Constraints

Check email shape, password length, and age range, and collect a message for each failed check. `length` requires the `ext-string` feature in Rust; enabled by default in every binding.

```json
{
    "valid": { "and": [
        { "in": ["@", { "var": "email" }] },
        { ">=": [{ "length": { "var": "password" } }, 8] },
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
}
```

Data:

```json
{"email": "ada@example.com", "password": "short", "age": 25}
```

Result: `{"valid": false, "errors": ["Password must be at least 8 characters"]}`

> **Templating recipe.** Needs the engine's templating mode (`templating` feature + `Engine::builder().with_templating(true)` in Rust, the `templating` constructor flag in every binding); run it in the [playground](https://goplasmatic.github.io/datalogic-rs/playground/) with **Templating** switched on.

### Conditional Validation

If it is a business account, require a company name:

```json
{
    "if": [
        { "and": [
            { "==": [{ "var": "account_type" }, "business"] },
            { "missing": ["company_name"] }
        ]},
        { "error": "Company name required for business accounts" },
        { "valid": true }
    ]
}
```

Data:

```json
{"account_type": "business", "contact_email": "ops@acme.io"}
```

Result: `{"error": "Company name required for business accounts"}`

> **Templating recipe.** The `error` and `valid` branches are literal output fields, which needs templating mode (see [Required Fields](#required-fields) above); run it in the [playground](https://goplasmatic.github.io/datalogic-rs/playground/) with **Templating** switched on.

---

## Access Control

Determine user permissions.

### Role-Based Access

Admins can always act; editors only on resources they own:

```json
{
    "or": [
        { "==": [{ "var": "user.role" }, "admin"] },
        { "and": [
            { "==": [{ "var": "user.role" }, "editor"] },
            { "==": [{ "var": "resource.owner_id" }, { "var": "user.id" }] }
        ]}
    ]
}
```

Data:

```json
{
    "user": {"role": "editor", "id": 42},
    "resource": {"owner_id": 42}
}
```

Result: `true`

**Try it:**

<div class="playground-widget" data-logic='{"or":[{"==":[{"var":"user.role"},"admin"]},{"and":[{"==":[{"var":"user.role"},"editor"]},{"==":[{"var":"resource.owner_id"},{"var":"user.id"}]}]}]}' data-data='{"user":{"role":"editor","id":42},"resource":{"owner_id":42}}'>
</div>

### Permission Checking

Is the required permission in the user's permission list:

```json
{
    "in": [
        { "var": "required_permission" },
        { "var": "user.permissions" }
    ]
}
```

Data:

```json
{
    "user": {
        "permissions": ["read", "write", "delete"]
    },
    "required_permission": "write"
}
```

Result: `true`

**Try it:**

<div class="playground-widget" data-logic='{"in":[{"var":"required_permission"},{"var":"user.permissions"}]}' data-data='{"user":{"permissions":["read","write","delete"]},"required_permission":"write"}'>
</div>

### Time-Based Access

Grant access only to permitted users, within allowed hours (9 AM to 6 PM), on a weekday:

```json
{
    "and": [
        { "in": ["access_data", { "var": "user.permissions" }] },
        { "and": [
            { ">=": [{ "var": "current_hour" }, 9] },
            { "<": [{ "var": "current_hour" }, 18] }
        ]},
        { "in": [{ "var": "current_day" }, [1, 2, 3, 4, 5]] }
    ]
}
```

Data:

```json
{
    "user": {"permissions": ["access_data", "export_reports"]},
    "current_hour": 14,
    "current_day": 3
}
```

Result: `true`

**Try it:**

<div class="playground-widget" data-logic='{"and":[{"in":["access_data",{"var":"user.permissions"}]},{"and":[{">=":[{"var":"current_hour"},9]},{"<":[{"var":"current_hour"},18]}]},{"in":[{"var":"current_day"},[1,2,3,4,5]]}]}' data-data='{"user":{"permissions":["access_data","export_reports"]},"current_hour":14,"current_day":3}'>
</div>

---

## Fraud Detection

Score and flag potentially fraudulent transactions.

### Risk Scoring

Sum weighted signals: high amount (+30), new account (+25), billing/shipping country mismatch (+20), repeated attempts (+25), unusual hour (+15). A score above 50 flags the transaction for review:

```json
{
    "+": [
        { "if": [{ ">": [{ "var": "amount" }, 1000] }, 30, 0] },
        { "if": [{ "<": [{ "var": "account_age_days" }, 7] }, 25, 0] },
        { "if": [
            { "!=": [{ "var": "billing_country" }, { "var": "shipping_country" }] },
            20,
            0
        ]},
        { "if": [{ ">": [{ "var": "attempts_last_hour" }, 3] }, 25, 0] },
        { "if": [
            { "or": [
                { "<": [{ "var": "hour" }, 6] },
                { ">": [{ "var": "hour" }, 23] }
            ]},
            15,
            0
        ]}
    ]
}
```

Data:

```json
{
    "amount": 1500,
    "account_age_days": 3,
    "billing_country": "US",
    "shipping_country": "CA",
    "attempts_last_hour": 1,
    "hour": 14
}
```

Result: `75` (high amount + new account + different country)

**Try it:**

<div class="playground-widget" data-logic='{"+":[{"if":[{">":[{"var":"amount"},1000]},30,0]},{"if":[{"<":[{"var":"account_age_days"},7]},25,0]},{"if":[{"!=":[{"var":"billing_country"},{"var":"shipping_country"}]},20,0]},{"if":[{">":[{"var":"attempts_last_hour"},3]},25,0]},{"if":[{"or":[{"<":[{"var":"hour"},6]},{">":[{"var":"hour"},23]}]},15,0]}]}' data-data='{"amount":1500,"account_age_days":3,"billing_country":"US","shipping_country":"CA","attempts_last_hour":1,"hour":14}'>
</div>

### Velocity Checks

Flag when any velocity signal crosses its threshold: too many transactions in a short window, too much total volume, or the same card used from too many IPs:

```json
{
    "or": [
        { ">": [{ "var": "transactions_last_hour" }, 10] },
        { ">": [{ "var": "total_amount_last_hour" }, 5000] },
        { ">": [{ "var": "unique_ips_last_day" }, 3] }
    ]
}
```

Data:

```json
{
    "transactions_last_hour": 14,
    "total_amount_last_hour": 1200,
    "unique_ips_last_day": 2
}
```

Result: `true` (more than 10 transactions in the last hour)

**Try it:**

<div class="playground-widget" data-logic='{"or":[{">":[{"var":"transactions_last_hour"},10]},{">":[{"var":"total_amount_last_hour"},5000]},{">":[{"var":"unique_ips_last_day"},3]}]}' data-data='{"transactions_last_hour":14,"total_amount_last_hour":1200,"unique_ips_last_day":2}'>
</div>

---

## Data Transformation

Transform and reshape data.

### API Response Mapping

Reshape raw records into an API response: rename fields, derive a full name, normalize email case, and compute counts. `lower` and `length` require the `ext-string` feature in Rust; enabled by default in every binding.

```json
{
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
}
```

Data:

```json
{
    "raw_users": [
        {"user_id": 101, "first_name": "Ada", "last_name": "Lovelace", "email": "Ada@Example.COM", "status": "active"},
        {"user_id": 102, "first_name": "Alan", "last_name": "Turing", "email": "Alan.Turing@Example.COM", "status": "inactive"}
    ]
}
```

Result: `{"users": [{"id": 101, "fullName": "Ada Lovelace", "email": "ada@example.com", "isActive": true}, {"id": 102, "fullName": "Alan Turing", "email": "alan.turing@example.com", "isActive": false}], "total": 2, "activeCount": 1}`

> **Templating recipe.** Needs the engine's templating mode (`templating` feature + `Engine::builder().with_templating(true)` in Rust, the `templating` constructor flag in every binding); run it in the [playground](https://goplasmatic.github.io/datalogic-rs/playground/) with **Templating** switched on.

### Report Generation

Build a report object with a computed title, a generation timestamp, and reduced summary stats. `format_date` and `now` require the `datetime` feature and `length` the `ext-string` feature in Rust; both enabled by default in every binding.

```json
{
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
}
```

Data:

```json
{
    "period": "Q2 2026",
    "top_category": "Electronics",
    "transactions": [
        {"amount": 1200, "category": "Electronics"},
        {"amount": 450, "category": "Home"},
        {"amount": 900, "category": "Electronics"}
    ]
}
```

Result: `{"report": {"title": "Sales Report - Q2 2026", "generated": "2026-07-03 09:41", "summary": {"totalSales": 2550, "avgTransaction": 850, "topCategory": "Electronics"}}}` (`generated` reflects the evaluation timestamp, so it varies run to run)

> **Templating recipe.** Needs the engine's templating mode (`templating` feature + `Engine::builder().with_templating(true)` in Rust, the `templating` constructor flag in every binding); run it in the [playground](https://goplasmatic.github.io/datalogic-rs/playground/) with **Templating** switched on.

---

## Notification Rules

Determine when and how to send notifications.

### Alert Conditions

Route by severity: an error rate above 10 pages someone immediately, above 5 posts a Slack warning, above 1 lands in the email digest, and anything lower sends nothing:

```json
{
    "if": [
        { ">": [{ "var": "error_rate" }, 10] },
        { "channel": "pager", "priority": "critical" },
        { "if": [
            { ">": [{ "var": "error_rate" }, 5] },
            { "channel": "slack", "priority": "warning" },
            { "if": [
                { ">": [{ "var": "error_rate" }, 1] },
                { "channel": "email", "priority": "info" },
                null
            ]}
        ]}
    ]
}
```

Data:

```json
{"error_rate": 7.5}
```

Result: `{"channel": "slack", "priority": "warning"}`

> **Templating recipe.** The channel/priority branches are output templates, which needs templating mode (`templating` feature + `Engine::builder().with_templating(true)` in Rust, the `templating` constructor flag in every binding); run it in the [playground](https://goplasmatic.github.io/datalogic-rs/playground/) with **Templating** switched on.

### User Preferences

Send only if the user has notifications enabled, subscribes to this notification type, and is not inside their quiet hours:

```json
{
    "and": [
        { "var": "user.notifications_enabled" },
        { "in": [
            { "var": "notification.type" },
            { "var": "user.enabled_types" }
        ]},
        { "!": { "and": [
            { ">=": [{ "var": "current_hour" }, { "var": "user.quiet_start" }] },
            { "<": [{ "var": "current_hour" }, { "var": "user.quiet_end" }] }
        ]}}
    ]
}
```

Data:

```json
{
    "user": {
        "notifications_enabled": true,
        "enabled_types": ["security", "billing"],
        "quiet_start": 22,
        "quiet_end": 8
    },
    "notification": {"type": "security"},
    "current_hour": 14
}
```

Result: `true`

**Try it:**

<div class="playground-widget" data-logic='{"and":[{"var":"user.notifications_enabled"},{"in":[{"var":"notification.type"},{"var":"user.enabled_types"}]},{"!":{"and":[{">=":[{"var":"current_hour"},{"var":"user.quiet_start"}]},{"<":[{"var":"current_hour"},{"var":"user.quiet_end"}]}]}}]}' data-data='{"user":{"notifications_enabled":true,"enabled_types":["security","billing"],"quiet_start":22,"quiet_end":8},"notification":{"type":"security"},"current_hour":14}'>
</div>

---

## Where next

- Open the [interactive playground](https://goplasmatic.github.io/datalogic-rs/playground/) to edit any of these rules live (switch on **Templating** for the templating recipes).
- Browse the full [operators overview](../operators/overview.md) for everything these recipes are built from.
- See [how datalogic-rs compares](../comparison.md) to other JSONLogic engines.
