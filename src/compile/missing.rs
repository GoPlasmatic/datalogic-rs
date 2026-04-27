//! Compilation for `missing` and `missing_some` — pre-parse static path
//! arguments so runtime evaluation can skip the parser.

use datavalue::{NumberValue, OwnedDataValue};

use crate::node::{
    CompileCtx, CompiledMissingArg, CompiledMissingData, CompiledMissingMin, CompiledMissingPaths,
    CompiledMissingSomeData, CompiledNode, PathSegment,
};

use super::path::parse_path_segments;

/// Build a `CompiledMissing` from raw operator args. Each arg that is a
/// literal string is pre-parsed; everything else (including literal arrays
/// of strings, var lookups, computed expressions) goes through the runtime
/// dispatch path.
pub(super) fn compile_missing(args: Box<[CompiledNode]>, ctx: &mut CompileCtx) -> CompiledNode {
    let mapped: Vec<CompiledMissingArg> = args
        .into_vec()
        .into_iter()
        .map(|arg| match &arg {
            CompiledNode::Value {
                value: OwnedDataValue::String(s),
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
            value: OwnedDataValue::Number(n),
            ..
        }) => match n {
            NumberValue::Integer(v) if v >= 0 => CompiledMissingMin::Static(v as usize),
            NumberValue::Integer(_) => CompiledMissingMin::Static(0),
            NumberValue::Float(f) => CompiledMissingMin::Static(f.max(0.0) as usize),
        },
        Some(other) => CompiledMissingMin::Dynamic(other),
        None => CompiledMissingMin::Static(1),
    };

    let paths = match paths_arg {
        Some(CompiledNode::Value {
            value: OwnedDataValue::Array(arr),
            ..
        }) if arr.iter().all(|v| matches!(v, OwnedDataValue::String(_))) => {
            let parsed: Vec<(Box<str>, Box<[PathSegment]>)> = arr
                .into_iter()
                .map(|v| {
                    let s = match v {
                        OwnedDataValue::String(s) => s,
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
