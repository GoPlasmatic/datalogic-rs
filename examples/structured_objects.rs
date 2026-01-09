//! Example demonstrating the structured objects (preserve_structure) feature.
//!
//! When `preserve_structure` mode is enabled, DataLogic treats unknown keys as
//! literal output fields rather than operators. This enables JSON templating
//! where the output structure mirrors the input template.

use datalogic_rs::DataLogic;
use serde_json::json;

fn main() {
    println!("Structured Objects (preserve_structure) Examples\n");
    println!("=================================================\n");

    // Create engine with preserve_structure enabled
    let engine = DataLogic::with_preserve_structure();

    // Example 1: Basic object template
    println!("1. Basic Object Template");
    println!("------------------------");

    let template = json!({
        "name": {"var": "user.name"},
        "email": {"var": "user.email"},
        "active": true
    });

    let compiled = engine.compile(&template).unwrap();
    let data = json!({
        "user": {
            "name": "Alice Johnson",
            "email": "alice@example.com"
        }
    });

    let result = engine.evaluate_owned(&compiled, data).unwrap();
    println!(
        "   Template: {}",
        serde_json::to_string_pretty(&template).unwrap()
    );
    println!(
        "   Result:   {}\n",
        serde_json::to_string_pretty(&result).unwrap()
    );

    // Example 2: Nested object structures
    println!("2. Nested Object Structures");
    println!("---------------------------");

    let template = json!({
        "profile": {
            "firstName": {"var": "first"},
            "lastName": {"var": "last"},
            "fullName": {"cat": [{"var": "first"}, " ", {"var": "last"}]}
        },
        "metadata": {
            "createdAt": {"var": "timestamp"},
            "version": "1.0"
        }
    });

    let compiled = engine.compile(&template).unwrap();
    let data = json!({
        "first": "John",
        "last": "Doe",
        "timestamp": "2024-01-15T10:30:00Z"
    });

    let result = engine.evaluate_owned(&compiled, data).unwrap();
    println!(
        "   Result: {}\n",
        serde_json::to_string_pretty(&result).unwrap()
    );

    // Example 3: Conditional fields
    println!("3. Conditional Fields");
    println!("---------------------");

    let template = json!({
        "status": {"if": [
            {"var": "isActive"},
            "active",
            "inactive"
        ]},
        "tier": {"if": [
            {">": [{"var": "points"}, 1000]},
            "gold",
            {"if": [
                {">": [{"var": "points"}, 500]},
                "silver",
                "bronze"
            ]}
        ]},
        "points": {"var": "points"}
    });

    let compiled = engine.compile(&template).unwrap();

    let data1 = json!({"isActive": true, "points": 1500});
    let result1 = engine.evaluate_owned(&compiled, data1).unwrap();
    println!("   User with 1500 points: {}", result1);

    let data2 = json!({"isActive": false, "points": 750});
    let result2 = engine.evaluate_owned(&compiled, data2).unwrap();
    println!("   User with 750 points:  {}\n", result2);

    // Example 4: Arrays with mapped content
    println!("4. Arrays with Mapped Content");
    println!("-----------------------------");

    let template = json!({
        "items": {"map": [
            {"var": "products"},
            {
                "id": {"var": ".id"},
                "displayName": {"cat": [{"var": ".name"}, " ($", {"var": ".price"}, ")"]},
                "inStock": {">": [{"var": ".quantity"}, 0]}
            }
        ]},
        "totalProducts": {"length": {"var": "products"}}
    });

    let compiled = engine.compile(&template).unwrap();
    let data = json!({
        "products": [
            {"id": 1, "name": "Widget", "price": 9.99, "quantity": 50},
            {"id": 2, "name": "Gadget", "price": 19.99, "quantity": 0},
            {"id": 3, "name": "Gizmo", "price": 14.99, "quantity": 25}
        ]
    });

    let result = engine.evaluate_owned(&compiled, data).unwrap();
    println!(
        "   Result: {}\n",
        serde_json::to_string_pretty(&result).unwrap()
    );

    // Example 5: API response transformation
    println!("5. API Response Transformation");
    println!("------------------------------");

    let template = json!({
        "success": true,
        "data": {
            "user": {
                "id": {"var": "userId"},
                "displayName": {"var": "name"},
                "role": {"if": [
                    {"var": "isAdmin"},
                    "administrator",
                    "user"
                ]}
            },
            "permissions": {"filter": [
                {"var": "allPermissions"},
                {"var": ".enabled"}
            ]}
        },
        "timestamp": {"now": []}
    });

    let compiled = engine.compile(&template).unwrap();
    let data = json!({
        "userId": "usr_12345",
        "name": "Jane Smith",
        "isAdmin": true,
        "allPermissions": [
            {"name": "read", "enabled": true},
            {"name": "write", "enabled": true},
            {"name": "delete", "enabled": false}
        ]
    });

    let result = engine.evaluate_owned(&compiled, data).unwrap();
    println!(
        "   Result: {}\n",
        serde_json::to_string_pretty(&result).unwrap()
    );

    // Example 6: Comparing with vs without preserve_structure
    println!("6. With vs Without preserve_structure");
    println!("--------------------------------------");

    let standard_engine = DataLogic::new();
    let preserve_engine = DataLogic::with_preserve_structure();

    let template = json!({
        "result": {"var": "x"},
        "label": "Output"
    });

    let data = json!({"x": 42});

    // Standard engine treats multi-key objects as errors
    let standard_result = standard_engine.compile(&template);
    println!(
        "   Standard engine: {:?}",
        standard_result.err().map(|e| e.to_string())
    );

    // Preserve engine treats unknown keys as output fields
    let preserve_compiled = preserve_engine.compile(&template).unwrap();
    let preserve_result = preserve_engine
        .evaluate_owned(&preserve_compiled, data)
        .unwrap();
    println!("   Preserve engine: {}", preserve_result);

    println!("\nDone!");
}
