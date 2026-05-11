//! Post-compile pass that caches per-operator analysis results onto every
//! `BuiltinOperator` node, plus the small helper that pre-builds inline
//! literals at construction time.

use super::CompiledNode;
use crate::arena::DataValue;
use crate::opcode::OpCode;
use datavalue::OwnedDataValue;

/// Pre-build a `DataValue<'static>` for every literal whose payload either
/// fits inline (Null, Bool, Number) or borrows static slices (empty string,
/// empty array, empty object). Non-empty Strings/Arrays/Objects can't be
/// pre-built without either external self-cell crates or transmute-based
/// self-reference, so they fall through to `literal_fallback` at dispatch
/// time and pay one bumpalo alloc (string) or a deep-convert pass
/// (non-empty array/object) per evaluation.
#[inline]
pub(super) fn precompute_lit(value: &OwnedDataValue) -> Option<Box<DataValue<'static>>> {
    match value {
        OwnedDataValue::Null => Some(Box::new(DataValue::Null)),
        OwnedDataValue::Bool(b) => Some(Box::new(DataValue::Bool(*b))),
        OwnedDataValue::Number(n) => Some(Box::new(DataValue::Number(*n))),
        OwnedDataValue::String(s) if s.is_empty() => Some(Box::new(DataValue::String(""))),
        OwnedDataValue::Array(a) if a.is_empty() => Some(Box::new(DataValue::Array(&[]))),
        OwnedDataValue::Object(o) if o.is_empty() => Some(Box::new(DataValue::Object(&[]))),
        _ => None,
    }
}

/// Opcodes that consume `args[0]` as an iterator input via
/// [`crate::operators::array::resolve_iter_input`]. Used by the post-compile
/// populate pass to decide whether `iter_arg_kind` should be classified or
/// left at the `General` default. Mirrors the actual call sites — Merge does
/// not currently route through `resolve_iter_input`.
#[inline]
fn iterates_args0(opcode: OpCode) -> bool {
    let _opcode = opcode;
    #[cfg(feature = "ext-array")]
    if matches!(opcode, OpCode::Sort) {
        return true;
    }
    matches!(
        opcode,
        OpCode::Filter
            | OpCode::Map
            | OpCode::All
            | OpCode::Some
            | OpCode::None
            | OpCode::Reduce
            | OpCode::Min
            | OpCode::Max
    )
}

/// Walk the compiled tree and cache per-operator analysis results
/// (`predicate_hint`, `iter_arg_kind`) onto every `BuiltinOperator` node.
/// Pure compile-time bookkeeping — no arena, no unsafe.
///
/// Non-trivial literals (non-empty Strings/Arrays/Objects) are NOT
/// pre-allocated; they fall through to `literal_fallback` at dispatch time.
/// Trivial literals (Null/Bool/Number/empty primitives) are handled by
/// [`precompute_lit`] at node construction.
pub(crate) fn populate_lits(node: &mut CompiledNode) {
    node.visit_children_mut(&mut populate_lits);

    if let CompiledNode::BuiltinOperator {
        opcode,
        args,
        predicate_hint,
        iter_arg_kind,
        ..
    } = node
    {
        // Cache the fast-predicate detection result so quantifier/filter
        // operators consult `predicate_hint` instead of re-running the
        // structural detection on every iteration. Re-derive on every
        // call (rather than guarding with `is_none`) so a clone of an
        // already-populated tree gets a fresh hint matching the cloned
        // args — `Box<[PathSegment]>` and `OwnedDataValue` move on clone,
        // and the cached hint borrows nothing from them anyway.
        *predicate_hint =
            crate::operators::array::FastPredicate::try_detect_owned(*opcode, args).map(Box::new);
        // Cache the iterator-input classification for ops that consume
        // `args[0]` as an iterable. Read by `resolve_iter_input` so the
        // runtime shape match collapses to a byte compare. Other opcodes
        // keep the default `General` (the populate pass overwrites on
        // every clone).
        *iter_arg_kind = if iterates_args0(*opcode) && !args.is_empty() {
            crate::operators::array::IterArgKind::classify(&args[0])
        } else {
            crate::operators::array::IterArgKind::General
        };
    }
}
