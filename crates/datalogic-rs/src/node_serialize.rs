//! Reverse-compilation: walk a [`CompiledNode`] tree and produce its
//! canonical JSONLogic string.
//!
//! Used by [`crate::Logic::to_json`] and (when the `trace` feature is on) by
//! the trace UI's [`crate::ExpressionNode`] builder. The output reflects the
//! *compiled* shape — constant-folded sub-expressions appear as literals,
//! since the original operator is gone by then. Re-parsing the output
//! through [`crate::Engine::compile`] yields a [`crate::Logic`] that
//! evaluates identically.

use crate::CompiledNode;
use crate::node::PathSegment;
use crate::opcode::OpCode;

/// Serialise an entire compiled tree as a JSONLogic string.
pub(crate) fn node_to_json_string(node: &CompiledNode) -> String {
    match node {
        CompiledNode::Value { value, .. } => value.to_json_string(),
        CompiledNode::Array { nodes, .. } => {
            let items: Vec<String> = nodes.iter().map(node_to_json_string).collect();
            format!("[{}]", items.join(", "))
        }
        CompiledNode::BuiltinOperator { opcode, args, .. } => builtin_to_json_string(opcode, args),
        CompiledNode::CustomOperator(data) => custom_to_json_string(&data.name, &data.args),
        // Memo wrappers are invisible in serialized output — `to_json()`
        // of a CSE'd tree is byte-identical to the unwrapped tree.
        CompiledNode::Cse(data) => node_to_json_string(&data.inner),
        #[cfg(feature = "templating")]
        CompiledNode::StructuredObject(data) => structured_to_json_string(&data.fields),
        CompiledNode::Var {
            scope_level,
            segments,
            default_value,
            ..
        } => compiled_var_to_json_string(*scope_level, segments, default_value.as_deref()),
        #[cfg(feature = "ext-control")]
        CompiledNode::Exists(data) => compiled_exists_to_json_string(&data.segments),
        #[cfg(feature = "error-handling")]
        CompiledNode::Throw(data) => {
            if let datavalue::OwnedDataValue::Object(pairs) = &data.error {
                if let Some((_, datavalue::OwnedDataValue::String(s))) =
                    pairs.iter().find(|(k, _)| k == "type")
                {
                    return format!("{{\"throw\": \"{}\"}}", s);
                }
            }
            format!("{{\"throw\": {}}}", data.error.to_json_string())
        }
        CompiledNode::Missing(data) => {
            let parts: Vec<String> = data
                .args
                .iter()
                .map(|a| match a {
                    crate::node::CompiledMissingArg::Now((path, _)) => format!("\"{}\"", path),
                    crate::node::CompiledMissingArg::Later(n) => node_to_json_string(n),
                })
                .collect();
            format!("{{\"missing\": [{}]}}", parts.join(", "))
        }
        CompiledNode::MissingSome(data) => {
            let min_str = match &data.min_present {
                crate::node::CompiledMissingMin::Now(n) => n.to_string(),
                crate::node::CompiledMissingMin::Later(n) => node_to_json_string(n),
            };
            let paths_str = match &data.paths {
                crate::node::CompiledMissingPaths::Now(paths) => {
                    let items: Vec<String> =
                        paths.iter().map(|(p, _)| format!("\"{}\"", p)).collect();
                    format!("[{}]", items.join(", "))
                }
                crate::node::CompiledMissingPaths::Later(n) => node_to_json_string(n),
            };
            format!("{{\"missing_some\": [{}, {}]}}", min_str, paths_str)
        }
        CompiledNode::InvalidArgs { .. } => "{\"<invalid args>\": null}".to_string(),
    }
}

/// Render an operator's argument list: a single arg inlines, multiple args
/// become a JSON array. Shared by the builtin and custom operator renderers.
fn args_to_json_string(args: &[CompiledNode]) -> String {
    if args.len() == 1 {
        node_to_json_string(&args[0])
    } else {
        let items: Vec<String> = args.iter().map(node_to_json_string).collect();
        format!("[{}]", items.join(", "))
    }
}

pub(crate) fn builtin_to_json_string(opcode: &OpCode, args: &[CompiledNode]) -> String {
    format!("{{\"{}\": {}}}", opcode.as_str(), args_to_json_string(args))
}

pub(crate) fn custom_to_json_string(name: &str, args: &[CompiledNode]) -> String {
    format!("{{\"{}\": {}}}", name, args_to_json_string(args))
}

#[cfg(feature = "templating")]
pub(crate) fn structured_to_json_string(fields: &[(String, CompiledNode)]) -> String {
    let items: Vec<String> = fields
        .iter()
        .map(|(key, node)| format!("\"{}\": {}", key, node_to_json_string(node)))
        .collect();
    format!("{{{}}}", items.join(", "))
}

pub(crate) fn compiled_var_to_json_string(
    scope_level: u32,
    segments: &[PathSegment],
    default_value: Option<&CompiledNode>,
) -> String {
    if scope_level == 0 {
        let path: String = segments
            .iter()
            .map(|seg| match seg {
                PathSegment::Field(s) | PathSegment::FieldOrIndex(s, _) => s.to_string(),
                PathSegment::Index(i) => i.to_string(),
            })
            .collect::<Vec<_>>()
            .join(".");
        match default_value {
            Some(def) => format!("{{\"var\": [\"{}\", {}]}}", path, node_to_json_string(def)),
            None => format!("{{\"var\": \"{}\"}}", path),
        }
    } else {
        let mut parts = vec![format!("[{}]", scope_level)];
        for seg in segments {
            match seg {
                PathSegment::Field(s) | PathSegment::FieldOrIndex(s, _) => {
                    parts.push(format!("\"{}\"", s))
                }
                PathSegment::Index(i) => parts.push(i.to_string()),
            }
        }
        format!("{{\"val\": [{}]}}", parts.join(", "))
    }
}

#[cfg(feature = "ext-control")]
pub(crate) fn compiled_exists_to_json_string(segments: &[PathSegment]) -> String {
    if segments.len() == 1 {
        match &segments[0] {
            PathSegment::Field(s) | PathSegment::FieldOrIndex(s, _) => {
                format!("{{\"exists\": \"{}\"}}", s)
            }
            PathSegment::Index(i) => format!("{{\"exists\": {}}}", i),
        }
    } else {
        let parts: Vec<String> = segments
            .iter()
            .map(|seg| match seg {
                PathSegment::Field(s) | PathSegment::FieldOrIndex(s, _) => format!("\"{}\"", s),
                PathSegment::Index(i) => i.to_string(),
            })
            .collect();
        format!("{{\"exists\": [{}]}}", parts.join(", "))
    }
}
