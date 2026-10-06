//! Public path-resolution surface — translates the raw `Vec<u32>` breadcrumb
//! that [`crate::Error`] carries into structured [`PathStep`]s consumers can
//! act on.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::Logic;
use crate::node::CompiledNode;

/// One node along the path from the root of a compiled rule down to the
/// failing sub-expression. Returned root-to-leaf by
/// [`crate::Logic::resolve_node_ids`] / [`crate::Error::resolve_path`].
///
/// `#[non_exhaustive]` so future fields can be added in 5.x without
/// breaking downstream — external code reads fields freely but cannot
/// construct via struct literal. UI tooling that consumes this type
/// over the wire can roundtrip via the derived `Serialize` /
/// `Deserialize`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PathStep {
    /// Compile-time node id, matching [`crate::Error::node_ids`].
    pub node_id: u32,
    /// Operator name at this node, when one applies. `None` for plain values
    /// and arrays.
    pub operator: Option<String>,
    /// Position within the parent node's argument list. `None` for the root
    /// step (no parent) and for non-positional contexts.
    pub arg_index: Option<u32>,
    /// JSONLogic-flavoured pointer from the root to this node — e.g.
    /// `/if/0/>/0` for the `var` slot of the inner `>` inside an `if`.
    /// Empty string for the root step. Tokens are escaped as RFC 6901
    /// escapes them (`/` is `~1`), as in a trace's pointers; for a rule
    /// compiled with `TracedSession::compile` it is
    /// [`Logic::pointer`](crate::Logic::pointer), the pointer into the
    /// rule as written.
    pub json_pointer: String,
}

/// Internal index entry collected during the walk.
struct NodeInfo {
    operator: Option<String>,
    arg_index: Option<u32>,
    json_pointer: String,
}

impl Logic {
    /// Translate a breadcrumb of compiled-node ids into structured
    /// [`PathStep`]s, root-to-leaf.
    ///
    /// Input is the leaf-to-root breadcrumb stored on [`crate::Error::node_ids`].
    /// Walks the compiled tree once to build an id → location index, then
    /// resolves each input id; ids absent from the tree are skipped (defensive
    /// against synthetic nodes from operator fast paths).
    pub fn resolve_node_ids(&self, ids: &[u32]) -> Vec<PathStep> {
        if ids.is_empty() {
            return Vec::new();
        }

        let mut index: HashMap<u32, NodeInfo> = HashMap::new();
        walk(&self.root, None, String::new(), &mut index);

        let mut out = Vec::with_capacity(ids.len());
        // Breadcrumb is leaf-to-root; reverse for natural root-to-leaf reading.
        for &id in ids.iter().rev() {
            if let Some(ni) = index.get(&id) {
                out.push(PathStep {
                    node_id: id,
                    operator: ni.operator.clone(),
                    arg_index: ni.arg_index,
                    json_pointer: self
                        .pointer(id)
                        .map_or_else(|| ni.json_pointer.clone(), str::to_string),
                });
            }
        }
        out
    }
}

/// Depth-first walk of a [`CompiledNode`], recording (operator, arg_index,
/// json_pointer) for every reachable node id. `arg_index` and
/// `json_pointer` describe how *this* node is reached from above.
///
/// For a rule compiled without folding, the pointers match the ones a
/// traced compile records, except where an operator's lone argument was
/// written without its array (`{"!": {"var": "x"}}`): the compiled tree
/// does not keep that, so the pointer names it as item 0.
///
/// Recursion delegates the "what are this node's children" question to
/// [`CompiledNode::visit_indexed_children`] so the variant match lives in
/// exactly one place.
fn walk(
    node: &CompiledNode,
    arg_index: Option<u32>,
    json_pointer: String,
    out: &mut HashMap<u32, NodeInfo>,
) {
    // CSE memo wrappers are path-transparent: delegate before the generic
    // body so the wrapped node's operator/pointer are recorded exactly as
    // in an unwrapped tree (no extra "/op/0" step for the wrapper).
    if let CompiledNode::Cse(data) = node {
        return walk(&data.inner, arg_index, json_pointer, out);
    }

    let id = node.id();
    let operator = node.operator_name().map(|c| c.into_owned());

    // Children of an `Array` form pointers like "/<idx>"; for every other
    // variant the current node's operator name is the pointer prefix.
    let child_parent_op = if matches!(node, CompiledNode::Array { .. }) {
        None
    } else {
        operator.as_deref()
    };

    // Recurse first while borrowing `operator` / `json_pointer`, then move
    // both owned values into the map — node ids are unique, so insertion
    // order does not matter, and this avoids cloning them per node.
    match node {
        // A template field sits under its key, not its position.
        #[cfg(feature = "templating")]
        CompiledNode::StructuredObject(data) => {
            for (i, (key, child)) in data.fields.iter().enumerate() {
                let mut pointer = json_pointer.clone();
                crate::node::push_pointer_token(&mut pointer, key);
                walk(child, Some(i as u32), pointer, out);
            }
        }
        _ => node.visit_indexed_children(&mut |i, child| {
            let pointer = build_pointer(&json_pointer, child_parent_op, i);
            walk(child, Some(i), pointer, out);
        }),
    }

    out.insert(
        id,
        NodeInfo {
            operator,
            arg_index,
            json_pointer,
        },
    );
}

/// The pointer of child `idx` of the node at `parent_pointer`: under the
/// parent's operator key, or directly under the parent for an array.
#[inline]
fn build_pointer(parent_pointer: &str, parent_op: Option<&str>, idx: u32) -> String {
    let mut pointer = parent_pointer.to_string();
    if let Some(op) = parent_op {
        crate::node::push_pointer_token(&mut pointer, op);
    }
    pointer.push('/');
    pointer.push_str(itoa::Buffer::new().format(idx));
    pointer
}

#[cfg(test)]
mod tests {
    fn engine() -> crate::Engine {
        crate::Engine::new()
    }

    #[test]
    fn resolve_root_only() {
        // Use a rule with a `var` that survives static evaluation as the root.
        let compiled = engine().compile(r#"{"==": [{"var": "x"}, 1]}"#).unwrap();
        let root_id = compiled.root.id();
        let steps = compiled.resolve_node_ids(&[root_id]);
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].node_id, root_id);
        assert_eq!(steps[0].operator.as_deref(), Some("=="));
        assert_eq!(steps[0].arg_index, None);
        assert_eq!(steps[0].json_pointer, "");
    }

    #[test]
    fn resolve_empty_path_returns_empty() {
        let compiled = engine().compile(r#"{"==": [{"var": "x"}, 1]}"#).unwrap();
        assert!(compiled.resolve_node_ids(&[]).is_empty());
    }

    #[test]
    fn resolve_unknown_ids_are_skipped() {
        let compiled = engine().compile(r#"{"==": [{"var": "x"}, 1]}"#).unwrap();
        // u32::MAX won't exist in the tree.
        assert!(compiled.resolve_node_ids(&[u32::MAX]).is_empty());
    }

    #[test]
    fn resolve_via_evaluation_error() {
        // {"+": ["x", 1]} — the string-vs-number arithmetic raises NaN.
        let engine = engine();
        let compiled = engine.compile(r#"{"+": ["x", 1]}"#).unwrap();
        let arena = bumpalo::Bump::new();
        let data = datavalue::DataValue::from_str("null", &arena).unwrap();
        let err = engine.evaluate(&compiled, data, &arena).unwrap_err();
        // The merged Error should carry a non-empty path now.
        let steps = err.resolve_path(&compiled);
        assert!(
            !steps.is_empty(),
            "expected resolved path for arithmetic failure, got {:?}",
            err
        );
        // First step (root-to-leaf) is the outermost operator.
        assert_eq!(steps[0].operator.as_deref(), Some("+"));
        assert_eq!(steps[0].json_pointer, "");
    }

    /// Error-path pointers escape tokens as trace pointers do, and a rule
    /// compiled for tracing reports its recorded pointers.
    #[test]
    fn pointers_escape_and_follow_the_traced_compile() {
        let engine = engine();
        let rule = r#"{"/": [{"+": [{"var": "x"}, 1]}, 2]}"#;
        let steps = |compiled: &crate::Logic| {
            let arena = bumpalo::Bump::new();
            let err = engine
                .evaluate(compiled, r#"{"x": "a"}"#, &arena)
                .unwrap_err();
            err.resolve_path(compiled)
        };
        let plain = engine.compile(rule).unwrap();
        let got: Vec<_> = steps(&plain).into_iter().map(|s| s.json_pointer).collect();
        assert_eq!(got, ["", "/~1/0"]);
        #[cfg(feature = "trace")]
        {
            let traced = engine.trace().compile(rule).unwrap();
            let traced_steps = steps(&traced);
            assert_eq!(traced_steps.last().unwrap().json_pointer, "/~1/0");
            for step in &traced_steps {
                assert_eq!(
                    traced.pointer(step.node_id),
                    Some(step.json_pointer.as_str())
                );
            }
        }
    }

    /// Pointers of every node of a rule compiled without folding.
    fn pointers(rule: &str, templating: bool) -> Vec<(Option<u32>, String)> {
        let engine = crate::Engine::builder()
            .with_constant_folding(false)
            .with_templating(templating)
            .build();
        let compiled = engine.compile(rule).unwrap();
        let mut index = std::collections::HashMap::new();
        super::walk(&compiled.root, None, String::new(), &mut index);
        let mut out: Vec<_> = index
            .into_iter()
            .map(|(_, n)| (n.arg_index, n.json_pointer))
            .collect();
        out.sort_by(|a, b| a.1.cmp(&b.1));
        out
    }

    /// A `var` default is item 1 of `{"var": [path, default]}`.
    #[test]
    fn var_default_is_item_one() {
        let got = pointers(r#"{"var": ["x", {"+": [1, {"var": "y"}]}]}"#, false);
        let ptrs: Vec<_> = got.iter().map(|(i, p)| (*i, p.as_str())).collect();
        assert_eq!(
            ptrs,
            [
                (None, ""),
                (Some(1), "/var/1"),
                (Some(0), "/var/1/+/0"),
                (Some(1), "/var/1/+/1"),
            ]
        );
    }

    /// A template field sits under its key.
    #[cfg(feature = "templating")]
    #[test]
    fn template_fields_are_keyed() {
        let got = pointers(r#"{"a": 1, "b/c": {"var": "x"}}"#, true);
        let ptrs: Vec<_> = got.iter().map(|(i, p)| (*i, p.as_str())).collect();
        assert_eq!(ptrs, [(None, ""), (Some(0), "/a"), (Some(1), "/b~1c")]);
    }
}
