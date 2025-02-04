use serde_json::Value;

pub trait ValueCoercion {
    fn coerce_to_bool(&self) -> bool;
    fn coerce_to_number(&self) -> f64;
    fn coerce_to_string(&self) -> String;
    fn coerce_append(result: &mut String, value: &Value);
}

impl ValueCoercion for Value {
    #[inline(always)]
    fn coerce_to_bool(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::Number(n) => n.as_f64().unwrap_or(0.0) != 0.0,
            Value::String(s) => !s.is_empty(),
            Value::Array(a) => !a.is_empty(),
            Value::Object(o) => !o.is_empty(),
            Value::Null => false,
        }
    }

    #[inline(always)]
    fn coerce_to_number(&self) -> f64 {
        const TRUE_VAL: f64 = 1.0;
        const FALSE_VAL: f64 = 0.0;
        
        match self {
            Value::Number(n) => n.as_f64().unwrap_or(FALSE_VAL),
            Value::String(s) => {
                if s.is_empty() { return FALSE_VAL; }
                if let Some(c) = s.chars().next() {
                    if c.is_ascii_digit() || c == '-' {
                        return s.parse::<i64>()
                            .map(|i| i as f64)
                            .unwrap_or_else(|_| s.parse::<f64>().unwrap_or(FALSE_VAL));
                    }
                }
                FALSE_VAL
            }
            Value::Bool(true) => TRUE_VAL,
            _ => FALSE_VAL,
        }
    }

    #[inline(always)]
    fn coerce_to_string(&self) -> String {
        match self {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Null => "null".to_string(),
            Value::Array(arr) => {
                let mut result = String::with_capacity(arr.len() * 8);
                for item in arr.iter() {
                    Self::coerce_append(&mut result, item);
                }
                result
            },
            Value::Object(_) => "[object Object]".to_string(),
        }
    }

    #[inline(always)]
    fn coerce_append(result: &mut String, value: &Value) {
        match value {
            Value::String(s) => result.push_str(s),
            Value::Number(n) => result.push_str(&n.to_string()),
            Value::Bool(b) => result.push_str(if *b { "true" } else { "false" }),
            Value::Null => result.push_str("null"),
            Value::Array(arr) => {
                for item in arr.iter() {
                    Self::coerce_append(result, item);
                }
            },
            Value::Object(_) => result.push_str("[object Object]"),
        }
    }

}

pub trait ValueConvert {
    fn to_value(&self) -> Value;
}

impl ValueConvert for f64 {
    #[inline(always)]
    fn to_value(&self) -> Value {
        const ZERO_FRACT: f64 = 0.0;
        if self.fract() == ZERO_FRACT {
            Value::from(*self as i64)
        } else {
            Value::from(*self)
        }
    }
}
