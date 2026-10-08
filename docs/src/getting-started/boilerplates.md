# Starter Boilerplates

Starting points for three services: route authorization in Express, a pricing endpoint in FastAPI, and a feature-flag endpoint in Axum. Each compiles its rules once at startup and evaluates them per request. For a fuller Express walkthrough see [Integration: Express](../integrations/express.md).

---

## Node.js + Express

Authorize routes with `@goplasmatic/datalogic-node` middleware. The middleware compiles one rule per route and evaluates it against request properties (here, a role header).

### Middleware Implementation

```javascript
import express from 'express';
import { Engine } from '@goplasmatic/datalogic-node';

const app = express();
const engine = new Engine();

// Authorization rules, as you might load them from a database
const rules = {
  "/admin": { "==": [{ "var": "user.role" }, "admin"] },
  "/billing": { "in": [{ "var": "user.role" }, ["admin", "billing_manager"]] }
};

// Compile each rule once at startup, keyed by route
const compiledRules = {};
for (const [route, rule] of Object.entries(rules)) {
  compiledRules[route] = engine.compile(rule);
}

// Authorization middleware
const authorize = (req, res, next) => {
  const routeRule = compiledRules[req.path];
  if (!routeRule) return next(); // No rule for this route

  // Request context the rule reads
  const context = {
    user: {
      role: req.headers['x-user-role'] || 'guest'
    }
  };

  try {
    const isAllowed = routeRule.evaluate(context); // a JS value: true / false
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

## Python + FastAPI

Compute a discount at the API boundary with `datalogic-py`. The same shape works for tax or shipping rules.

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

## Rust + Axum

A feature-flag endpoint that compiles its rule at startup and opens a session per request. `session.eval_into::<T, _>(...)` needs the `serde_json` feature:
`datalogic-rs = { version = "5", features = ["serde_json"] }`.

```rust
use axum::{routing::post, Json, Router};
use datalogic_rs::{Engine, Logic};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

struct AppState {
    engine: Engine,
    rule: Logic,
}

#[derive(Deserialize, Serialize)]
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
    // A session owns the arena for this request's evaluation
    let mut session = state.engine.session();

    // An evaluation error counts as "flag off"
    let result = session.eval_into::<bool, _>(
        &state.rule,
        &serde_json::to_value(payload).unwrap()
    ).unwrap_or(false);

    Json(FlagResponse { enabled: result })
}

#[tokio::main]
async fn main() {
    let engine = Engine::new();
    // Rule: enable the beta feature for beta users or users in CA
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
