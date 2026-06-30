# Starter Boilerplates

Ready-to-run microservice integration templates for major frameworks. These patterns demonstrate clean route protection, dynamic calculations, and in-memory feature-flag evaluations using `datalogic`.

---

## 🟢 Node.js + Express (`node-express-rules`)

Protect routes dynamically using `@goplasmatic/datalogic-node` middleware. This pattern compiles your rule sets and matches incoming request properties (path, headers, user roles) against them.

### Middleware Implementation

```javascript
import express from 'express';
import { Engine } from '@goplasmatic/datalogic-node';

const app = express();
const engine = new Engine();

// Example authorization rules stored in database
const rules = {
  "/admin": { "==": [{ "var": "user.role" }, "admin"] },
  "/billing": { "in": [{ "var": "user.role" }, ["admin", "billing_manager"]] }
};

// Compile rules for O(1) matching speed
const compiledRules = {};
for (const [route, rule] of Object.entries(rules)) {
  compiledRules[route] = engine.compile(JSON.stringify(rule));
}

// Authorization middleware
const authorize = (req, res, next) => {
  const routeRule = compiledRules[req.path];
  if (!routeRule) return next(); // No rules defined for this route

  // Mock user session context
  const context = {
    user: {
      role: req.headers['x-user-role'] || 'guest'
    }
  };

  try {
    const isAllowed = JSON.parse(routeRule.evaluate(JSON.stringify(context)));
    if (isAllowed) {
      next();
    } else {
      res.status(403).json({ error: 'Forbidden' });
    }
  } catch (err) {
    res.status(500).json({ error: 'Auth evaluation error' });
  }
};

app.use(authorize);

app.get('/admin', (req, res) => res.send('Welcome, Admin!'));
app.get('/billing', (req, res) => res.send('Billing dashboard'));
```

---

## 🐍 Python + FastAPI (`python-fastapi-pricing`)

Perform fast calculations for dynamic discounts, sales tax, or shipping fees at the API boundary using `datalogic-py`.

### Pricing Endpoint

```python
from fastapi import FastAPI, HTTPException
from pydantic import BaseModel
from datalogic_py import Engine, DataLogicError

app = FastAPI()
engine = Engine()

# Rule: If cart value > 100 AND user is VIP, discount = 20%; otherwise 5%
discount_rule = engine.compile({
    "if": [
        {
            "and": [
                {">": [{"var": "cart_total"}, 100]},
                {"==": [{"var": "user.is_vip"}, True]}
            ]
        },
        0.20,
        0.05
    ]
})

class CartContext(BaseModel):
    cart_total: float
    user: dict  # e.g. {"name": "Alice", "is_vip": True}

@app.post("/calculate-discount")
async def get_discount(context: CartContext):
    try:
        # Evaluate against request data
        discount_percentage = discount_rule.evaluate(context.model_dump())
        return {"discount_percentage": discount_percentage}
    except DataLogicError as e:
        raise HTTPException(status_code=400, detail=f"Rule evaluation failed: {str(e)}")
```

---

## 🦀 Rust + Axum (`rust-axum-feature-flags`)

A high-performance feature-flag evaluator that uses transient session recycling to achieve sub-microsecond latency.

```rust
use axum::{routing::post, Json, Router};
use datalogic_rs::{Engine, Logic};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

struct AppState {
    engine: Engine,
    rule: Logic,
}

#[derive(Deserialize)]
struct UserContext {
    user_id: String,
    country: String,
    beta_user: bool,
}

#[derive(Serialize)]
struct FlagResponse {
    enabled: bool,
}

async fn check_flag(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
    Json(payload): Json<UserContext>,
) -> Json<FlagResponse> {
    // 1. Create a session for thread-local arena allocation
    let mut session = state.engine.session();
    
    // 2. Evaluate
    let result = session.eval_into::<bool, _, _>(
        &state.rule,
        &serde_json::to_value(payload).unwrap()
    ).unwrap_or(false);
    
    // 3. Reset the arena buffer for reuse on the next request
    session.reset();

    Json(FlagResponse { enabled: result })
}

#[tokio::main]
async fn main() {
    let engine = Engine::new();
    // Rule: Enable beta feature if user is beta_user OR resides in CA
    let rule = engine.compile(r#"{
        "or": [
            {"==": [{"var": "beta_user"}, true]},
            {"==": [{"var": "country"}, "CA"]}
        ]
    }"#).unwrap();

    let state = Arc::new(AppState { engine, rule });

    let app = Router::new()
        .route("/flag", post(check_flag))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
```
