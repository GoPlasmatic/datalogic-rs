//! Reverse-compilation: walk a [`CompiledNode`] tree and produce its
//! canonical JSONLogic string.
//!
//! Used by [`crate::Logic::to_json`] and (when the `trace` feature is on) by
//! the trace UI's [`crate::ExpressionNode`] builder. The output reflects the
//! *compiled* shape — constant-folded sub-expressions appear as literals,
//! since the original operator is gone by then. Re-parsing the output
//! through [`crate::Engine::compile`] yields a [`crate::Logic`] that
//! evaluates identically.
//!
//! Every string that lands in the output — paths, operator names, template
//! keys, `throw` types — goes through [`push_json_str`], so text holding
//! quotes, backslashes or control characters stays valid JSON.

use crate::CompiledNode;
use crate::OpCode;
use crate::node::PathSegment;

/// Append `s` to `out` as a quoted JSON string literal.
///
/// The escapes match datavalue's emitter: `\"`, `\\`, `\n`, `\r`, `\t`,
/// `\b`, `\f`, `\u00XX` for the other control characters, everything else
/// verbatim. A string therefore renders the same here as it does inside a
/// literal value.
pub(crate) fn push_json_str(out: &mut String, s: &str) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    out.reserve(s.len() + 2);
    out.push('"');
    let mut run = 0;
    for (i, b) in s.bytes().enumerate() {
        let escape = match b {
            b'"' => "\\\"",
            b'\\' => "\\\\",
            b'\n' => "\\n",
            b'\r' => "\\r",
            b'\t' => "\\t",
            0x08 => "\\b",
            0x0C => "\\f",
            0x00..=0x1F => "",
            _ => continue,
        };
        // Every byte matched above is ASCII, so `i` is a char boundary.
        out.push_str(&s[run..i]);
        if escape.is_empty() {
            out.push_str("\\u00");
            out.push(HEX[(b >> 4) as usize] as char);
            out.push(HEX[(b & 0x0F) as usize] as char);
        } else {
            out.push_str(escape);
        }
        run = i + 1;
    }
    out.push_str(&s[run..]);
    out.push('"');
}

/// `s` as a quoted JSON string literal; see [`push_json_str`].
pub(crate) fn json_str(s: &str) -> String {
    let mut out = String::new();
    push_json_str(&mut out, s);
    out
}

/// The text of a path segment.
#[inline]
fn segment_str(seg: &PathSegment) -> &str {
    match seg {
        PathSegment::Field(s) | PathSegment::FieldOrIndex(s, _) => s,
    }
}

/// `segments` as a JSON array of strings.
fn segments_array(segments: &[PathSegment]) -> String {
    let items: Vec<String> = segments.iter().map(|s| json_str(segment_str(s))).collect();
    format!("[{}]", items.join(", "))
}

/// `{"<name>": <args>}`, with the name escaped.
fn call(name: &str, args: &str) -> String {
    let mut out = String::with_capacity(name.len() + args.len() + 6);
    out.push('{');
    push_json_str(&mut out, name);
    out.push_str(": ");
    out.push_str(args);
    out.push('}');
    out
}

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
            if let datavalue::OwnedDataValue::Object(pairs) = &data.error
                && let Some((_, datavalue::OwnedDataValue::String(s))) =
                    pairs.iter().find(|(k, _)| k == "type")
            {
                return call("throw", &json_str(s));
            }
            call("throw", &data.error.to_json_string())
        }
        CompiledNode::Missing(data) => {
            let parts: Vec<String> = data
                .args
                .iter()
                .map(|a| match a {
                    crate::node::CompiledMissingArg::Now((path, _)) => json_str(path),
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
                    let items: Vec<String> = paths.iter().map(|(p, _)| json_str(p)).collect();
                    format!("[{}]", items.join(", "))
                }
                crate::node::CompiledMissingPaths::Later(n) => node_to_json_string(n),
            };
            format!("{{\"missing_some\": [{}, {}]}}", min_str, paths_str)
        }
        // Re-emit the offending rule verbatim, not a placeholder. The node
        // keeps both the misused operator and its raw arguments, so this
        // recompiles to the very same node and raises the same error.
        //
        // The previous `{"<invalid args>": null}` was not JSONLogic the
        // engine could read back: in templating mode it re-parsed as an
        // ordinary output field, turning an erroring rule into a
        // successful one, and outside it as an unknown operator, losing
        // which op actually failed.
        CompiledNode::InvalidArgs { op_name, args, .. } => call(op_name, &args.to_json_string()),
    }
}

/// Render an operator's argument list as a JSON array, or a single
/// argument in place when `inline` allows and that reads back the same: an
/// argument that renders as an array would be read back as the argument
/// list itself (`{"max": [[1, 2]]}` is not `{"max": [1, 2]}`). Shared by
/// the builtin and custom operator renderers.
fn args_to_json_string(args: &[CompiledNode], inline: bool) -> String {
    if let [arg] = args {
        let rendered = node_to_json_string(arg);
        if inline && !rendered.starts_with('[') {
            return rendered;
        }
        return format!("[{rendered}]");
    }
    let items: Vec<String> = args.iter().map(node_to_json_string).collect();
    format!("[{}]", items.join(", "))
}

pub(crate) fn builtin_to_json_string(opcode: &OpCode, args: &[CompiledNode]) -> String {
    // `and` / `or` / `if` read only an argument array: a lone argument
    // written in place is an error, not that argument.
    let inline = opcode.meta().args_form != crate::operators::meta::ArgsForm::ArrayOnly;
    call(opcode.as_str(), &args_to_json_string(args, inline))
}

pub(crate) fn custom_to_json_string(name: &str, args: &[CompiledNode]) -> String {
    call(name, &args_to_json_string(args, true))
}

#[cfg(feature = "templating")]
pub(crate) fn structured_to_json_string(fields: &[(String, CompiledNode)]) -> String {
    let items: Vec<String> = fields
        .iter()
        .map(|(key, node)| format!("{}: {}", json_str(key), node_to_json_string(node)))
        .collect();
    format!("{{{}}}", items.join(", "))
}

/// Render a compiled `var` / `val` read.
///
/// `var` and `val` compile to the same node, so the name the rule used is
/// gone. The read is written as a `var` path, its segments joined with
/// `.`, whenever that reads back the same segments. A `val` key that holds
/// a `.` does not: `{"val": "a.b"}` reads the key `a.b`, while
/// `{"var": "a.b"}` reads `a` then `b`. Such a read is written in the `val`
/// form, as a list of literal keys that is never split. A level marker
/// (`scope_level > 0`) exists only in the `val` form.
fn compiled_var_to_json_string(
    scope_level: u32,
    segments: &[PathSegment],
    default_value: Option<&CompiledNode>,
) -> String {
    if scope_level > 0 {
        let mut list = format!("[[{}]", scope_level);
        for seg in segments {
            list.push_str(", ");
            push_json_str(&mut list, segment_str(seg));
        }
        list.push(']');
        return call("val", &list);
    }
    // A `var` path is split on `.`, and an empty one reads the whole
    // context, so a dotted segment or a lone empty one needs the `val`
    // form. Only `val` builds those, and `val` takes no default.
    let needs_val = match segments {
        [seg] => segment_str(seg).is_empty() || segment_str(seg).contains('.'),
        _ => segments.iter().any(|s| segment_str(s).contains('.')),
    };
    if needs_val && default_value.is_none() {
        return match segments {
            [seg] => call("val", &json_str(segment_str(seg))),
            _ => call("val", &segments_array(segments)),
        };
    }
    let path = segments
        .iter()
        .map(segment_str)
        .collect::<Vec<_>>()
        .join(".");
    match default_value {
        Some(def) => call(
            "var",
            &format!("[{}, {}]", json_str(&path), node_to_json_string(def)),
        ),
        None => call("var", &json_str(&path)),
    }
}

#[cfg(feature = "ext-control")]
pub(crate) fn compiled_exists_to_json_string(segments: &[PathSegment]) -> String {
    match segments {
        [seg] => call("exists", &json_str(segment_str(seg))),
        _ => call("exists", &segments_array(segments)),
    }
}
