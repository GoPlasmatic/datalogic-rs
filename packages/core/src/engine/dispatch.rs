//! Arena-mode dispatch hub.
//!
//! [`dispatch_node_inner`] is the exhaustive `CompiledNode` match that
//! routes each node shape to its operator implementation. It is invoked from
//! `Engine::dispatch_node`, which handles the literal fast path,
//! breadcrumb accumulation, and trace recording before delegating here.

use super::Engine;
use crate::arena::ContextStack;
use crate::{CompiledNode, Error, Result};

/// Build the dispatch match. Splits the ~50 `BuiltinOperator` arms into
/// three regular shapes plus a tail of irregular arms so the bulk of the
/// dispatch is one tabular invocation:
///
/// - `simple    [ Op => fn, ... ]` — `fn(args, ctx, engine, arena)`.
/// - `iter      [ Op => fn, ... ]` — `fn(args, *iter_arg_kind, ctx, engine, arena)`,
///   for ops that consume the cached iterator-input classification.
/// - `with_kind [ Op => fn(kind_expr), ... ]` — `fn(args, ctx, engine, arena, kind_expr)`,
///   for ops that share an impl behind a discriminator (Divide/Modulo,
///   Abs/Ceil/Floor).
/// - `other  { pat => expr, ... }` — verbatim arms (compiled `Var` /
///   `Exists` / `Missing` / structured-object, etc.); pasted in front of
///   the generated arms.
///
/// Per-arm `#[cfg(...)]` attributes attach to each `Op => fn` line.
/// Arm ordering doesn't affect codegen — the heavy `bumpalo::Vec`-building
/// cases live in `#[inline(never)]` helpers, so the dispatch's stack frame
/// is sized for the small/common arms regardless.
macro_rules! dispatch {
    (
        node: $node:expr,
        ctx: $ctx:expr,
        engine: $engine:expr,
        arena: $arena:expr,
        other: { $($others:tt)* },
        simple: [ $( $(#[$scfg:meta])* $sop:ident => $sfn:path ),* $(,)? ],
        iter: [ $( $(#[$icfg:meta])* $iop:ident => $ifn:path ),* $(,)? ],
        with_kind: [ $( $(#[$kcfg:meta])* $kop:ident => ($kfn:path, $kkind:expr) ),* $(,)? ] $(,)?
    ) => {
        match $node {
            $($others)*
            $(
                $(#[$scfg])*
                CompiledNode::BuiltinOperator {
                    opcode: crate::OpCode::$sop,
                    args,
                    ..
                } => $sfn(args, $ctx, $engine, $arena),
            )*
            $(
                $(#[$icfg])*
                CompiledNode::BuiltinOperator {
                    opcode: crate::OpCode::$iop,
                    args,
                    iter_arg_kind,
                    ..
                } => $ifn(args, *iter_arg_kind, $ctx, $engine, $arena),
            )*
            $(
                $(#[$kcfg])*
                CompiledNode::BuiltinOperator {
                    opcode: crate::OpCode::$kop,
                    args,
                    ..
                } => $kfn(args, $ctx, $engine, $arena, $kkind),
            )*
        }
    };
}

/// Inner dispatch — never called directly; reachable only via
/// `Engine::dispatch_node` which handles the literal fast path,
/// breadcrumb accumulation, and trace recording.
///
/// `#[inline(always)]` is load-bearing here: this function is the hot
/// dispatch and the compiler inlines it into `dispatch_node` in the
/// single-file layout. Crossing the module boundary loses that inline
/// decision (measured ~1 ns regression on the 15 ns baseline).
#[inline(always)]
pub(super) fn dispatch_node_inner<'a>(
    engine: &Engine,
    node: &'a CompiledNode,
    ctx: &mut ContextStack<'a>,
    arena: &'a bumpalo::Bump,
) -> Result<&'a crate::arena::DataValue<'a>> {
    dispatch! {
        node: node,
        ctx: ctx,
        engine: engine,
        arena: arena,

        other: {
            // Compiled var: full dispatch via the arena helper. Root and
            // frame data are both arena-resident `DataValue`s, so lookups
            // are zero-copy borrows.
            CompiledNode::Var {
                scope_level,
                segments,
                reduce_hint,
                metadata_hint,
                default_value,
                ..
            } => crate::operators::variable::evaluate_val_compiled(
                crate::operators::variable::CompiledVarSpec {
                    scope_level: *scope_level,
                    segments,
                    reduce_hint: *reduce_hint,
                    metadata_hint: *metadata_hint,
                    default_value: default_value.as_deref(),
                },
                ctx,
                engine,
                arena,
            ),

            // Compiled exists: full dispatch — root scope walks the input
            // directly, others walk arena frame data. Result is always a
            // Bool singleton.
            #[cfg(feature = "ext-control")]
            CompiledNode::Exists(data) => crate::operators::variable::evaluate_exists_compiled(
                data.scope_level,
                &data.segments,
                ctx,
            ),

            // Value literal: the outer `dispatch_node` wrapper already
            // routes `Value` through `literal_fallback`, so this arm is
            // unreachable in normal flow. Defensive fallback — degrades
            // gracefully to a fresh arena alloc if a future code path
            // ever calls the inner dispatcher directly with a literal.
            CompiledNode::Value { value, .. } => Ok(super::literal_fallback(value, arena)),

            // Compiled missing / missing_some — pre-parsed segments.
            CompiledNode::Missing(data) => {
                crate::operators::missing::evaluate_compiled_missing(data, ctx, engine, arena)
            }
            CompiledNode::MissingSome(data) => {
                crate::operators::missing::evaluate_compiled_missing_some(data, ctx, engine, arena)
            }

            // Compile-time placeholder for malformed args on
            // `and` / `or` / `if`. Keep the canonical "Invalid Arguments"
            // message (the JSONLogic suite uses it as the error tag) but
            // attach the captured op name to the operator field so nested
            // failures still identify the misused op.
            CompiledNode::InvalidArgs { op_name, .. } => {
                Err(crate::Error::invalid_args().with_operator(*op_name))
            }

            // CompiledThrow — constant-folded error literal.
            #[cfg(feature = "error-handling")]
            CompiledNode::Throw(data) => Err(Error::thrown(data.error.clone())),

            // Out-of-line — bumpalo::Vec construction would otherwise force
            // a large stack frame on every dispatch arm via worst-case
            // spill sizing. See the comments on the helpers below.
            #[cfg(feature = "preserve")]
            CompiledNode::StructuredObject(data) => {
                evaluate_structured_object(data, ctx, engine, arena)
            }
            CompiledNode::Array { nodes, .. } => evaluate_array_literal(nodes, ctx, engine, arena),
            CompiledNode::CustomOperator(data) => evaluate_custom_operator(data, ctx, engine, arena),
        },

        // Standard `BuiltinOperator { opcode, args, .. } => fn(args, ctx,
        // engine, arena)` shape.
        simple: [
            // Variable / context
            Val => crate::operators::variable::evaluate_val,
            #[cfg(feature = "ext-control")]
            Exists => crate::operators::variable::evaluate_exists,

            // Array / collection
            Merge => crate::operators::array::evaluate_merge,
            Missing => crate::operators::missing::evaluate_missing,
            MissingSome => crate::operators::missing::evaluate_missing_some,
            #[cfg(feature = "ext-string")]
            Length => crate::operators::array::evaluate_length,
            #[cfg(feature = "ext-array")]
            Slice => crate::operators::array::evaluate_slice,

            // Arithmetic (binary)
            Add => crate::operators::arithmetic::evaluate_add,
            Multiply => crate::operators::arithmetic::evaluate_multiply,
            Subtract => crate::operators::arithmetic::evaluate_subtract,

            // Comparison
            Equals => crate::operators::comparison::evaluate_equals,
            StrictEquals => crate::operators::comparison::evaluate_strict_equals,
            NotEquals => crate::operators::comparison::evaluate_not_equals,
            StrictNotEquals => crate::operators::comparison::evaluate_strict_not_equals,
            GreaterThan => crate::operators::comparison::evaluate_greater_than,
            GreaterThanEqual => crate::operators::comparison::evaluate_greater_than_equal,
            LessThan => crate::operators::comparison::evaluate_less_than,
            LessThanEqual => crate::operators::comparison::evaluate_less_than_equal,

            // Logical
            Not => crate::operators::logical::evaluate_not,
            BoolCast => crate::operators::logical::evaluate_bool_cast,
            And => crate::operators::logical::evaluate_and,
            Or => crate::operators::logical::evaluate_or,

            // Control. `if` and `?:` both arrive as OpCode::If — see
            // `OpCode::FromStr`. `evaluate_if` handles ternary identically.
            If => crate::operators::control::evaluate_if,
            #[cfg(feature = "ext-control")]
            Coalesce => crate::operators::control::evaluate_coalesce,
            #[cfg(feature = "ext-control")]
            Switch => crate::operators::control::evaluate_switch,

            // String
            Concat => crate::operators::string::evaluate_concat,
            Substr => crate::operators::string::evaluate_substr,
            In => crate::operators::string::evaluate_in,
            #[cfg(feature = "ext-string")]
            StartsWith => crate::operators::string::evaluate_starts_with,
            #[cfg(feature = "ext-string")]
            EndsWith => crate::operators::string::evaluate_ends_with,
            #[cfg(feature = "ext-string")]
            Upper => crate::operators::string::evaluate_upper,
            #[cfg(feature = "ext-string")]
            Lower => crate::operators::string::evaluate_lower,
            #[cfg(feature = "ext-string")]
            Trim => crate::operators::string::evaluate_trim,
            #[cfg(feature = "ext-string")]
            Split => crate::operators::string::evaluate_split,

            // DateTime
            #[cfg(feature = "datetime")]
            Datetime => crate::operators::datetime::evaluate_datetime,
            #[cfg(feature = "datetime")]
            Timestamp => crate::operators::datetime::evaluate_timestamp,
            #[cfg(feature = "datetime")]
            ParseDate => crate::operators::datetime::evaluate_parse_date,
            #[cfg(feature = "datetime")]
            FormatDate => crate::operators::datetime::evaluate_format_date,
            #[cfg(feature = "datetime")]
            DateDiff => crate::operators::datetime::evaluate_date_diff,
            #[cfg(feature = "datetime")]
            Now => crate::operators::datetime::evaluate_now,

            // Type
            #[cfg(feature = "ext-control")]
            Type => crate::operators::inspect::evaluate_type,

            // Throw / Try
            #[cfg(feature = "error-handling")]
            Throw => crate::operators::error_handling::evaluate_throw,
            #[cfg(feature = "error-handling")]
            Try => crate::operators::error_handling::evaluate_try,
        ],

        // `BuiltinOperator { opcode, args, iter_arg_kind, .. } => fn(args,
        // *iter_arg_kind, ctx, engine, arena)` shape — operators that
        // consume the cached iterator-input classification.
        iter: [
            Filter => crate::operators::array::evaluate_filter,
            Map => crate::operators::array::evaluate_map,
            All => crate::operators::array::evaluate_all,
            Some => crate::operators::array::evaluate_some,
            None => crate::operators::array::evaluate_none,
            Reduce => crate::operators::array::evaluate_reduce,
            Max => crate::operators::arithmetic::evaluate_max,
            Min => crate::operators::arithmetic::evaluate_min,
            #[cfg(feature = "ext-array")]
            Sort => crate::operators::array::evaluate_sort,
        ],

        // `BuiltinOperator { opcode, args, .. } => fn(args, ctx, engine,
        // arena, kind)` shape — operators sharing one impl behind a
        // discriminator (Divide/Modulo, Abs/Ceil/Floor).
        with_kind: [
            Divide => (crate::operators::arithmetic::div_or_mod, crate::operators::arithmetic::DivOp::Divide),
            Modulo => (crate::operators::arithmetic::div_or_mod, crate::operators::arithmetic::DivOp::Modulo),
            #[cfg(feature = "ext-math")]
            Abs => (crate::operators::arithmetic::unary_math, crate::operators::arithmetic::UnaryMathOp::Abs),
            #[cfg(feature = "ext-math")]
            Ceil => (crate::operators::arithmetic::unary_math, crate::operators::arithmetic::UnaryMathOp::Ceil),
            #[cfg(feature = "ext-math")]
            Floor => (crate::operators::arithmetic::unary_math, crate::operators::arithmetic::UnaryMathOp::Floor),
        ],
    }
}

// Heavy arms below are kept out-of-line so the dispatch fn's stack frame
// is sized for the small/common arms only. Each builds a `bumpalo::Vec`
// (multi-word locals + drop glue) which, when inlined, forced the
// dispatch prologue to reserve ~464 B of stack on every recursive call.
// `#[inline(never)]` is load-bearing — see the comment on
// `dispatch_node_inner`.

#[cfg(feature = "preserve")]
#[inline(never)]
fn evaluate_structured_object<'a>(
    data: &'a crate::node::StructuredObjectData,
    ctx: &mut crate::arena::ContextStack<'a>,
    engine: &super::Engine,
    arena: &'a bumpalo::Bump,
) -> crate::Result<&'a crate::arena::DataValue<'a>> {
    use crate::arena::DataValue;
    if data.fields.is_empty() {
        return Ok(crate::arena::singletons::singleton_empty_object());
    }
    let mut pairs: bumpalo::collections::Vec<'a, (&'a str, DataValue<'a>)> =
        bumpalo::collections::Vec::with_capacity_in(data.fields.len(), arena);
    for (key, n) in data.fields.iter() {
        let val_av = engine.dispatch_node(n, ctx, arena)?;
        let val_owned = *val_av;
        let k: &'a str = arena.alloc_str(key);
        pairs.push((k, val_owned));
    }
    Ok(arena.alloc(DataValue::Object(pairs.into_bump_slice())))
}

#[inline(never)]
fn evaluate_array_literal<'a>(
    nodes: &'a [crate::CompiledNode],
    ctx: &mut crate::arena::ContextStack<'a>,
    engine: &super::Engine,
    arena: &'a bumpalo::Bump,
) -> crate::Result<&'a crate::arena::DataValue<'a>> {
    use crate::arena::DataValue;
    if nodes.is_empty() {
        return Ok(crate::arena::singletons::singleton_empty_array());
    }
    let mut items: bumpalo::collections::Vec<'a, DataValue<'a>> =
        bumpalo::collections::Vec::with_capacity_in(nodes.len(), arena);
    for n in nodes.iter() {
        let av = engine.dispatch_node(n, ctx, arena)?;
        items.push(*av);
    }
    Ok(arena.alloc(DataValue::Array(items.into_bump_slice())))
}

#[inline(never)]
fn evaluate_custom_operator<'a>(
    data: &'a crate::node::CustomOperatorData,
    ctx: &mut crate::arena::ContextStack<'a>,
    engine: &super::Engine,
    arena: &'a bumpalo::Bump,
) -> crate::Result<&'a crate::arena::DataValue<'a>> {
    use crate::arena::DataValue;
    let op = engine
        .custom_operators
        .get(&data.name)
        .ok_or_else(|| Error::invalid_operator(data.name.clone()))?;
    let mut args: bumpalo::collections::Vec<'a, &'a DataValue<'a>> =
        bumpalo::collections::Vec::with_capacity_in(data.args.len(), arena);
    for arg in data.args.iter() {
        args.push(engine.dispatch_node(arg, ctx, arena)?);
    }
    let mut wrapped = crate::operator::EvalContext::new(ctx);
    op.evaluate(&args, &mut wrapped, arena)
}
