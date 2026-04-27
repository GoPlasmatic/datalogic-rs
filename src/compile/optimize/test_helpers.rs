//! Shared test fixtures for optimizer-pass unit tests.

use crate::node::{CompiledNode, MetadataHint, PathSegment, ReduceHint, SYNTHETIC_ID};
use crate::opcode::OpCode;
use datavalue::OwnedDataValue;

pub(super) fn val(v: OwnedDataValue) -> CompiledNode {
    CompiledNode::synthetic_value(v)
}

pub(super) fn var_node(name: &str) -> CompiledNode {
    CompiledNode::CompiledVar {
        id: SYNTHETIC_ID,
        scope_level: 0,
        segments: vec![PathSegment::Field(name.into())].into_boxed_slice(),
        reduce_hint: ReduceHint::None,
        metadata_hint: MetadataHint::None,
        default_value: None,
    }
}

pub(super) fn builtin(opcode: OpCode, args: Vec<CompiledNode>) -> CompiledNode {
    CompiledNode::BuiltinOperator {
        id: SYNTHETIC_ID,
        opcode,
        args: args.into_boxed_slice(),
        predicate_hint: None,
        iter_arg_kind: crate::operators::array::IterArgKind::General,
    }
}
