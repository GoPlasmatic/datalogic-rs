use serde_json::Value;
use crate::{rule::ArgType, Error};
use std::borrow::Cow;

pub struct ValOperator;

impl ValOperator {
    pub fn apply<'a>(&self, arg: &ArgType, context: &'a Value, root: &'a Value, path: &str) -> Result<Cow<'a, Value>, Error> {
        match arg {
            ArgType::Unary(rule) => {
                let value = rule.apply(context, root, path)?;
                match &*value {
                    Value::Array(arr) => {
                        let mut current = context;
                        for key in arr {
                            match access_value(current, key) {
                                Some(value) => current = value,
                                None => return Ok(Cow::Owned(Value::Null))
                            }
                        }
                        Ok(Cow::Borrowed(current))
                    },
                    _ => Ok(match access_value(context, &value) {
                        Some(value) => Cow::Borrowed(value),
                        None => Cow::Owned(Value::Null)
                    })
                }
            },
            ArgType::Multiple(rules) => {
                match rules.len() {
                    0 => Ok(Cow::Borrowed(context)),
                    _ => {
                        let first_value = rules[0].apply(context, root, path)?;
                        if let Value::Array(arr) = &*first_value {
                            if arr.len() == 1 {
                                if let Some(Value::Number(n)) = arr.first() {
                                    if let Some(levels) = n.as_i64() {
                                        let levels = levels.unsigned_abs();
                                        // Check for special case of 'index'
                                        let remaining_rules = &rules[1..];
                                        if remaining_rules.len() == 1 {
                                            if let Ok(value) = remaining_rules[0].apply(context, root, path) {
                                                if value.as_str() == Some("index") {
                                                    // Extract index from path
                                                    if let Some(idx) = self.extract_last_index(path) {
                                                        return Ok(Cow::Owned(Value::Number(idx.into())));
                                                    }
                                                }
                                            }
                                        }

                                        // Get target path by climbing up n levels
                                        let target_path = if path.is_empty() || path == "$" {
                                            "$".to_string()
                                        } else {
                                            let segments: Vec<&str> = path.split(['.', '['])
                                                .filter(|s| !s.is_empty())
                                                .collect();
                                            if segments.len() <= levels as usize {
                                                "$".to_string()
                                            } else {
                                                let new_len = segments.len() - levels as usize;
                                                format!("${}", segments[..new_len]
                                                    .iter()
                                                    .map(|s| {
                                                        if let Some(s) = s.strip_suffix(']') {
                                                            format!("[{}]", &s[..s.len()-1])
                                                        } else {
                                                            format!(".{}", s)
                                                        }
                                                    })
                                                    .collect::<String>())
                                            }
                                        };

                                        // Navigate to target context
                                        let mut current = if target_path == "$" {
                                            root
                                        } else {
                                            let segments: Vec<&str> = target_path[2..].split(['.', '['])
                                                .filter(|s| !s.is_empty())
                                                .map(|s| if let Some(s) = s.strip_suffix(']') { &s[..s.len()-1] } else { s })
                                                .collect();
                                            let mut curr = root;
                                            for segment in segments {
                                                if let Some(value) = access_value(curr, &Value::String(segment.to_string())) {
                                                    curr = value;
                                                } else {
                                                    return Ok(Cow::Owned(Value::Null));
                                                }
                                            }
                                            curr
                                        };

                                        // Process remaining rules
                                        for rule in &rules[1..] {
                                            let value = rule.apply(current, root, &target_path)?;
                                            if let Some(val) = access_value(current, &value) {
                                                current = val;
                                            } else {
                                                return Ok(Cow::Owned(Value::Null));
                                            }
                                        }
                                        return Ok(Cow::Borrowed(current));
                                    }
                                }
                            }
                        }

                        // Original logic for non-jumping cases
                        let mut current = context;
                        if let Some(val) = access_value(current, &first_value) {
                            current = val;
                        } else {
                            return Ok(Cow::Owned(Value::Null));
                        }

                        for rule in &rules[1..] {
                            let value = rule.apply(current, root, path)?;
                            if let Some(val) = access_value(current, &value) {
                                current = val;
                            } else {
                                return Ok(Cow::Owned(Value::Null));
                            }
                        }
                        Ok(Cow::Borrowed(current))
                    }
                }
            },
        }
    }

    fn extract_last_index(&self, path: &str) -> Option<u64> {
        // Handle paths like "$.foo.bar[1]" or "$.items[2].value"
        path.split(['.', '['])
            .filter(|s| !s.is_empty())
            .next_back()
            .and_then(|s| {
                if let Some(s) = s.strip_suffix(']') {
                    s.parse::<u64>().ok()
                } else {
                    None
                }
            })
    }
}

pub struct ExistsOperator;

impl ExistsOperator {
    pub fn apply<'a>(&self, arg: &ArgType, context: &'a Value, root: &'a Value, path: &str) -> Result<Cow<'a, Value>, Error> {
        match arg {
            ArgType::Unary(rule) => {
                let value = rule.apply(context, root, path)?;
                match &*value {
                    Value::Array(arr) => {
                        let mut current = context;
                        for key in arr {
                            match access_value(current, key) {
                                Some(value) => current = value,
                                None => return Ok(Cow::Owned(Value::Bool(false))),
                            }
                        }
                        Ok(Cow::Owned(Value::Bool(true)))
                    },
                    _ => Ok(Cow::Owned(Value::Bool(access_value(context, &value).is_some())))
                }
            },
            ArgType::Multiple(rules) => {
                match rules.len() {
                    0 => Ok(Cow::Owned(Value::Bool(false))),
                    _ => {
                        let mut current = context;
                        for rule in rules {
                            let value = rule.apply(current, root, path)?;
                            match &*value {
                                Value::Array(arr) => {
                                    for key in arr {
                                        match access_value(current, key) {
                                            Some(value) => current = value,
                                            None => return Ok(Cow::Owned(Value::Bool(false)))
                                        }
                                    }
                                },
                                _ => {
                                    match access_value(current, &value) {
                                        Some(value) => current = value,
                                        None => return Ok(Cow::Owned(Value::Bool(false)))
                                    }
                                }
                            }
                        }
                        Ok(Cow::Owned(Value::Bool(true)))
                    }
                }
            }
        }
    }
}

#[inline(always)]
fn access_value<'a>(data: &'a Value, key: &Value) -> Option<&'a Value> {
    match (data, key) {
        (Value::Null, _) => None,
        (Value::Object(map), Value::String(s)) => map.get(s),
        (Value::Array(arr), Value::Number(n)) => {
            if let Some(idx) = n.as_u64() {
                arr.get(idx as usize)
            } else {
                None
            }
        }
        (Value::Array(arr), Value::String(s)) => {
            if let Ok(idx) = s.parse::<f64>() {
                arr.get(idx as usize)
            } else {
                None
            }
        }
        _ => None
    }
}