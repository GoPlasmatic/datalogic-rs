use serde_json::Value;
use super::coercion::ValueCoercion;

#[inline(always)]
pub fn evaluate_equals(args: &[Value]) -> bool {
    if args.len() < 2 {
        return false;
    }
    
    let first = &args[0];
    match first {
        Value::Number(_) => {
            let first_num = first.coerce_to_number();
            args[1..].iter().all(|val| val.coerce_to_number() == first_num)
        },
        Value::String(_) => {
            let first_str = first.coerce_to_string();
            args[1..].iter().all(|val| val.coerce_to_string() == first_str)
        },
        Value::Bool(_) => {
            let first_bool = first.coerce_to_bool();
            args[1..].iter().all(|val| val.coerce_to_bool() == first_bool)
        },
        _ => false
    }
}

#[inline(always)]
pub fn evaluate_not_equals(args: &[Value]) -> bool {
    if args.len() < 2 {
        return false;
    }
    
    let first = &args[0];
    match first {
        Value::Number(_) => {
            let first_num = first.coerce_to_number();
            args[1..].iter().all(|val| val.coerce_to_number() != first_num)
        },
        Value::String(_) => {
            let first_str = first.coerce_to_string();
            args[1..].iter().all(|val| val.coerce_to_string() != first_str)
        },
        Value::Bool(_) => {
            let first_bool = first.coerce_to_bool();
            args[1..].iter().all(|val| val.coerce_to_bool() != first_bool)
        },
        _ => false
    }
}

#[inline(always)]
pub fn evaluate_strict_equals(args: &[Value]) -> bool {
    if args.len() < 2 {
        return false;
    }
    let first = &args[0];
    args[1..].iter().all(|val| val == first)
}

#[inline(always)]
pub fn evaluate_strict_not_equals(args: &[Value]) -> bool {
    if args.len() < 2 {
        return false;
    }
    let first = &args[0];
    args[1..].iter().all(|val| val != first)
}

#[inline(always)]
pub fn evaluate_greater_than(args: &[Value]) -> bool {
    if args.len() < 2 {
        return false;
    }
    
    let mut prev_num = args[0].coerce_to_number();
    args[1..].iter().all(|val| {
        let curr_num = val.coerce_to_number();
        let result = prev_num > curr_num;
        prev_num = curr_num;
        result
    })
}

#[inline(always)]
pub fn evaluate_greater_than_or_equals(args: &[Value]) -> bool {
    if args.len() < 2 {
        return false;
    }
    
    let mut prev_num = args[0].coerce_to_number();
    args[1..].iter().all(|val| {
        let curr_num = val.coerce_to_number();
        let result = prev_num >= curr_num;
        prev_num = curr_num;
        result
    })
}

#[inline(always)]
pub fn evaluate_less_than(args: &[Value]) -> bool {
    if args.len() < 2 {
        return false;
    }
    
    let mut prev_num = args[0].coerce_to_number();
    args[1..].iter().all(|val| {
        let curr_num = val.coerce_to_number();
        let result = prev_num < curr_num;
        prev_num = curr_num;
        result
    })
}

#[inline(always)]
pub fn evaluate_less_than_or_equals(args: &[Value]) -> bool {
    if args.len() < 2 {
        return false;
    }
    
    let mut prev_num = args[0].coerce_to_number();
    args[1..].iter().all(|val| {
        let curr_num = val.coerce_to_number();
        let result = prev_num <= curr_num;
        prev_num = curr_num;
        result
    })
}