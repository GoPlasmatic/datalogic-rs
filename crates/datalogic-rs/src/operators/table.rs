//! The built-in operator table: one row per operator.
//!
//! Every built-in operator is described once, in one row of the
//! [`operators!`] invocation below, and implemented once, in one plain
//! function the row points at. Everything else is generated from the row:
//!
//! | Generated | From |
//! |---|---|
//! | `enum OpCode` | the variant column, gated by the family's `cfg` |
//! | `OpCode::from_str` | the names column (canonical first, then aliases) |
//! | `OpCode::as_str` | the first name, or the row's `display` for an internal opcode |
//! | [`builtin_operator_names`] | the names column, in table order |
//! | `OpCode::meta` | the `OpMeta` column |
//! | `OpCode::iterates_arg0` | the `iter` shape |
//! | `OpCode::arity` | the `eager` signature (`raw` / `iter` rows report [`Arity::ANY`]) |
//! | [`dispatch_builtin`] | the shape and impl columns |
//! | [`CATALOGUE`] | every row in the source, compiled in or not |
//!
//! # Row grammar
//!
//! ```text
//! family Name (cfg-predicate) {
//!     Variant ["name", "alias", ...] => shape impl::path, OpMeta-expr;
//! }
//!
//! shape      := raw | iter | eager(Extractor, ...)
//! impl::path := a::b::f | a::b::f(extra-arg, ...)     (extra args: raw and eager rows)
//! ```
//!
//! - **`raw`**: `f(args, ctx, engine, arena, extra...)`. The body evaluates
//!   its own arguments (lazy and control-flow operators, variadics).
//! - **`iter`**: `f(args, iter_arg_kind, ctx, engine, arena)`. Also receives
//!   the iteration-source classification the populate pass cached for
//!   `args[0]` (and only `iter` rows get one classified).
//! - **`eager(E0, E1, ...)`**: `f(cx, e0, e1, ..., extra...)`, a plain typed function.
//!   The generated adapter (see [`super::eager`]) checks arity against the
//!   row's `on_missing` / `on_extra` policy, evaluates every present
//!   argument, coerces each through its extractor, and converts the result.
//!
//! The `cfg` predicate is written once per family and applied to every row
//! in the block, so a row cannot sit behind the wrong feature gate.
//!
//! # Adding an operator
//!
//! 1. A row in the right family below.
//! 2. The function the row points at.
//! 3. A suite file under `tests/suites/`.
//!
//! If the operator reads the data context, pushes a frame, has an effect,
//! or treats missing arguments specially, declare it in the row's
//! `OpMeta`; the fold, CSE and scope passes derive their classification
//! from it (see [`super::meta`]).

// Which variants, extractors and adapters a build uses depends on which
// operator families it compiles in; with every family on, all of them are
// used and dead code is still reported.
#![cfg_attr(not(feature = "all-operators"), allow(dead_code, unused_imports))]

use super::eager::{self, Arity};
use super::extract::{
    self, Any, Int, Lazy, Lenient, Nullable, Obj, Opt, Rest, Str, StrictNum, Truthy,
};
use super::meta::*;
use crate::arena::{ContextStack, DataValue};
use crate::compile::hooks;
use crate::operators::array::IterArgKind;
use crate::{CompiledNode, Engine, Result};
use bumpalo::Bump;

#[cfg(feature = "datetime")]
use super::datetime;
#[cfg(feature = "error-handling")]
use super::error_handling;
#[cfg(feature = "flagd")]
use super::flagd;
#[cfg(feature = "ext-control")]
use super::inspect;
#[cfg(feature = "ext-object")]
use super::object;
#[cfg(feature = "tensor")]
use super::tensor::{self, DTypeArg, F64Arg, I64Arg, I64List, ShapeArg, TensorArg, UsizeArg};
use super::{arithmetic, array, comparison, control, logical, missing, string, variable};

/// One operator as the source declares it, whether or not its family is
/// compiled into this build. Lets tooling and the conformance runner tell
/// "gated off" from "misspelled".
#[doc(hidden)]
#[derive(Debug, Clone, Copy)]
pub struct CatalogueEntry {
    /// Family name, as written in the table (`Core`, `DateTime`, ...).
    pub family: &'static str,
    /// The family's `cfg` predicate, as written (`all()`,
    /// `feature = "datetime"`).
    pub gate: &'static str,
    /// Whether the family is compiled into this build.
    pub enabled: bool,
    /// The `OpCode` variant name.
    pub variant: &'static str,
    /// Canonical name first, then aliases. Empty for an internal opcode.
    pub names: &'static [&'static str],
    /// The row's shape, as written (`raw`, `iter`, `eager(Str, Str)`).
    pub shape: &'static str,
}

impl CatalogueEntry {
    /// The Cargo feature that gates this operator, or `None` for the core
    /// family.
    pub fn feature(&self) -> Option<&'static str> {
        let rest = self.gate.strip_prefix("feature")?.trim_start();
        let rest = rest.strip_prefix('=')?.trim();
        Some(rest.trim_matches('"'))
    }
}

macro_rules! operators {
    (
        $(
            family $fam:ident ($gate:meta) {
                $(
                    $v:ident [ $($name:literal),* ] => $shape:ident $( ( $($ext:ty),* ) )?
                        $($f:ident)::+ $( ( $($karg:expr),* ) )? , $meta:expr ;
                )*
            }
        )*
    ) => {
        /// A built-in operator. Generated from the operator table.
        #[repr(u8)]
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(crate) enum OpCode {
            $( $( #[cfg($gate)] $v, )* )*
        }

        /// Every name `OpCode::from_str` accepts in this build, in table
        /// order (canonical first, then its aliases).
        const NAMES: &[&str] = &[ $( $( $( #[cfg($gate)] $name, )* )* )* ];

        /// Every operator in the source, compiled in or not.
        #[doc(hidden)]
        pub const CATALOGUE: &[CatalogueEntry] = &[
            $( $(
                operators!(@entry $fam ($gate) $v [ $($name),* ] $shape $( ( $($ext),* ) )?),
            )* )*
        ];

        impl std::str::FromStr for OpCode {
            type Err = ();

            fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
                match s {
                    $( $( $( #[cfg($gate)] $name => Ok(OpCode::$v), )* )* )*
                    _ => Err(()),
                }
            }
        }

        impl OpCode {
            /// Every opcode compiled into this build, in table order.
            pub(crate) const ALL: &'static [OpCode] = &[ $( $( #[cfg($gate)] OpCode::$v, )* )* ];

            /// The row's declared facts.
            pub(crate) const fn meta(self) -> &'static OpMeta {
                match self {
                    $( $( #[cfg($gate)] OpCode::$v => { const META: OpMeta = $meta; &META } )* )*
                }
            }

            /// Canonical name (for display, serialization and errors).
            pub(crate) const fn as_str(self) -> &'static str {
                match self {
                    $( $( #[cfg($gate)] OpCode::$v => operators!(@canonical OpCode::$v; $($name),*), )* )*
                }
            }

            /// The row's `algebra`, by value: one load from a table
            /// indexed by discriminant. For the fast paths that run it on
            /// every evaluation.
            #[inline(always)]
            pub(crate) fn algebra(self) -> Option<Algebra> {
                const ALGEBRA: &[Option<Algebra>] = &[
                    $( $( #[cfg($gate)] { const META: OpMeta = $meta; META.algebra }, )* )*
                ];
                ALGEBRA[self as usize]
            }

            /// The row's arithmetic operation, if its `algebra` is one.
            /// A `match` of per-row constants rather than a table: the
            /// arithmetic rows are contiguous and in `ArithOp` order, so
            /// this lowers to a range check with no memory access, which
            /// the `map` / `reduce` fusion detectors (run per evaluation)
            /// measurably need.
            #[inline(always)]
            pub(crate) const fn arith_op(self) -> Option<ArithOp> {
                match self {
                    $( $(
                        #[cfg($gate)]
                        OpCode::$v => {
                            const OP: Option<ArithOp> = match ($meta).algebra {
                                Some(Algebra::Arith(op)) => Some(op),
                                _ => None,
                            };
                            OP
                        }
                    )* )*
                }
            }

            /// The row as the catalogue records it: family, gate, names
            /// and shape.
            pub(crate) const fn catalogue_entry(self) -> &'static CatalogueEntry {
                match self {
                    $( $(
                        #[cfg($gate)]
                        OpCode::$v => {
                            const ENTRY: CatalogueEntry =
                                operators!(@entry $fam ($gate) $v [ $($name),* ] $shape $( ( $($ext),* ) )?);
                            &ENTRY
                        }
                    )* )*
                }
            }

            /// Whether `args[0]` is an iteration source (an `iter` row),
            /// so the populate pass classifies it.
            pub(crate) const fn iterates_arg0(self) -> bool {
                match self {
                    $( $( #[cfg($gate)] OpCode::$v => operators!(@is_iter $shape), )* )*
                }
            }

            /// Argument count the row reads, derived from its `eager`
            /// signature. `raw` and `iter` rows check their own.
            pub(crate) const fn arity(self) -> Arity {
                match self {
                    $( $( #[cfg($gate)] OpCode::$v => operators!(@arity $shape $( ( $($ext),* ) )?), )* )*
                }
            }
        }

        /// Dispatch a `BuiltinOperator` node to its implementation.
        ///
        /// `#[inline(always)]`: inlined into `dispatch_node_inner`, the
        /// `match` lowers to the same jump table over the `OpCode`
        /// discriminant the hand-written arms produced. Every arm only
        /// calls a function; `eager` rows call their adapter, which is a
        /// separate item, so no extraction code lands in the dispatch
        /// frame.
        #[inline(always)]
        pub(crate) fn dispatch_builtin<'a>(
            opcode: OpCode,
            args: &'a [CompiledNode],
            iter_arg_kind: IterArgKind,
            ctx: &mut ContextStack<'a>,
            engine: &Engine,
            arena: &'a Bump,
        ) -> Result<&'a DataValue<'a>> {
            let _ = iter_arg_kind;
            match opcode {
                $( $(
                    #[cfg($gate)]
                    OpCode::$v => operators!(
                        @call $v $shape $( ( $($ext),* ) )? [ $($f)::+ ] [ $( $($karg),* )? ]
                        (args, iter_arg_kind, ctx, engine, arena)
                    ),
                )* )*
            }
        }
    };

    // ── catalogue entry ───────────────────────────────────────────────────
    (@entry $fam:ident ($gate:meta) $v:ident [ $($name:literal),* ] $shape:ident $( ( $($ext:ty),* ) )?) => {
        CatalogueEntry {
            family: stringify!($fam),
            gate: stringify!($gate),
            enabled: cfg!($gate),
            variant: stringify!($v),
            names: &[ $($name),* ],
            shape: stringify!($shape $( ( $($ext),* ) )?),
        }
    };

    // ── canonical name ────────────────────────────────────────────────────
    (@canonical $op:expr; $first:literal $(, $rest:literal)*) => { $first };
    (@canonical $op:expr;) => {
        match $op.meta().display {
            Some(name) => name,
            None => panic!("an internal opcode needs a `display` name"),
        }
    };

    // ── shape-derived facts ───────────────────────────────────────────────
    (@is_iter iter) => { true };
    (@is_iter $other:ident) => { false };

    (@arity raw) => { Arity::ANY };
    (@arity iter) => { Arity::ANY };
    (@arity eager ( $($e:ty),* )) => {
        const { Arity::of(&[ $( <$e as extract::Extract<'static>>::SPAN ),* ]) }
    };

    // ── dispatch arm bodies ───────────────────────────────────────────────
    (@call $v:ident raw [ $($p:tt)* ] [ $($k:expr),* ]
        ($args:ident, $iak:ident, $ctx:ident, $engine:ident, $arena:ident)) => {
        $($p)*($args, $ctx, $engine, $arena $(, $k)*)
    };
    (@call $v:ident iter [ $($p:tt)* ] [ ]
        ($args:ident, $iak:ident, $ctx:ident, $engine:ident, $arena:ident)) => {
        $($p)*($args, $iak, $ctx, $engine, $arena)
    };
    (@call $v:ident eager ( ) [ $($p:tt)* ] [ ] ($args:ident, $iak:ident, $ctx:ident, $engine:ident, $arena:ident)) => {
        eager::call0::<{ OpCode::$v as u8 }, _, _>($args, $ctx, $engine, $arena, $($p)*)
    };
    (@call $v:ident eager ( $a0:ty ) [ $($p:tt)* ] [ ] ($args:ident, $iak:ident, $ctx:ident, $engine:ident, $arena:ident)) => {
        eager::call1::<{ OpCode::$v as u8 }, $a0, _, _>($args, $ctx, $engine, $arena, $($p)*)
    };
    (@call $v:ident eager ( $a0:ty ) [ $($p:tt)* ] [ $($k:expr),+ ] ($args:ident, $iak:ident, $ctx:ident, $engine:ident, $arena:ident)) => {
        eager::call1::<{ OpCode::$v as u8 }, $a0, _, _>($args, $ctx, $engine, $arena,
            |cx, v0| $($p)*(cx, v0, $($k),+))
    };
    (@call $v:ident eager ( $a0:ty, $a1:ty ) [ $($p:tt)* ] [ ] ($args:ident, $iak:ident, $ctx:ident, $engine:ident, $arena:ident)) => {
        eager::call2::<{ OpCode::$v as u8 }, $a0, $a1, _, _>($args, $ctx, $engine, $arena, $($p)*)
    };
    (@call $v:ident eager ( $a0:ty, $a1:ty ) [ $($p:tt)* ] [ $($k:expr),+ ] ($args:ident, $iak:ident, $ctx:ident, $engine:ident, $arena:ident)) => {
        eager::call2::<{ OpCode::$v as u8 }, $a0, $a1, _, _>($args, $ctx, $engine, $arena,
            |cx, v0, v1| $($p)*(cx, v0, v1, $($k),+))
    };
    (@call $v:ident eager ( $a0:ty, $a1:ty, $a2:ty ) [ $($p:tt)* ] [ ] ($args:ident, $iak:ident, $ctx:ident, $engine:ident, $arena:ident)) => {
        eager::call3::<{ OpCode::$v as u8 }, $a0, $a1, $a2, _, _>($args, $ctx, $engine, $arena, $($p)*)
    };
    (@call $v:ident eager ( $a0:ty, $a1:ty, $a2:ty ) [ $($p:tt)* ] [ $($k:expr),+ ] ($args:ident, $iak:ident, $ctx:ident, $engine:ident, $arena:ident)) => {
        eager::call3::<{ OpCode::$v as u8 }, $a0, $a1, $a2, _, _>($args, $ctx, $engine, $arena,
            |cx, v0, v1, v2| $($p)*(cx, v0, v1, v2, $($k),+))
    };
    (@call $v:ident eager ( $a0:ty, $a1:ty, $a2:ty, $a3:ty ) [ $($p:tt)* ] [ ] ($args:ident, $iak:ident, $ctx:ident, $engine:ident, $arena:ident)) => {
        eager::call4::<{ OpCode::$v as u8 }, $a0, $a1, $a2, $a3, _, _>($args, $ctx, $engine, $arena, $($p)*)
    };
    (@call $v:ident eager ( $a0:ty, $a1:ty, $a2:ty, $a3:ty ) [ $($p:tt)* ] [ $($k:expr),+ ] ($args:ident, $iak:ident, $ctx:ident, $engine:ident, $arena:ident)) => {
        eager::call4::<{ OpCode::$v as u8 }, $a0, $a1, $a2, $a3, _, _>($args, $ctx, $engine, $arena,
            |cx, v0, v1, v2, v3| $($p)*(cx, v0, v1, v2, v3, $($k),+))
    };
    (@call $v:ident $($rest:tt)*) => {
        compile_error!(concat!(
            "operator row `", stringify!($v),
            "`: unknown shape, more than four eager arguments, or extra arguments on an iter row"
        ))
    };
}

operators! {
    family Core (all()) {
        // ── variable access ──────────────────────────────────────────────
        // `var` is accepted as a synonym of `val`; the compile hook reads
        // the source name, because `var`'s second argument is a default
        // and `val`'s is a path segment.
        Val ["val", "var"] => raw variable::evaluate_val,
            OpMeta { reads_context: true, compile: Some(CompileHook::Args(hooks::val)), ..PURE };
        // `{"var": [path, default]}` with a computed path. Internal: only
        // the `var` compile hook emits it; it renders back as `var`.
        VarDefault [] => raw variable::evaluate_var_default,
            OpMeta { reads_context: true, display: Some("var"), ..PURE };

        // ── comparison ───────────────────────────────────────────────────
        Equals ["=="] => raw comparison::evaluate_equals,
            OpMeta { algebra: Some(Algebra::Eq { strict: false, negate: false }), ..PURE };
        StrictEquals ["==="] => raw comparison::evaluate_strict_equals,
            OpMeta { algebra: Some(Algebra::Eq { strict: true, negate: false }), ..PURE };
        NotEquals ["!="] => raw comparison::evaluate_not_equals,
            OpMeta { algebra: Some(Algebra::Eq { strict: false, negate: true }), ..PURE };
        StrictNotEquals ["!=="] => raw comparison::evaluate_strict_not_equals,
            OpMeta { algebra: Some(Algebra::Eq { strict: true, negate: true }), ..PURE };
        GreaterThan [">"] => raw comparison::evaluate_greater_than,
            OpMeta { algebra: Some(Algebra::Ord(OrdOp::Gt)), ..PURE };
        GreaterThanEqual [">="] => raw comparison::evaluate_greater_than_equal,
            OpMeta { algebra: Some(Algebra::Ord(OrdOp::Ge)), ..PURE };
        LessThan ["<"] => raw comparison::evaluate_less_than,
            OpMeta { algebra: Some(Algebra::Ord(OrdOp::Lt)), ..PURE };
        LessThanEqual ["<="] => raw comparison::evaluate_less_than_equal,
            OpMeta { algebra: Some(Algebra::Ord(OrdOp::Le)), ..PURE };

        // ── logic and control (lazy) ─────────────────────────────────────
        Not ["!"] => eager(Opt<Truthy>) logical::not, PURE;
        BoolCast ["!!"] => eager(Opt<Truthy>) logical::bool_cast, PURE;
        And ["and"] => raw logical::evaluate_and, OpMeta { args_form: ArgsForm::ArrayOnly, ..PURE };
        Or ["or"] => raw logical::evaluate_or, OpMeta { args_form: ArgsForm::ArrayOnly, ..PURE };
        // `?:` is accepted as a synonym; the ternary is the 3-argument `if`.
        If ["if", "?:"] => raw control::evaluate_if, OpMeta { args_form: ArgsForm::ArrayOnly, ..PURE };

        // ── arithmetic ───────────────────────────────────────────────────
        Add ["+"] => raw arithmetic::evaluate_add,
            OpMeta { algebra: Some(Algebra::Arith(ArithOp::Add)), ..PURE };
        Subtract ["-"] => raw arithmetic::evaluate_subtract,
            OpMeta { algebra: Some(Algebra::Arith(ArithOp::Sub)), ..PURE };
        Multiply ["*"] => raw arithmetic::evaluate_multiply,
            OpMeta { algebra: Some(Algebra::Arith(ArithOp::Mul)), ..PURE };
        Divide ["/"] => raw arithmetic::div_or_mod(arithmetic::DivOp::Divide), PURE;
        Modulo ["%"] => raw arithmetic::div_or_mod(arithmetic::DivOp::Modulo), PURE;
        // `max` / `min` / `merge` disambiguate a literal array from an
        // argument list at runtime (`{"max": [[1, 2]]}`), so folding their
        // static form would bake in the wrong reading.
        Max ["max"] => iter arithmetic::evaluate_max, OpMeta { fold: Fold::Never, ..PURE };
        Min ["min"] => iter arithmetic::evaluate_min, OpMeta { fold: Fold::Never, ..PURE };

        // ── strings ──────────────────────────────────────────────────────
        Concat ["cat"] => raw string::evaluate_concat, OpMeta { cost: Cost::Bytes, ..PURE };
        Substr ["substr"] => eager(Str, Lenient<Int>, Lenient<Int>) string::substr,
            OpMeta { on_missing: Miss::Return(crate::arena::singletons::singleton_empty_string), cost: Cost::Bytes, ..PURE };
        In ["in"] => eager(Any, Any) string::in_,
            OpMeta { on_missing: Miss::Return(crate::arena::singletons::singleton_false), cost: Cost::Bytes, ..PURE };

        // ── arrays ───────────────────────────────────────────────────────
        Merge ["merge"] => raw array::evaluate_merge,
            OpMeta { fold: Fold::Never, cost: Cost::PerItem, ..PURE };
        Filter ["filter"] => iter array::evaluate_filter, ITERATOR;
        Map ["map"] => iter array::evaluate_map, ITERATOR;
        Reduce ["reduce"] => iter array::evaluate_reduce, ITERATOR;
        All ["all"] => iter array::evaluate_all, ITERATOR;
        Some ["some"] => iter array::evaluate_some, ITERATOR;
        None ["none"] => iter array::evaluate_none, ITERATOR;

        // ── missing values ───────────────────────────────────────────────
        Missing ["missing"] => raw missing::evaluate_missing,
            OpMeta { reads_context: true, compile: Some(CompileHook::Args(hooks::missing)), ..PURE };
        MissingSome ["missing_some"] => raw missing::evaluate_missing_some,
            OpMeta { reads_context: true, compile: Some(CompileHook::Args(hooks::missing_some)), ..PURE };
    }

    family DateTime (feature = "datetime") {
        Datetime ["datetime"] => eager(Any) datetime::datetime,
            OpMeta { on_missing: Miss::Err("datetime requires an argument"), ..PURE };
        Timestamp ["timestamp"] => eager(Any) datetime::timestamp,
            OpMeta { on_missing: Miss::Err("timestamp requires an argument"), ..PURE };
        ParseDate ["parse_date"] => eager(Any, Any, Lazy) datetime::parse_date,
            OpMeta {
                on_missing: Miss::Err("parse_date requires date string and format"),
                compile: Some(CompileHook::Args(hooks::timezone_literal)),
                ..PURE
            };
        FormatDate ["format_date"] => eager(Any, Any, Lazy) datetime::format_date,
            OpMeta {
                on_missing: Miss::Err("format_date requires datetime and format"),
                compile: Some(CompileHook::Args(hooks::timezone_literal)),
                ..PURE
            };
        DateDiff ["date_diff"] => eager(Any, Any, Any) datetime::date_diff,
            OpMeta { on_missing: Miss::Err("date_diff requires two dates and a unit"), ..PURE };
        Now ["now"] => eager() datetime::now, OpMeta { effect: Effect::Clock, ..PURE };
    }

    family ExtString (feature = "ext-string") {
        Length ["length"] => eager(Any) array::length,
            OpMeta { on_extra: Extra::InvalidArgs, ..PURE };
        StartsWith ["starts_with"] => eager(Str, Str) string::starts_with, OpMeta { cost: Cost::Bytes, ..PURE };
        EndsWith ["ends_with"] => eager(Str, Str) string::ends_with, OpMeta { cost: Cost::Bytes, ..PURE };
        Upper ["upper"] => eager(Str) string::upper, OpMeta { cost: Cost::Bytes, ..PURE };
        Lower ["lower"] => eager(Str) string::lower, OpMeta { cost: Cost::Bytes, ..PURE };
        Trim ["trim"] => eager(Str) string::trim, OpMeta { cost: Cost::Bytes, ..PURE };
        Split ["split"] => eager(Str, Str) string::split, OpMeta { cost: Cost::Bytes, ..PURE };
    }

    family ExtArray (feature = "ext-array") {
        // `sort`'s `args[1]` is the scalar direction flag; the key
        // expression at `args[2]` runs per element.
        Sort ["sort"] => iter array::evaluate_sort,
            OpMeta { frames: Frames::At(2), cost: Cost::NLogN, ..ITERATOR };
        Slice ["slice"] => raw array::evaluate_slice, OpMeta { cost: Cost::PerItem, ..PURE };
        GroupBy ["group_by"] => iter array::evaluate_group_by, ITERATOR;
        // Without a key expression nothing runs under a frame, so
        // `distinct` folds like any pure operator.
        Distinct ["distinct"] => iter array::evaluate_distinct,
            OpMeta { cost: Cost::Quadratic, ..ITERATOR };
    }

    family ExtObject (feature = "ext-object") {
        Keys ["keys"] => eager(Nullable<Obj>) object::keys, OpMeta { cost: Cost::PerItem, ..PURE };
        Values ["values"] => eager(Nullable<Obj>) object::values, OpMeta { cost: Cost::PerItem, ..PURE };
        Entries ["entries"] => eager(Nullable<Obj>) object::entries, OpMeta { cost: Cost::PerItem, ..PURE };
    }

    family ExtControl (feature = "ext-control") {
        Exists ["exists"] => raw variable::evaluate_exists,
            OpMeta { reads_context: true, compile: Some(CompileHook::Args(hooks::exists)), ..PURE };
        Coalesce ["??"] => raw control::evaluate_coalesce, PURE;
        Switch ["switch", "match"] => raw control::evaluate_switch, PURE;
        Type ["type"] => eager(Any) inspect::type_,
            OpMeta { on_missing: Miss::Return(inspect::type_of_nothing), ..PURE };
    }

    family ErrorHandling (feature = "error-handling") {
        Try ["try"] => raw error_handling::evaluate_try,
            OpMeta { effect: Effect::Catches, frames: Frames::LastIfMulti, ..PURE };
        Throw ["throw"] => raw error_handling::evaluate_throw,
            OpMeta { effect: Effect::Throws, compile: Some(CompileHook::Args(hooks::throw_literal)), ..PURE };
    }

    family ExtMath (feature = "ext-math") {
        Abs ["abs"] => eager(StrictNum, Rest<StrictNum>) arithmetic::unary_math(arithmetic::UnaryMathOp::Abs), PURE;
        Ceil ["ceil"] => eager(StrictNum, Rest<StrictNum>) arithmetic::unary_math(arithmetic::UnaryMathOp::Ceil), PURE;
        Floor ["floor"] => eager(StrictNum, Rest<StrictNum>) arithmetic::unary_math(arithmetic::UnaryMathOp::Floor), PURE;
    }

    family Tensor (feature = "tensor") {
        // Variant names carry a `Tensor` prefix so they never collide with
        // an existing opcode (`Concat` is `cat`, `Type` is `type`).
        TensorMake ["tensor"] => eager(Any, Lazy) tensor::tensor,
            OpMeta { compile: Some(CompileHook::Raw(hooks::tensor_wire_body)), ..TENSOR };
        TensorZeros ["zeros"] => eager(ShapeArg, DTypeArg) tensor::zeros, TENSOR;
        TensorFull ["full"] => eager(ShapeArg, DTypeArg, Any) tensor::full, TENSOR;
        TensorScatter ["scatter"] => eager(Any, ShapeArg, DTypeArg, Opt<Any>) tensor::scatter, TENSOR;
        TensorRleExpand ["rle_expand"] => eager(Any, ShapeArg, DTypeArg) tensor::rle_expand, TENSOR;
        TensorOneHot ["one_hot"] => eager(Any, UsizeArg, DTypeArg) tensor::one_hot, TENSOR;
        TensorStack ["stack"] => eager(Any, I64Arg) tensor::stack, TENSOR;
        TensorConcat ["concat"] => eager(Any, I64Arg) tensor::concat, TENSOR;
        TensorUnstack ["unstack"] => eager(TensorArg, I64Arg) tensor::unstack, TENSOR;
        // `reshape` shares the input's payload and `shape` / `dtype` read the
        // header, so each charges a flat 1 rather than per element.
        TensorReshape ["reshape"] => eager(TensorArg, ShapeArg) tensor::reshape,
            OpMeta { cost: Cost::Node, ..TENSOR };
        TensorTranspose ["transpose"] => eager(TensorArg, Opt<ShapeArg>) tensor::transpose, TENSOR;
        TensorPad ["pad"] => eager(TensorArg, ShapeArg, ShapeArg, Opt<Any>) tensor::pad, TENSOR;
        TensorCrop ["crop"] => eager(TensorArg, ShapeArg, ShapeArg) tensor::crop, TENSOR;
        TensorCast ["cast"] => eager(TensorArg, DTypeArg) tensor::cast, TENSOR;
        TensorNormalize ["normalize"] => eager(TensorArg, F64Arg, Opt<F64Arg>) tensor::normalize, TENSOR;
        TensorArgmax ["argmax"] => eager(TensorArg, I64Arg) tensor::argmax, TENSOR;
        TensorGather ["gather"] => eager(TensorArg, I64List, Opt<I64Arg>) tensor::gather, TENSOR;
        TensorToList ["to_list"] => eager(TensorArg) tensor::to_list, TENSOR;
        TensorShape ["shape"] => eager(TensorArg) tensor::shape, OpMeta { cost: Cost::Node, ..TENSOR };
        TensorDtype ["dtype"] => eager(TensorArg) tensor::dtype, OpMeta { cost: Cost::Node, ..TENSOR };
    }

    family Flagd (feature = "flagd") {
        // Without a bucketing expression, `fractional` reads
        // `$flagd.flagKey` and `targetingKey` from the root data.
        // Never memoised: CSE was introduced with an explicit exclusion
        // list (fractional, sem_ver, now, try, throw) and no soundness
        // argument was made for the flagd pair; a flag rule calls each
        // once, so there is nothing to gain from relaxing it.
        Fractional ["fractional"] => raw flagd::evaluate_fractional,
            OpMeta { reads_context: true, cse: Cse::Never, ..PURE };
        // Pure given its arguments, so a fully literal call folds. Never
        // memoised, for the same reason as `fractional`.
        SemVer ["sem_ver"] => eager(Any, Any, Any) flagd::sem_ver,
            OpMeta {
                on_missing: Miss::Return(crate::arena::singletons::singleton_null),
                on_extra: Extra::Return(crate::arena::singletons::singleton_null),
                cse: Cse::Never,
                ..PURE
            };
    }
}

/// Every name [`OpCode`]'s `from_str` accepts in this build: canonical
/// names and their aliases, in table order. Derived from the table, so it
/// cannot drift from dispatch.
pub(crate) fn builtin_operator_names() -> impl Iterator<Item = &'static str> {
    NAMES.iter().copied()
}
