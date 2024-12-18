use datalogic_rs::*;
use serde_json::json;

fn main() {
    // Example 1: Dynamic Pricing Rules
    let pricing_logic = json!({
        "if": [
            // If holiday season (month 11-12) and premium member
            {"and": [
                {"in": [{"var": "date.month"}, [11, 12]]},
                {"==": [{"var": "user.membership"}, "premium"]}
            ]},
            // Then apply 25% discount
            {"*": [{"var": "cart.total"}, 0.75]},
            // Else if cart total > 200
            {">": [{"var": "cart.total"}, 200]},
            // Then apply 10% discount
            {"*": [{"var": "cart.total"}, 0.90]},
            // Else no discount
            {"var": "cart.total"}
        ]
    });

    // Example 2: Loan Approval Rules
    let loan_eligibility = json!({
        "and": [
            // Age between 21 and 65
            {">=": [{"var": "applicant.age"}, 21]},
            {"<=": [{"var": "applicant.age"}, 65]},
            // Credit score above 700
            {">": [{"var": "applicant.credit_score"}, 700]},
            // Debt-to-income ratio below 40%
            {"<": [
                {"/": [
                    {"var": "applicant.monthly_debt"},
                    {"var": "applicant.monthly_income"}
                ]},
                0.4
            ]},
            // Employment history > 2 years
            {">": [{"var": "applicant.years_employed"}, 2]}
        ]
    });

    // Example 3: Fraud Detection Rules
    let fraud_check = json!({
        "or": [
            // Multiple high-value transactions in short time
            {"and": [
                {">": [{"var": "transaction.amount"}, 1000]},
                {"<": [
                    {"-": [
                        {"var": "transaction.timestamp"},
                        {"var": "last_transaction.timestamp"}
                    ]},
                    300 // 5 minutes in seconds
                ]}
            ]},
            // Transaction from unusual location
            {"!": {"in": [
                {"var": "transaction.country"},
                {"var": "user.usual_countries"}
            ]}},
            // Unusual shopping pattern
            {"and": [
                {">": [{"var": "daily_transaction_count"}, 10]},
                {">": [{"var": "transaction.amount"}, {"*": [{"var": "user.average_transaction"}, 3]}]}
            ]}
        ]
    });

    // Example data for testing
    let transaction_data = json!({
        "transaction": {
            "amount": 1200.00,
            "timestamp": 1677649200,
            "country": "FR"
        },
        "last_transaction": {
            "timestamp": 1677649100
        },
        "user": {
            "usual_countries": ["US", "CA", "GB"],
            "average_transaction": 200
        },
        "daily_transaction_count": 12
    });

    let loan_data = json!({
        "applicant": {
            "age": 35,
            "credit_score": 720,
            "monthly_debt": 2000,
            "monthly_income": 6000,
            "years_employed": 5
        }
    });

    let pricing_data = json!({
        "date": {
            "month": 12
        },
        "user": {
            "membership": "premium"
        },
        "cart": {
            "total": 300.00
        }
    });

    // Test the rules
    let pricing_rule = Rule::from_value(&pricing_logic).unwrap();
    let final_price = JsonLogic::apply(&pricing_rule, &pricing_data).unwrap();
    println!("Final price after discounts: ${}", final_price);

    let loan_eligibility_rule = Rule::from_value(&loan_eligibility).unwrap();
    let is_eligible = JsonLogic::apply(&loan_eligibility_rule, &loan_data).unwrap();
    println!("Loan application approved: {}", is_eligible);

    let fraud_check_rule = Rule::from_value(&fraud_check).unwrap();
    let is_fraudulent = JsonLogic::apply(&fraud_check_rule, &transaction_data).unwrap();
    println!("Transaction flagged as fraudulent: {}", is_fraudulent);
}