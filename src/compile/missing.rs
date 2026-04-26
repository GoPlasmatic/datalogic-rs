//! Compilation for `missing` and `missing_some` — pre-parse static path
//! arguments so runtime evaluation can skip the parser.

use serde_json::Value;

use crate::node::{
    CompileCtx, CompiledMissingArg, CompiledMissingData, CompiledMissingMin, CompiledMissingPaths,
    CompiledMissingSomeData, CompiledNode, PathSegment,
};

use super::path::parse_path_segments;

/// Build a `CompiledMissing` from raw operator args. Each arg that is a
/// literal `Value::String` is pre-parsed; everything else (including
/// literal arrays of strings, var lookups, computed expressions) goes
/// through the runtime dispatch path.
pub(super) fn compile_missing(args: Box<[CompiledNode]>, ctx: &mut CompileCtx) -> CompiledNode {
    let mapped: Vec<CompiledMissingArg> = args
        .into_vec()
        .into_iter()
        .map(|arg| match &arg {
            CompiledNode::Value {
                value: Value::String(s),
                ..
            } => {
                let segments = parse_path_segments(s).into_boxed_slice();
                CompiledMissingArg::Static {
                    path: s.clone().into_boxed_str(),
                    segments,
                }
            }
            _ => CompiledMissingArg::Dynamic(arg),
        })
        .collect();
    CompiledNode::CompiledMissing(Box::new(CompiledMissingData {
        id: ctx.next_id(),
        args: mapped.into_boxed_slice(),
    }))
}

/// Build a `CompiledMissingSome` from raw operator args. `min_present`
/// (a literal integer) and a literal array of string paths both pre-parse;
/// anything dynamic falls back to runtime evaluation.
pub(super) fn compile_missing_some(
    args: Box<[CompiledNode]>,
    ctx: &mut CompileCtx,
) -> CompiledNode {
    let mut iter = args.into_vec().into_iter();
    let min_arg = iter.next();
    let paths_arg = iter.next();

    let min_present = match min_arg {
        Some(CompiledNode::Value {
            value: Value::Number(n),
            ..
        }) => match n.as_i64() {
            Some(v) if v >= 0 => CompiledMissingMin::Static(v as usize),
            Some(_) => CompiledMissingMin::Static(0),
            None => CompiledMissingMin::Static(n.as_f64().unwrap_or(0.0).max(0.0) as usize),
        },
        Some(other) => CompiledMissingMin::Dynamic(other),
        None => CompiledMissingMin::Static(1),
    };

    let paths = match paths_arg {
        Some(CompiledNode::Value {
            value: Value::Array(arr),
            ..
        }) if arr.iter().all(|v| matches!(v, Value::String(_))) => {
            let parsed: Vec<(Box<str>, Box<[PathSegment]>)> = arr
                .into_iter()
                .map(|v| {
                    let s = match v {
                        Value::String(s) => s,
                        _ => unreachable!(),
                    };
                    let segments = parse_path_segments(&s).into_boxed_slice();
                    (s.into_boxed_str(), segments)
                })
                .collect();
            CompiledMissingPaths::Static(parsed.into_boxed_slice())
        }
        Some(other) => CompiledMissingPaths::Dynamic(other),
        None => CompiledMissingPaths::Static(Box::new([])),
    };

    CompiledNode::CompiledMissingSome(Box::new(CompiledMissingSomeData {
        id: ctx.next_id(),
        min_present,
        paths,
    }))
}
