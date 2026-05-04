//! Public path-resolution surface — translates the raw `Vec<u32>` breadcrumb
//! that [`crate::Error`] carries into structured [`PathStep`]s consumers can
//! act on.

use std::collections::HashMap;

use serde::Serialize;

use crate::CompiledLogic;
use crate::node::CompiledNode;

/// One node along the path from the root of a compiled rule down to the
/// failing sub-expression. Returned root-to-leaf by
/// [`crate::CompiledLogic::resolve_path`] / [`crate::Error::resolved_path`].
#[non_exhaustive]
#[derive(Debug, Clone, Serialize)]
pub struct PathStep {
    /// Compile-time node id, matching [`crate::Error::path`].
    pub node_id: u32,
    /// Operator name at this node, when one applies. `None` for plain values
    /// and arrays.
    pub operator: Option<String>,
    /// Position within the parent node's argument list. `None` for the root
    /// step (no parent) and for non-positional contexts.
    pub arg_index: Option<u32>,
    /// JSONLogic-flavoured pointer from the root to this node — e.g.
    /// `/if/0/>/0` for the `var` slot of the inner `>` inside an `if`.
    /// Empty string for the root step.
    pub json_pointer: String,
}

/// Internal index entry collected during the walk.
struct NodeInfo {
    operator: Option<String>,
    arg_index: Option<u32>,
    json_pointer: String,
}

impl CompiledLogic {
    /// Translate a breadcrumb of [`crate::CompiledNode`] ids into structured
    /// [`PathStep`]s, root-to-leaf.
    ///
    /// Input is the leaf-to-root breadcrumb stored on [`crate::Error::path`].
    /// Walks the compiled tree once to build an id → location index, then
    /// resolves each input id; ids absent from the tree are skipped (defensive
    /// against synthetic nodes from operator fast paths).
    pub fn resolve_path(&self, ids: &[u32]) -> Vec<PathStep> {
        if ids.is_empty() {
            return Vec::new();
        }

        let mut index: HashMap<u32, NodeInfo> = HashMap::new();
        walk(&self.root, None, None, "", &mut index);

        let mut out = Vec::with_capacity(ids.len());
        // Breadcrumb is leaf-to-root; reverse for natural root-to-leaf reading.
        for &id in ids.iter().rev() {
            if let Some(ni) = index.get(&id) {
                out.push(PathStep {
                    node_id: id,
                    operator: ni.operator.clone(),
                    arg_index: ni.arg_index,
                    json_pointer: ni.json_pointer.clone(),
                });
            }
        }
        out
    }
}

/// Depth-first walk of a [`CompiledNode`], recording (operator, arg_index,
/// json_pointer) for every reachable node id. `parent_op` and
/// `parent_pointer` describe how *this* node is reached from above.
fn walk(
    node: &CompiledNode,
    parent_op: Option<&str>,
    arg_index: Option<u32>,
    parent_pointer: &str,
    out: &mut HashMap<u32, NodeInfo>,
) {
    let id = node.id();
    let operator = node.operator_name();
    let json_pointer = build_pointer(parent_pointer, parent_op, arg_index);
    out.insert(
        id,
        NodeInfo {
            operator: operator.clone(),
            arg_index,
            json_pointer: json_pointer.clone(),
        },
    );

    // The "parent operator" passed to children is the *current* node's
    // operator name — children's pointers are formed against it.
    let child_parent_op = operator.as_deref();

    match node {
        CompiledNode::Value { .. } => {
            // Leaf — nothing to recurse into.
        }
        CompiledNode::Array { nodes, .. } => {
            for (i, child) in nodes.iter().enumerate() {
                walk(child, None, Some(i as u32), &json_pointer, out);
            }
        }
        CompiledNode::BuiltinOperator { args, .. } => {
            for (i, child) in args.iter().enumerate() {
                walk(child, child_parent_op, Some(i as u32), &json_pointer, out);
            }
        }
        CompiledNode::CustomOperator(data) => {
            for (i, child) in data.args.iter().enumerate() {
                walk(child, child_parent_op, Some(i as u32), &json_pointer, out);
            }
        }
        #[cfg(feature = "preserve")]
        CompiledNode::StructuredObject(data) => {
            for (i, (_key, child)) in data.fields.iter().enumerate() {
                walk(child, child_parent_op, Some(i as u32), &json_pointer, out);
            }
        }
        CompiledNode::CompiledVar { default_value, .. } => {
            if let Some(def) = default_value {
                walk(def, child_parent_op, Some(0), &json_pointer, out);
            }
        }
        #[cfg(feature = "ext-control")]
        CompiledNode::CompiledExists(_) => {
            // Leaf — exists arguments are pre-parsed segments.
        }
        #[cfg(feature = "error-handling")]
        CompiledNode::CompiledThrow(_) => {
            // Leaf — error payload is a baked-in OwnedDataValue.
        }
        CompiledNode::CompiledMissing(data) => {
            for (i, arg) in data.args.iter().enumerate() {
                if let crate::node::CompiledMissingArg::Dynamic(child) = arg {
                    walk(child, child_parent_op, Some(i as u32), &json_pointer, out);
                }
            }
        }
        CompiledNode::CompiledMissingSome(data) => {
            if let crate::node::CompiledMissingMin::Dynamic(child) = &data.min_present {
                walk(child, child_parent_op, Some(0), &json_pointer, out);
            }
            if let crate::node::CompiledMissingPaths::Dynamic(child) = &data.paths {
                walk(child, child_parent_op, Some(1), &json_pointer, out);
            }
        }
    }
}

#[inline]
fn build_pointer(parent_pointer: &str, parent_op: Option<&str>, arg_index: Option<u32>) -> String {
    match (parent_op, arg_index) {
        (Some(op), Some(idx)) => format!("{}/{}/{}", parent_pointer, op, idx),
        // Child of an Array (no operator key) — JSON pointer "/idx".
        (None, Some(idx)) => format!("{}/{}", parent_pointer, idx),
        _ => parent_pointer.to_string(),
    }
}

#[cfg(test)]
mod tests {
    fn engine() -> crate::DataLogic {
        crate::DataLogic::new()
    }

    #[test]
    fn resolve_root_only() {
        // Use a rule with a `var` that survives static evaluation as the root.
        let compiled = engine().compile(r#"{"==": [{"var": "x"}, 1]}"#).unwrap();
        let root_id = compiled.root.id();
        let steps = compiled.resolve_path(&[root_id]);
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].node_id, root_id);
        assert_eq!(steps[0].operator.as_deref(), Some("=="));
        assert_eq!(steps[0].arg_index, None);
        assert_eq!(steps[0].json_pointer, "");
    }

    #[test]
    fn resolve_empty_path_returns_empty() {
        let compiled = engine().compile(r#"{"==": [{"var": "x"}, 1]}"#).unwrap();
        assert!(compiled.resolve_path(&[]).is_empty());
    }

    #[test]
    fn resolve_unknown_ids_are_skipped() {
        let compiled = engine().compile(r#"{"==": [{"var": "x"}, 1]}"#).unwrap();
        // u32::MAX won't exist in the tree.
        assert!(compiled.resolve_path(&[u32::MAX]).is_empty());
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
        let steps = err.resolved_path(&compiled);
        assert!(
            !steps.is_empty(),
            "expected resolved path for arithmetic failure, got {:?}",
            err
        );
        // First step (root-to-leaf) is the outermost operator.
        assert_eq!(steps[0].operator.as_deref(), Some("+"));
        assert_eq!(steps[0].json_pointer, "");
    }
}
