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

/// The text of a path segment.
#[inline]
fn segment_str(seg: &PathSegment) -> &str {
    match seg {
        PathSegment::Field(s) | PathSegment::FieldOrIndex(s, _) => s,
    }
}

/// Write `segments` as a JSON array of strings into `out`.
fn write_segments_array(out: &mut String, segments: &[PathSegment]) {
    out.push('[');
    for (i, s) in segments.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        push_json_str(out, segment_str(s));
    }
    out.push(']');
}

/// Write `{"<name>": ` prefix into `out`, with the name escaped.
#[inline]
fn begin_call(out: &mut String, name: &str) {
    out.push('{');
    push_json_str(out, name);
    out.push_str(": ");
}

/// Serialise an entire compiled tree as a JSONLogic string.
pub(crate) fn node_to_json_string(node: &CompiledNode) -> String {
    let mut out = String::new();
    write_node_json(&mut out, node);
    out
}

/// Append a compiled tree as a JSONLogic string into `out`.
fn write_node_json(out: &mut String, node: &CompiledNode) {
    match node {
        CompiledNode::Value { value, .. } => out.push_str(&value.to_json_string()),
        CompiledNode::Array { nodes, .. } => {
            out.push('[');
            for (i, n) in nodes.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write_node_json(out, n);
            }
            out.push(']');
        }
        CompiledNode::BuiltinOperator { opcode, args, .. } => {
            write_builtin_json(out, opcode, args);
        }
        CompiledNode::CustomOperator(data) => {
            write_custom_json(out, &data.name, &data.args);
        }
        // Memo wrappers are invisible in serialized output — `to_json()`
        // of a CSE'd tree is byte-identical to the unwrapped tree.
        CompiledNode::Cse(data) => write_node_json(out, &data.inner),
        #[cfg(feature = "templating")]
        CompiledNode::StructuredObject(data) => write_structured_json(out, &data.fields),
        CompiledNode::Var {
            scope_level,
            segments,
            default_value,
            ..
        } => write_compiled_var_json(out, *scope_level, segments, default_value.as_deref()),
        #[cfg(feature = "ext-control")]
        CompiledNode::Exists(data) => write_compiled_exists_json(out, &data.segments),
        #[cfg(feature = "error-handling")]
        CompiledNode::Throw(data) => {
            begin_call(out, "throw");
            if let datavalue::OwnedDataValue::Object(pairs) = &data.error
                && let Some((_, datavalue::OwnedDataValue::String(s))) =
                    pairs.iter().find(|(k, _)| k == "type")
            {
                push_json_str(out, s);
            } else {
                out.push_str(&data.error.to_json_string());
            }
            out.push('}');
        }
        CompiledNode::Missing(data) => {
            out.push_str("{\"missing\": [");
            for (i, a) in data.args.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                match a {
                    crate::node::CompiledMissingArg::Now((path, _)) => push_json_str(out, path),
                    crate::node::CompiledMissingArg::Later(n) => write_node_json(out, n),
                }
            }
            out.push_str("]}");
        }
        CompiledNode::MissingSome(data) => {
            out.push_str("{\"missing_some\": [");
            match &data.min_present {
                crate::node::CompiledMissingMin::Now(n) => {
                    let mut buf = itoa::Buffer::new();
                    out.push_str(buf.format(*n));
                }
                crate::node::CompiledMissingMin::Later(n) => write_node_json(out, n),
            }
            out.push_str(", ");
            match &data.paths {
                crate::node::CompiledMissingPaths::Now(paths) => {
                    out.push('[');
                    for (i, (p, _)) in paths.iter().enumerate() {
                        if i > 0 {
                            out.push_str(", ");
                        }
                        push_json_str(out, p);
                    }
                    out.push(']');
                }
                crate::node::CompiledMissingPaths::Later(n) => write_node_json(out, n),
            }
            out.push_str("]}");
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
        CompiledNode::InvalidArgs { op_name, args, .. } => {
            begin_call(out, op_name);
            out.push_str(&args.to_json_string());
            out.push('}');
        }
    }
}

/// Render an operator's argument list as a JSON array, or a single
/// argument in place when `inline` allows and that reads back the same: an
/// argument that renders as an array would be read back as the argument
/// list itself (`{"max": [[1, 2]]}` is not `{"max": [1, 2]}`). Shared by
/// the builtin and custom operator renderers.
fn write_args_json(out: &mut String, args: &[CompiledNode], inline: bool) {
    if let [arg] = args {
        if inline {
            let start = out.len();
            write_node_json(out, arg);
            if out.as_bytes().get(start) == Some(&b'[') {
                out.insert(start, '[');
                out.push(']');
            }
        } else {
            out.push('[');
            write_node_json(out, arg);
            out.push(']');
        }
        return;
    }
    out.push('[');
    for (i, arg) in args.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        write_node_json(out, arg);
    }
    out.push(']');
}

fn write_builtin_json(out: &mut String, opcode: &OpCode, args: &[CompiledNode]) {
    // `and` / `or` / `if` read only an argument array: a lone argument
    // written in place is an error, not that argument.
    let inline = opcode.meta().args_form != crate::operators::meta::ArgsForm::ArrayOnly;
    begin_call(out, opcode.as_str());
    write_args_json(out, args, inline);
    out.push('}');
}

#[cfg(feature = "trace")]
pub(crate) fn builtin_to_json_string(opcode: &OpCode, args: &[CompiledNode]) -> String {
    let mut out = String::new();
    write_builtin_json(&mut out, opcode, args);
    out
}

fn write_custom_json(out: &mut String, name: &str, args: &[CompiledNode]) {
    begin_call(out, name);
    write_args_json(out, args, true);
    out.push('}');
}

#[cfg(feature = "trace")]
pub(crate) fn custom_to_json_string(name: &str, args: &[CompiledNode]) -> String {
    let mut out = String::new();
    write_custom_json(&mut out, name, args);
    out
}

#[cfg(feature = "templating")]
fn write_structured_json(out: &mut String, fields: &[(String, CompiledNode)]) {
    out.push('{');
    for (i, (key, node)) in fields.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        push_json_str(out, key);
        out.push_str(": ");
        write_node_json(out, node);
    }
    out.push('}');
}

#[cfg(all(feature = "templating", feature = "trace"))]
pub(crate) fn structured_to_json_string(fields: &[(String, CompiledNode)]) -> String {
    let mut out = String::new();
    write_structured_json(&mut out, fields);
    out
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
fn write_compiled_var_json(
    out: &mut String,
    scope_level: u32,
    segments: &[PathSegment],
    default_value: Option<&CompiledNode>,
) {
    if scope_level > 0 {
        begin_call(out, "val");
        out.push_str("[[");
        let mut buf = itoa::Buffer::new();
        out.push_str(buf.format(scope_level));
        out.push(']');
        for seg in segments {
            out.push_str(", ");
            push_json_str(out, segment_str(seg));
        }
        out.push_str("]}");
        return;
    }
    // A `var` path is split on `.`, and an empty one reads the whole
    // context, so a dotted segment or a lone empty one needs the `val`
    // form. Only `val` builds those, and `val` takes no default.
    let needs_val = match segments {
        [seg] => segment_str(seg).is_empty() || segment_str(seg).contains('.'),
        _ => segments.iter().any(|s| segment_str(s).contains('.')),
    };
    if needs_val && default_value.is_none() {
        begin_call(out, "val");
        match segments {
            [seg] => push_json_str(out, segment_str(seg)),
            _ => write_segments_array(out, segments),
        }
        out.push('}');
        return;
    }
    let path = segments
        .iter()
        .map(segment_str)
        .collect::<Vec<_>>()
        .join(".");
    begin_call(out, "var");
    match default_value {
        Some(def) => {
            out.push('[');
            push_json_str(out, &path);
            out.push_str(", ");
            write_node_json(out, def);
            out.push(']');
        }
        None => push_json_str(out, &path),
    }
    out.push('}');
}

#[cfg(feature = "ext-control")]
fn write_compiled_exists_json(out: &mut String, segments: &[PathSegment]) {
    begin_call(out, "exists");
    match segments {
        [seg] => push_json_str(out, segment_str(seg)),
        _ => write_segments_array(out, segments),
    }
    out.push('}');
}

#[cfg(all(feature = "ext-control", feature = "trace"))]
pub(crate) fn compiled_exists_to_json_string(segments: &[PathSegment]) -> String {
    let mut out = String::new();
    write_compiled_exists_json(&mut out, segments);
    out
}
