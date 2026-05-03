//! Arena-mode dispatch hub.
//!
//! [`evaluate_node_inner`] is the exhaustive `CompiledNode` match that
//! routes each node shape to its operator implementation. It is invoked from
//! `DataLogic::evaluate_node`, which handles the literal fast path,
//! breadcrumb accumulation, and trace recording before delegating here.

use super::DataLogic;
use crate::arena::DataContextStack;
use crate::{CompiledNode, Error, Result};

/// Inner dispatch — never called directly; reachable only via
/// `DataLogic::evaluate_node` which handles the literal fast path,
/// breadcrumb accumulation, and trace recording.
///
/// `#[inline(always)]` is load-bearing here: this function is the hot
/// dispatch and the compiler inlines it into `evaluate_node` in the
/// single-file layout. Crossing the module boundary loses that inline
/// decision (measured ~1 ns regression on the 15 ns baseline).
#[inline(always)]
pub(super) fn evaluate_node_inner<'a>(
    engine: &DataLogic,
    node: &'a CompiledNode,
    actx: &mut DataContextStack<'a>,
    arena: &'a bumpalo::Bump,
) -> Result<&'a crate::arena::DataValue<'a>> {
    match node {
        // Compiled var: full dispatch via the arena helper. Root and
        // frame data are both arena-resident `DataValue`s, so lookups
        // are zero-copy borrows.
        CompiledNode::CompiledVar {
            scope_level,
            segments,
            reduce_hint,
            metadata_hint,
            default_value,
            ..
        } => crate::operators::variable::evaluate_compiled_var_arena(
            crate::operators::variable::CompiledVarSpec {
                scope_level: *scope_level,
                segments,
                reduce_hint: *reduce_hint,
                metadata_hint: *metadata_hint,
                default_value: default_value.as_deref(),
            },
            actx,
            engine,
            arena,
        ),

        // Compiled exists: full dispatch — root scope walks the input
        // directly, others walk arena frame data. Result is always a
        // Bool singleton.
        #[cfg(feature = "ext-control")]
        CompiledNode::CompiledExists(data) => {
            crate::operators::variable::evaluate_compiled_exists_arena(
                data.scope_level,
                &data.segments,
                actx,
                arena,
            )
        }

        // Value literal: handled by the outer `evaluate_node`
        // wrapper before reaching this match.
        CompiledNode::Value { .. } => unreachable!("literal handled by wrapper"),

        // Raw val/exists operator forms (rare — most are precompiled
        // to CompiledVar/CompiledExists, but dynamic-path forms remain
        // as BuiltinOperator). `var` and `val` both arrive here as
        // OpCode::Val — see `OpCode::FromStr` for the normalization.
        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::Val,
            args,
            ..
        } => crate::operators::variable::evaluate_val_arena(args, actx, engine, arena),
        #[cfg(feature = "ext-control")]
        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::Exists,
            args,
            ..
        } => crate::operators::variable::evaluate_exists_arena(args, actx, engine, arena),

        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::Filter,
            args,
            iter_arg_kind,
            ..
        } => crate::operators::array::evaluate_filter_arena(
            args,
            *iter_arg_kind,
            actx,
            engine,
            arena,
        ),

        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::Map,
            args,
            iter_arg_kind,
            ..
        } => crate::operators::array::evaluate_map_arena(args, *iter_arg_kind, actx, engine, arena),

        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::All,
            args,
            iter_arg_kind,
            ..
        } => crate::operators::array::evaluate_all_arena(args, *iter_arg_kind, actx, engine, arena),

        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::Some,
            args,
            iter_arg_kind,
            ..
        } => {
            crate::operators::array::evaluate_some_arena(args, *iter_arg_kind, actx, engine, arena)
        }

        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::None,
            args,
            iter_arg_kind,
            ..
        } => {
            crate::operators::array::evaluate_none_arena(args, *iter_arg_kind, actx, engine, arena)
        }

        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::Reduce,
            args,
            iter_arg_kind,
            ..
        } => crate::operators::array::evaluate_reduce_arena(
            args,
            *iter_arg_kind,
            actx,
            engine,
            arena,
        ),

        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::Merge,
            args,
            ..
        } => crate::operators::array::evaluate_merge_arena(args, actx, engine, arena),

        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::Missing,
            args,
            ..
        } => crate::operators::missing::evaluate_missing_arena(args, actx, engine, arena),
        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::MissingSome,
            args,
            ..
        } => crate::operators::missing::evaluate_missing_some_arena(args, actx, engine, arena),
        CompiledNode::CompiledMissing(data) => {
            crate::operators::missing::evaluate_compiled_missing_arena(data, actx, engine, arena)
        }
        CompiledNode::CompiledMissingSome(data) => {
            crate::operators::missing::evaluate_compiled_missing_some_arena(
                data, actx, engine, arena,
            )
        }

        #[cfg(feature = "ext-string")]
        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::Length,
            args,
            ..
        } => crate::operators::array::evaluate_length_arena(args, actx, engine, arena),

        #[cfg(feature = "ext-array")]
        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::Sort,
            args,
            iter_arg_kind,
            ..
        } => {
            crate::operators::array::evaluate_sort_arena(args, *iter_arg_kind, actx, engine, arena)
        }

        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::Max,
            args,
            iter_arg_kind,
            ..
        } => crate::operators::arithmetic::evaluate_max_arena(
            args,
            *iter_arg_kind,
            actx,
            engine,
            arena,
        ),
        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::Min,
            args,
            iter_arg_kind,
            ..
        } => crate::operators::arithmetic::evaluate_min_arena(
            args,
            *iter_arg_kind,
            actx,
            engine,
            arena,
        ),
        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::Add,
            args,
            ..
        } => crate::operators::arithmetic::evaluate_add_arena(args, actx, engine, arena),
        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::Multiply,
            args,
            ..
        } => crate::operators::arithmetic::evaluate_multiply_arena(args, actx, engine, arena),

        // Comparison
        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::Equals,
            args,
            ..
        } => crate::operators::comparison::evaluate_equals_arena(args, actx, engine, arena),
        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::StrictEquals,
            args,
            ..
        } => crate::operators::comparison::evaluate_strict_equals_arena(args, actx, engine, arena),
        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::NotEquals,
            args,
            ..
        } => crate::operators::comparison::evaluate_not_equals_arena(args, actx, engine, arena),
        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::StrictNotEquals,
            args,
            ..
        } => crate::operators::comparison::evaluate_strict_not_equals_arena(
            args, actx, engine, arena,
        ),
        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::GreaterThan,
            args,
            ..
        } => crate::operators::comparison::evaluate_greater_than_arena(args, actx, engine, arena),
        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::GreaterThanEqual,
            args,
            ..
        } => crate::operators::comparison::evaluate_greater_than_equal_arena(
            args, actx, engine, arena,
        ),
        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::LessThan,
            args,
            ..
        } => crate::operators::comparison::evaluate_less_than_arena(args, actx, engine, arena),
        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::LessThanEqual,
            args,
            ..
        } => {
            crate::operators::comparison::evaluate_less_than_equal_arena(args, actx, engine, arena)
        }

        // Logical
        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::Not,
            args,
            ..
        } => crate::operators::logical::evaluate_not_arena(args, actx, engine, arena),
        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::DoubleNot,
            args,
            ..
        } => crate::operators::logical::evaluate_double_not_arena(args, actx, engine, arena),
        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::And,
            args,
            ..
        } => crate::operators::logical::evaluate_and_arena(args, actx, engine, arena),
        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::Or,
            args,
            ..
        } => crate::operators::logical::evaluate_or_arena(args, actx, engine, arena),

        // Control. `if` and `?:` both arrive here as OpCode::If — see
        // `OpCode::FromStr`. `evaluate_if_arena` handles the 3-arg case
        // identically to a ternary.
        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::If,
            args,
            ..
        } => crate::operators::control::evaluate_if_arena(args, actx, engine, arena),
        #[cfg(feature = "ext-control")]
        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::Coalesce,
            args,
            ..
        } => crate::operators::control::evaluate_coalesce_arena(args, actx, engine, arena),
        #[cfg(feature = "ext-control")]
        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::Switch,
            args,
            ..
        } => crate::operators::control::evaluate_switch_arena(args, actx, engine, arena),

        // Arithmetic binary forms
        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::Subtract,
            args,
            ..
        } => crate::operators::arithmetic::evaluate_subtract_arena(args, actx, engine, arena),
        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::Divide,
            args,
            ..
        } => crate::operators::arithmetic::arena_div_or_mod(
            args,
            actx,
            engine,
            arena,
            crate::operators::arithmetic::DivOp::Divide,
        ),
        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::Modulo,
            args,
            ..
        } => crate::operators::arithmetic::arena_div_or_mod(
            args,
            actx,
            engine,
            arena,
            crate::operators::arithmetic::DivOp::Modulo,
        ),

        // Math (unary)
        #[cfg(feature = "ext-math")]
        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::Abs,
            args,
            ..
        } => crate::operators::arithmetic::arena_unary_math(
            args,
            actx,
            engine,
            arena,
            crate::operators::arithmetic::UnaryMathOp::Abs,
        ),
        #[cfg(feature = "ext-math")]
        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::Ceil,
            args,
            ..
        } => crate::operators::arithmetic::arena_unary_math(
            args,
            actx,
            engine,
            arena,
            crate::operators::arithmetic::UnaryMathOp::Ceil,
        ),
        #[cfg(feature = "ext-math")]
        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::Floor,
            args,
            ..
        } => crate::operators::arithmetic::arena_unary_math(
            args,
            actx,
            engine,
            arena,
            crate::operators::arithmetic::UnaryMathOp::Floor,
        ),

        // String
        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::Cat,
            args,
            ..
        } => crate::operators::string::evaluate_cat_arena(args, actx, engine, arena),
        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::Substr,
            args,
            ..
        } => crate::operators::string::evaluate_substr_arena(args, actx, engine, arena),
        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::In,
            args,
            ..
        } => crate::operators::string::evaluate_in_arena(args, actx, engine, arena),
        #[cfg(feature = "ext-string")]
        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::StartsWith,
            args,
            ..
        } => crate::operators::string::evaluate_starts_with_arena(args, actx, engine, arena),
        #[cfg(feature = "ext-string")]
        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::EndsWith,
            args,
            ..
        } => crate::operators::string::evaluate_ends_with_arena(args, actx, engine, arena),
        #[cfg(feature = "ext-string")]
        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::Upper,
            args,
            ..
        } => crate::operators::string::evaluate_upper_arena(args, actx, engine, arena),
        #[cfg(feature = "ext-string")]
        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::Lower,
            args,
            ..
        } => crate::operators::string::evaluate_lower_arena(args, actx, engine, arena),
        #[cfg(feature = "ext-string")]
        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::Trim,
            args,
            ..
        } => crate::operators::string::evaluate_trim_arena(args, actx, engine, arena),
        #[cfg(feature = "ext-string")]
        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::Split,
            args,
            ..
        } => crate::operators::string::evaluate_split_arena(args, actx, engine, arena),

        // DateTime
        #[cfg(feature = "datetime")]
        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::Datetime,
            args,
            ..
        } => crate::operators::datetime::evaluate_datetime_arena(args, actx, engine, arena),
        #[cfg(feature = "datetime")]
        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::Timestamp,
            args,
            ..
        } => crate::operators::datetime::evaluate_timestamp_arena(args, actx, engine, arena),
        #[cfg(feature = "datetime")]
        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::ParseDate,
            args,
            ..
        } => crate::operators::datetime::evaluate_parse_date_arena(args, actx, engine, arena),
        #[cfg(feature = "datetime")]
        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::FormatDate,
            args,
            ..
        } => crate::operators::datetime::evaluate_format_date_arena(args, actx, engine, arena),
        #[cfg(feature = "datetime")]
        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::DateDiff,
            args,
            ..
        } => crate::operators::datetime::evaluate_date_diff_arena(args, actx, engine, arena),
        #[cfg(feature = "datetime")]
        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::Now,
            args,
            ..
        } => crate::operators::datetime::evaluate_now_arena(args, actx, engine, arena),

        // Type
        #[cfg(feature = "ext-control")]
        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::Type,
            args,
            ..
        } => crate::operators::type_op::evaluate_type_arena(args, actx, engine, arena),

        // Throw / Try
        #[cfg(feature = "error-handling")]
        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::Throw,
            args,
            ..
        } => crate::operators::throw::evaluate_throw_arena(args, actx, engine, arena),
        #[cfg(feature = "error-handling")]
        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::Try,
            args,
            ..
        } => crate::operators::try_op::evaluate_try_arena(args, actx, engine, arena),

        // Preserve
        #[cfg(feature = "preserve")]
        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::Preserve,
            args,
            ..
        } => crate::operators::preserve::evaluate_preserve_arena(args, actx, engine, arena),

        // Slice
        #[cfg(feature = "ext-array")]
        CompiledNode::BuiltinOperator {
            opcode: crate::OpCode::Slice,
            args,
            ..
        } => crate::operators::array::evaluate_slice_arena(args, actx, engine, arena),

        // CompiledThrow — constant-folded error literal.
        #[cfg(feature = "error-handling")]
        CompiledNode::CompiledThrow(data) => Err(Error::Thrown(data.error.clone())),

        // StructuredObject (preserve mode): out-of-line — bumpalo::Vec
        // construction would otherwise force a large stack frame on every
        // dispatch arm via worst-case spill sizing.
        #[cfg(feature = "preserve")]
        CompiledNode::StructuredObject(data) => {
            evaluate_structured_object_arena(data, actx, engine, arena)
        }

        // Array literal: out-of-line for the same reason as StructuredObject.
        CompiledNode::Array { nodes, .. } => {
            evaluate_array_literal_arena(nodes, actx, engine, arena)
        }

        // Custom operator: out-of-line — HashMap lookup + bumpalo::Vec
        // for args; rare on the hot path.
        CompiledNode::CustomOperator(data) => {
            evaluate_custom_operator_arena(data, actx, engine, arena)
        }

        // No fallback — every CompiledNode shape is covered by an
        // explicit arm above. Reaching this branch is a compile-error
        // (missing match arm) for newly-added shapes, not a runtime
        // panic. If a future variant lands and you see this, add the
        // dispatch arm.
        #[allow(unreachable_patterns)]
        _ => Err(Error::InvalidArguments(
            "internal: unhandled CompiledNode shape in arena dispatch".into(),
        )),
    }
}

// Heavy arms below are kept out-of-line so the dispatch fn's stack frame
// is sized for the small/common arms only. Each builds a `bumpalo::Vec`
// (multi-word locals + drop glue) which, when inlined, forced the
// dispatch prologue to reserve ~464 B of stack on every recursive call.
// `#[inline(never)]` is load-bearing — see the comment on
// `evaluate_node_inner`.

#[cfg(feature = "preserve")]
#[inline(never)]
fn evaluate_structured_object_arena<'a>(
    data: &'a crate::node::StructuredObjectData,
    actx: &mut crate::arena::DataContextStack<'a>,
    engine: &super::DataLogic,
    arena: &'a bumpalo::Bump,
) -> crate::Result<&'a crate::arena::DataValue<'a>> {
    use crate::arena::DataValue;
    if data.fields.is_empty() {
        return Ok(crate::arena::pool::singleton_empty_object());
    }
    let mut pairs: bumpalo::collections::Vec<'a, (&'a str, DataValue<'a>)> =
        bumpalo::collections::Vec::with_capacity_in(data.fields.len(), arena);
    for (key, n) in data.fields.iter() {
        let val_av = engine.evaluate_node(n, actx, arena)?;
        let val_owned = crate::arena::value::reborrow_arena_value(val_av);
        let k_arena: &'a str = arena.alloc_str(key);
        pairs.push((k_arena, val_owned));
    }
    Ok(arena.alloc(DataValue::Object(pairs.into_bump_slice())))
}

#[inline(never)]
fn evaluate_array_literal_arena<'a>(
    nodes: &'a [crate::CompiledNode],
    actx: &mut crate::arena::DataContextStack<'a>,
    engine: &super::DataLogic,
    arena: &'a bumpalo::Bump,
) -> crate::Result<&'a crate::arena::DataValue<'a>> {
    use crate::arena::DataValue;
    if nodes.is_empty() {
        return Ok(crate::arena::pool::singleton_empty_array());
    }
    let mut items: bumpalo::collections::Vec<'a, DataValue<'a>> =
        bumpalo::collections::Vec::with_capacity_in(nodes.len(), arena);
    for n in nodes.iter() {
        let av = engine.evaluate_node(n, actx, arena)?;
        items.push(crate::arena::value::reborrow_arena_value(av));
    }
    Ok(arena.alloc(DataValue::Array(items.into_bump_slice())))
}

#[inline(never)]
fn evaluate_custom_operator_arena<'a>(
    data: &'a crate::node::CustomOperatorData,
    actx: &mut crate::arena::DataContextStack<'a>,
    engine: &super::DataLogic,
    arena: &'a bumpalo::Bump,
) -> crate::Result<&'a crate::arena::DataValue<'a>> {
    use crate::arena::DataValue;
    let arena_op = engine
        .custom_arena_operators
        .get(&data.name)
        .ok_or_else(|| Error::InvalidOperator(data.name.clone()))?;
    let mut arena_args: bumpalo::collections::Vec<'a, &'a DataValue<'a>> =
        bumpalo::collections::Vec::with_capacity_in(data.args.len(), arena);
    for arg in data.args.iter() {
        arena_args.push(engine.evaluate_node(arg, actx, arena)?);
    }
    arena_op.evaluate(&arena_args, actx, arena)
}
