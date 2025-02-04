pub mod expression;
use std::str::FromStr;

pub use expression::*;

use serde_json::Value;
use smallvec::SmallVec;

#[derive(Debug)]
pub enum ArrayItem {
    Literal(Value),
    Expression(Rule),
}

#[derive(Debug)]
pub enum Instruction {
    PushValue(Value),
    Expr(Rule),
    ArrayExpr(SmallVec<[ArrayItem; 8]>),

    CallOp { 
        op: OpType, 
        arg_count: usize,
    },
}

#[derive(Debug)]
pub struct Rule {
    instructions: Box<SmallVec<[Instruction; 16]>>,
}

impl Rule {
    fn new() -> Self {
        Self {
            instructions: Box::new(SmallVec::new()),
        }
    }

    pub fn from_value(expr_json: &Value) -> Result<Rule, Error> {
        let mut rule = Rule::new();
        rule.parse_compile_value(expr_json);
        Ok(rule)
    }

    fn evaluate_static(expr_json: &Value) -> Option<Value> {
        match expr_json {
            Value::Object(map) if map.len() == 1 => {
                let (op, val) = map.into_iter().next().unwrap();
                let optype = OpType::from_str(op).unwrap();
                if optype == OpType::Var || optype == OpType::Missing || optype == OpType::MissingSome {
                    return None;
                }
                
                match val {
                    Value::Array(items) => {
                        let static_items: Option<Vec<Value>> = items.iter()
                            .map(Self::evaluate_static)
                            .collect();
                        
                        if let Some(static_items) = static_items {
                            match optype {
                                OpType::Add => evaluate_add(&static_items).ok(),
                                OpType::Multiply => evaluate_mul(&static_items).ok(),
                                OpType::Subtract => evaluate_sub(&static_items).ok(),
                                OpType::Divide => evaluate_div(&static_items).ok(),
                                OpType::Modulo => evaluate_mod(&static_items).ok(),
                                OpType::Max => evaluate_max(&static_items).ok(),
                                OpType::Min => evaluate_min(&static_items).ok(),

                                OpType::Equals => Some(Value::Bool(evaluate_equals(&static_items))),
                                OpType::NotEquals => Some(Value::Bool(evaluate_not_equals(&static_items))),
                                OpType::StrictEquals => Some(Value::Bool(evaluate_strict_equals(&static_items))),
                                OpType::StrictNotEquals => Some(Value::Bool(evaluate_strict_not_equals(&static_items))),
                                OpType::GreaterThan => Some(Value::Bool(evaluate_greater_than(&static_items))),
                                OpType::GreaterThanEqual => Some(Value::Bool(evaluate_greater_than_or_equals(&static_items))),
                                OpType::LessThan => Some(Value::Bool(evaluate_less_than(&static_items))),
                                OpType::LessThanEqual => Some(Value::Bool(evaluate_less_than_or_equals(&static_items))),

                                OpType::And => evaluate_and(&static_items).ok(),
                                OpType::Or => evaluate_or(&static_items).ok(),
                                OpType::Not => Some(Value::Bool(evaluate_not(&static_items))),
                                OpType::DoubleBang => Some(Value::Bool(evaluate_double_bang(&static_items))),

                                OpType::If => evaluate_if(&static_items).ok(),
                                OpType::Ternary => evaluate_ternary(&static_items).ok(),

                                OpType::In => evaluate_in(&static_items).ok(),
                                OpType::Cat => evaluate_cat(&static_items).ok(),
                                OpType::Substr => evaluate_substr(&static_items).ok(),

                                OpType::Map | OpType::Filter | OpType::All | OpType::Some | OpType::None => {
                                    let predicate = Rule::from_value(&static_items[1]).unwrap();
                                    match optype {
                                        OpType::Map => evaluate_map(&static_items, &predicate).ok(),
                                        OpType::Filter => evaluate_filter(&static_items, &predicate).ok(),
                                        OpType::All | OpType::Some | OpType::None => evaluate_array_op(&optype, &static_items, &predicate).ok(),
                                        _ => None
                                    }
                                }
                                OpType::Merge => evaluate_merge(&static_items).ok(),

                                _ => None
                            }
                        } else {
                            None
                        }
                    }
                    _ => None
                }
            }
            Value::Array(items) => {
                let static_items: Option<Vec<Value>> = items.iter()
                    .map(Self::evaluate_static)
                    .collect();
                
                    static_items.map(Value::Array)
            }
            _ => Some(expr_json.clone())
        }
    }

    fn is_flat_predicate(expr_json: &Value) -> (bool, OpType) {
        let map = match expr_json {
            Value::Object(map) => map,
            _ => return (false, OpType::Invalid)
        };

        let (op, arr) = map.into_iter().next().unwrap();
        let arr = match arr {
            Value::Array(arr) => arr,
            _ => return (false, OpType::Invalid)
        };

        for item in arr {
            match item {
                Value::Object(map) => {
                    let (_, val) = map.into_iter().next().unwrap();
                    let val = val.as_str().unwrap();
                    if val != "current" && val != "accumulator" {
                        return (false, OpType::Invalid)
                    }
                }
                _ => continue
            }
        }

        (true, OpType::from_str(op).unwrap())
    }

    fn parse_compile_value(&mut self, expr_json: &Value) {
        if let Some(static_result) = Self::evaluate_static(expr_json) {
            self.instructions.push(Instruction::PushValue(static_result));
            return;
        }

        match expr_json {
            Value::Object(map) if map.len() == 1 => {
                let (op, val) = map.into_iter().next().unwrap();
                let optype = OpType::from_str(op).unwrap();
    
                match val {
                    Value::Array(items) => {
                        match optype {
                            OpType::Map | OpType::Filter | OpType::All | OpType::Some | OpType::None => {
                                self.parse_compile_value(&items[0]);
                                let expr = Rule::from_value(&items[1]).unwrap();
                                self.instructions.push(Instruction::Expr(expr));
                                self.instructions.push(Instruction::CallOp { op: optype, arg_count: 2 });
                            }
                            OpType::Reduce => {
                                let (nested, nested_op) = Self::is_flat_predicate(&items[1]);
                                if nested {
                                    self.parse_compile_value(&items[0]);
                                    self.instructions.push(Instruction::CallOp { op: nested_op.clone(), arg_count: 1 });
                                    self.parse_compile_value(&items[2]);
                                    self.instructions.push(Instruction::CallOp { op: nested_op, arg_count: 2 });
                                } else {
                                    self.parse_compile_value(&items[0]);
                                    let expr = Rule::from_value(&items[1]).unwrap();
                                    self.instructions.push(Instruction::Expr(expr));
                                    self.parse_compile_value(&items[2]);
                                    self.instructions.push(Instruction::CallOp { op: optype, arg_count: 2 });
                                }
                            }
                            OpType::In => {
                                self.parse_compile_value(&items[0]);
                                self.parse_compile_value(&items[1]);
                                self.instructions.push(Instruction::CallOp { op: optype, arg_count: 2 });
                            }
                            OpType::MissingSome => {
                                self.parse_compile_value(&items[0]);
                                self.parse_compile_value(&items[1]);
                                self.instructions.push(Instruction::CallOp { op: optype, arg_count: 2 });
                            }
                            _ => {
                                let arg_count = items.len();
                                for item in items {
                                    self.parse_compile_value(item);
                                }
                                self.instructions.push(Instruction::CallOp { op: optype, arg_count });
                            }
                        }
                    }
                    single => {
                        self.parse_compile_value(single);
                        self.instructions.push(Instruction::CallOp { op: optype, arg_count: 1 });
                    }
                }
            }
            Value::Array(items) => {
                let mut array_items: SmallVec<[ArrayItem; 8]> = SmallVec::new();
                for item in items {
                    match item {
                        Value::Object(_) => {
                            let mut rule = Rule::new();
                            rule.parse_compile_value(item);                            
                            array_items.push(ArrayItem::Expression(rule));
                        }
                        _ => array_items.push(ArrayItem::Literal(item.clone()))
                    }
                }
                self.instructions.push(Instruction::ArrayExpr(array_items));
            }
            other => {
                self.instructions.push(Instruction::PushValue(other.clone()))
            }
        }
    }

    pub fn apply(&self, data: &Value) -> EvalResult {
        let mut stack = SmallVec::<[Value; 24]>::with_capacity(12);
        let mut predicate: Option<&Rule> = None;

        for instr in self.instructions.iter() {
            match instr {
                Instruction::PushValue(val) => {
                    stack.push(match val {
                        Value::Null => Value::Null,
                        Value::Bool(b) => Value::Bool(*b),
                        Value::Number(n) => n.as_f64().unwrap().to_value(),
                        Value::String(s) => Value::from(s.as_str()),
                        _ => val.clone()
                    });
                }
                Instruction::CallOp { op, arg_count } => {
                    let start = stack.len().saturating_sub(*arg_count);
                    let args = &stack[start..];

                    let result = match op {
                        OpType::Var => {
                            let path = args.first().unwrap_or(&Value::Null);
                            let default = args.get(1);
                            evaluate_var(path, data, default)
                        }
                        OpType::Add => evaluate_add(&args),
                        OpType::Subtract => evaluate_sub(&args),
                        OpType::Multiply => evaluate_mul(&args),
                        OpType::Divide => evaluate_div(&args),
                        OpType::Modulo => evaluate_mod(&args),
                        OpType::Max => evaluate_max(&args),
                        OpType::Min => evaluate_min(&args),
            
                        OpType::Equals => Ok(Value::Bool(evaluate_equals(&args))),
                        OpType::NotEquals => Ok(Value::Bool(evaluate_not_equals(&args))),
                        OpType::StrictEquals => Ok(Value::Bool(evaluate_strict_equals(&args))),
                        OpType::StrictNotEquals => Ok(Value::Bool(evaluate_strict_not_equals(&args))),
                        OpType::GreaterThan => Ok(Value::Bool(evaluate_greater_than(&args))),
                        OpType::GreaterThanEqual => Ok(Value::Bool(evaluate_greater_than_or_equals(&args))),
                        OpType::LessThan => Ok(Value::Bool(evaluate_less_than(&args))),
                        OpType::LessThanEqual => Ok(Value::Bool(evaluate_less_than_or_equals(&args))),
                    
                        OpType::And => evaluate_and(&args),
                        OpType::Or => evaluate_or(&args),
                        OpType::Not => Ok(Value::Bool(evaluate_not(&args))),
                        OpType::DoubleBang => Ok(Value::Bool(evaluate_double_bang(&args))),
            
                        OpType::If => evaluate_if(&args),
                        OpType::Ternary => evaluate_ternary(&args),
            
                        OpType::In => evaluate_in(&args),
                        OpType::Cat => evaluate_cat(&args),
                        OpType::Substr => evaluate_substr(&args),
            
                        OpType::Missing => evaluate_missing(&args, data),
                        OpType::MissingSome => evaluate_missing_some(&args, data),
                        OpType::Map | OpType::Filter | OpType::All | 
                        OpType::Some | OpType::None => {
                            match op {
                                OpType::Map => evaluate_map(&args, predicate.unwrap()),
                                OpType::Filter => evaluate_filter(&args, predicate.unwrap()),
                                OpType::All | OpType::Some | OpType::None => evaluate_array_op(op, &args, predicate.unwrap()),
                                _ => unreachable!()
                            }
                        },
                        OpType::Reduce => {
                            evaluate_reduce(&args, predicate.unwrap())
                        },
                        OpType::Merge => evaluate_merge(&args),
                        _ => {
                            println!("Invalid Expression: {:?}", op);
                            Err(super::Error::InvalidExpression("Invalid Expression".to_string()))
                        }
                    };
                    stack.truncate(start);
                    stack.push(result?);
                }
                Instruction::ArrayExpr(items) => {
                    let values: Vec<_> = items.iter()
                        .map(|item| match item {
                            ArrayItem::Expression(expr) => expr.apply(data).unwrap(),
                            ArrayItem::Literal(val) => val.clone()
                        })
                        .collect();
                    stack.push(Value::Array(values));
                }
                Instruction::Expr(expr) => {
                    predicate = Some(expr);
                }
            }
        }
        Ok(stack.pop().unwrap())
    }
}