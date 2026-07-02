//! Post-compile pass that caches per-operator analysis results onto every
//! `BuiltinOperator` node and pre-builds literals onto every `Value` node,
//! plus the small helper that pre-builds trivial literals at construction
//! time.

use super::CompiledNode;
use super::prelit::PreLit;
use crate::arena::DataValue;
use crate::opcode::OpCode;
use datavalue::OwnedDataValue;

/// Pre-build a [`PreLit`] for every literal whose payload either fits
/// inline (Null, Bool, Number, and the datetime scalars) or borrows static
/// slices (empty string, empty array, empty object). Called from every
/// `CompiledNode::Value` construction site — including the runtime
/// `synthetic_value` wrappers — so it stays cheap: one small box, no
/// clones. Non-empty Strings/Arrays/Objects are pre-built separately by
/// [`populate_lits`] at `Logic::new` (compile time only); a synthetic
/// composite wrapper keeps `lit = None` and falls through to
/// `literal_fallback` at dispatch time.
#[inline]
pub(super) fn precompute_lit(value: &OwnedDataValue) -> Option<PreLit> {
    let dv = match value {
        OwnedDataValue::Null => DataValue::Null,
        OwnedDataValue::Bool(b) => DataValue::Bool(*b),
        OwnedDataValue::Number(n) => DataValue::Number(*n),
        OwnedDataValue::String(s) if s.is_empty() => DataValue::String(""),
        OwnedDataValue::Array(a) if a.is_empty() => DataValue::Array(&[]),
        OwnedDataValue::Object(o) if o.is_empty() => DataValue::Object(&[]),
        #[cfg(feature = "datetime")]
        OwnedDataValue::DateTime(d) => DataValue::DateTime(*d),
        #[cfg(feature = "datetime")]
        OwnedDataValue::Duration(d) => DataValue::Duration(*d),
        _ => return None,
    };
    Some(PreLit::from_static(dv))
}

/// Opcodes that consume `args[0]` as an iterator input via
/// [`crate::operators::array::resolve_iter_input`]. Used by the post-compile
/// populate pass to decide whether `iter_arg_kind` should be classified or
/// left at the `General` default. Mirrors the actual call sites — Merge does
/// not currently route through `resolve_iter_input`.
#[inline]
fn iterates_args0(opcode: OpCode) -> bool {
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
/// (`predicate_hint`, `iter_arg_kind`) onto every `BuiltinOperator` node,
/// plus a pre-built [`PreLit`] onto every `Value` node holding a
/// non-trivial literal. Pure compile-time bookkeeping — no arena, no
/// unsafe.
///
/// Trivial literals (Null/Bool/Number/empty primitives) are handled by
/// [`precompute_lit`] at node construction; this pass covers the non-empty
/// Strings/Arrays/Objects, paying one clone + spine build per literal per
/// compile so dispatch returns a borrow instead of re-converting the
/// literal into the arena on every evaluation. Guarded by `is_none`
/// because a clone of an already-populated tree carries a correct
/// rebuilt `PreLit` (see `PreLit::clone`) and `value` never mutates after
/// construction.
pub(crate) fn populate_lits(node: &mut CompiledNode) {
    node.visit_children_mut(&mut populate_lits);

    if let CompiledNode::Value { value, lit, .. } = node {
        if lit.is_none() {
            *lit = PreLit::composite(value);
        }
        return;
    }

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
