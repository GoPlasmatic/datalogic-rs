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
//! | `OpCode::iterates_arg0` | the `iter` and `each` shapes |
//! | `OpCode::arity` | the `eager` signature, or a `raw` / `iter` row's `[arity]` ([`Arity::ANY`] without one) |
//! | [`dispatch_builtin`] | the shape and impl columns |
//! | [`CATALOGUE`] | every row in the source, compiled in or not |
//!
//! # Row grammar
//!
//! ```text
//! family Name (cfg-predicate) = DEFAULT {
//!     Variant ["name", "alias", ...] => shape impl::path [@ Kind(payload)] [{ field: value, ... [..BASE] }];
//! }
//!
//! shape      := raw[arity]? | iter[arity]? | each[arity] | eager(Extractor, ...)
//! arity      := n | min.. | min..=max
//! impl::path := a::b::f | a::b::f(extra-arg, ...)
//! ```
//!
//! - **`raw`**: `f(args, ctx, engine, arena, extra...)`. The body evaluates
//!   its own arguments: for an operator that inspects its argument nodes or
//!   evaluates only some of them (`if`, `val`, `throw`), and the few rows
//!   that measured faster this way (the comparisons, `and` / `or`). With
//!   `[arity]`, the generated arm first applies the row's `on_missing` /
//!   `on_extra` policy (see [`eager::raw`]), so the body can index any
//!   position below the minimum; without it, the body takes any count.
//! - **`iter`**: `f(args, iter_arg_kind, ctx, engine, arena)`. Also receives
//!   the iteration-source classification the populate pass cached for
//!   `args[0]` (and only `iter` rows get one classified).
//! - **`each[arity]`**: `f(items, args, ctx, engine, arena, extra...)`. An
//!   iterator whose source the generated arm resolves (see [`eager::each`]):
//!   a null, missing or empty-array `args[0]` returns the row's
//!   `on_empty_source` without calling `f`, and `f` receives the rest as
//!   [`Items`](super::array::Items) (a non-empty array, an object, or a
//!   scalar), plus every argument for its body and options.
//! - **`eager(E0, E1, ...)`**: `f(cx, e0, e1, ..., extra...)`, a plain typed function.
//!   The generated adapter (see [`super::eager`]) checks arity against the
//!   row's `on_missing` / `on_extra` policy, evaluates every present
//!   argument, coerces each through its extractor, and converts the result.
//!
//! **`@ Kind(payload)`** names the row's operation once: the row's `algebra`
//! becomes `Algebra::Kind(payload)` and `payload` is passed to the body as
//! its last argument (every shape). One body then serves every
//! row of the kind (`comparison::ordered` for `>`, `>=`, `<`, `<=`), and the
//! constant that selects its behaviour cannot disagree with what the fast
//! paths read. A row with `@` must not also set `algebra` in its `OpMeta`
//! (a compile error).
//!
//! The `cfg` predicate is written once per family and applied to every row
//! in the block, so a row cannot sit behind the wrong feature gate.
//!
//! **Metadata.** Each family names a default [`OpMeta`] preset
//! (`= PURE`, `= TENSOR`, ...). A row without braces takes it as is; a row
//! with `{ field: value, ... }` overrides those fields, and a trailing
//! `..BASE` swaps the default for another preset
//! (`{ on_empty_source: Some(singleton_empty_array), ..ITERATOR }`).
//!
//! # Adding an operator
//!
//! 1. A row in the right family below.
//! 2. The function the row points at.
//! 3. A suite file under `tests/suites/`.
//!
//! If the operator reads the data context, pushes a frame, has an effect,
//! or treats missing arguments specially, declare it in the row's
//! braces; the fold, CSE and scope passes derive their classification
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
use crate::arena::singletons::{
    singleton_empty_array, singleton_empty_string, singleton_false, singleton_null, singleton_true,
};
use crate::arena::{ContextStack, DataValue};
use crate::compile::hooks;
use crate::operators::array::IterArgKind;
use crate::{CompiledNode, Engine, Result};
use bumpalo::Bump;

#[cfg(feature = "ext-math")]
use super::arithmetic::UnaryMathOp;
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
    /// The row's shape keyword and `eager` signature, as written (`raw`,
    /// `iter`, `eager(Str, Str)`). A declared `[arity]` is reported by `OpCode::arity`.
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
            family $fam:ident ($gate:meta) = $default:path {
                $(
                    $v:ident [ $($name:literal),* ] => $shape:ident $( [ $($ar:tt)* ] )? $( ( $($ext:ty),* ) )?
                        $($f:ident)::+ $( ( $($karg:expr),* ) )?
                        $( @ $alg:ident ( $payload:expr ) )?
                        $( { $($meta:tt)* } )? ;
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

        /// A family of built-in operators: the JSONLogic core, or one of the
        /// extension families a Cargo feature compiles in. Every family is
        /// named in every build; [`Family::is_compiled`] says whether this
        /// build has it. See
        /// [`EngineBuilder::with_families`](crate::EngineBuilder::with_families).
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        #[non_exhaustive]
        pub enum Family {
            $(
                #[doc = concat!("The `", stringify!($fam), "` family (`", stringify!($gate), "`).")]
                $fam,
            )*
        }

        impl Family {
            /// Every family, in table order, compiled into this build or not.
            pub const ALL: &'static [Family] = &[ $( Family::$fam, )* ];

            /// The family's name as [`Engine::operators`](crate::Engine::operators)
            /// reports it in [`OperatorInfo::family`](crate::OperatorInfo::family)
            /// (`"Core"`, `"ExtString"`, ...).
            pub const fn name(self) -> &'static str {
                match self {
                    $( Family::$fam => stringify!($fam), )*
                }
            }

            /// Whether this build compiled the family in (its Cargo feature
            /// is on). The core always is.
            pub const fn is_compiled(self) -> bool {
                match self {
                    $( Family::$fam => cfg!($gate), )*
                }
            }

            /// This family's bit in an engine's family set.
            #[inline]
            pub(crate) const fn bit(self) -> u32 {
                1 << (self as u32)
            }
        }

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
                    $( $( #[cfg($gate)] OpCode::$v => { const META: OpMeta = operators!(@row_meta [$default] { $( $($meta)* )? } $( $alg ($payload) )?); &META } )* )*
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
                    $( $( #[cfg($gate)] { const META: OpMeta = operators!(@row_meta [$default] { $( $($meta)* )? } $( $alg ($payload) )?); META.algebra }, )* )*
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
                            const OP: Option<ArithOp> = match (operators!(@row_meta [$default] { $( $($meta)* )? } $( $alg ($payload) )?)).algebra {
                                Some(Algebra::Arith(op)) => Some(op),
                                _ => None,
                            };
                            OP
                        }
                    )* )*
                }
            }

            /// The family the row belongs to.
            pub(crate) const fn family(self) -> Family {
                match self {
                    $( $( #[cfg($gate)] OpCode::$v => Family::$fam, )* )*
                }
            }

            /// Whether the row's family is in `families`, a set of
            /// [`Family::bit`]s.
            #[inline]
            pub(crate) const fn in_families(self, families: u32) -> bool {
                families & self.family().bit() != 0
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
                    $( $( #[cfg($gate)] OpCode::$v => operators!(@arity $shape $( [ $($ar)* ] )? $( ( $($ext),* ) )?), )* )*
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
                        @call $v $shape { $( [ $($ar)* ] )? } $( ( $($ext),* ) )? [ $($f)::+ ] [ $( $($karg),* )? ] [ $($payload)? ]
                        (args, iter_arg_kind, ctx, engine, arena)
                    ),
                )* )*
            }
        }
    };

    // ── row metadata ──────────────────────────────────────────────────────
    // A row's `{ field: value, ... }` over its family's default, or over
    // the `..BASE` it names last. Parsed here rather than in the row
    // pattern because an `expr` fragment cannot be followed by `..`.
    (@row_meta [$default:path] { .. $base:path } $($rest:tt)*) => {
        operators!(@meta (OpMeta { ..$base }) $($rest)*)
    };
    (@row_meta [$default:path] { $( $field:ident : $value:expr ),* $(,)? } $($rest:tt)*) => {
        operators!(@meta (OpMeta { $( $field: $value, )* ..$default }) $($rest)*)
    };
    (@row_meta [$default:path] { $( $field:ident : $value:expr , )+ .. $base:path } $($rest:tt)*) => {
        operators!(@meta (OpMeta { $( $field: $value, )+ ..$base }) $($rest)*)
    };
    (@meta ($meta:expr)) => { $meta };
    (@meta ($meta:expr) $alg:ident ($payload:expr)) => {{
        const BASE: OpMeta = $meta;
        assert!(
            BASE.algebra.is_none(),
            "a row with `@ Kind(payload)` must not also set `algebra`"
        );
        OpMeta { algebra: Some(Algebra::$alg($payload)), ..BASE }
    }};

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
    (@is_iter each) => { true };
    (@is_iter $other:ident) => { false };

    (@arity raw) => { Arity::ANY };
    (@arity iter) => { Arity::ANY };
    (@arity raw [ $($r:tt)* ]) => { operators!(@range $($r)*) };
    (@arity iter [ $($r:tt)* ]) => { operators!(@range $($r)*) };
    (@arity each [ $($r:tt)* ]) => { operators!(@range $($r)*) };
    (@range $n:literal) => { Arity::exactly($n) };
    (@range $lo:literal ..) => { Arity::at_least($lo) };
    (@range $lo:literal ..= $hi:literal) => { Arity::between($lo, $hi) };
    (@arity eager ( $($e:ty),* )) => {
        const { Arity::of(&[ $( <$e as extract::Extract<'static>>::SPAN ),* ]) }
    };

    // ── dispatch arm bodies ───────────────────────────────────────────────
    (@call $v:ident raw { } [ $($p:tt)* ] [ $($k:expr),* ] [ $($pl:expr)? ]
        ($args:ident, $iak:ident, $ctx:ident, $engine:ident, $arena:ident)) => {
        $($p)*($args, $ctx, $engine, $arena $(, $k)* $(, $pl)?)
    };
    (@call $v:ident raw { [ $($ar:tt)* ] } [ $($p:tt)* ] [ $($k:expr),* ] [ $($pl:expr)? ]
        ($args:ident, $iak:ident, $ctx:ident, $engine:ident, $arena:ident)) => {
        eager::raw::<{ OpCode::$v as u8 }, _>($args, $ctx, $engine, $arena,
            |$args, $ctx, $engine, $arena| $($p)*($args, $ctx, $engine, $arena $(, $k)* $(, $pl)?))
    };
    (@call $v:ident iter { } [ $($p:tt)* ] [ $($k:expr),* ] [ $($pl:expr)? ]
        ($args:ident, $iak:ident, $ctx:ident, $engine:ident, $arena:ident)) => {
        $($p)*($args, $iak, $ctx, $engine, $arena $(, $k)* $(, $pl)?)
    };
    (@call $v:ident iter { [ $($ar:tt)* ] } [ $($p:tt)* ] [ $($k:expr),* ] [ $($pl:expr)? ]
        ($args:ident, $iak:ident, $ctx:ident, $engine:ident, $arena:ident)) => {
        eager::iter::<{ OpCode::$v as u8 }, _>($args, $iak, $ctx, $engine, $arena,
            |$args, $iak, $ctx, $engine, $arena| $($p)*($args, $iak, $ctx, $engine, $arena $(, $k)* $(, $pl)?))
    };
    (@call $v:ident each { [ $($ar:tt)* ] } [ $($p:tt)* ] [ $($k:expr),* ] [ $($pl:expr)? ]
        ($args:ident, $iak:ident, $ctx:ident, $engine:ident, $arena:ident)) => {
        eager::each::<{ OpCode::$v as u8 }, _>($args, $iak, $ctx, $engine, $arena,
            |items, $args, $ctx, $engine, $arena| $($p)*(items, $args, $ctx, $engine, $arena $(, $k)* $(, $pl)?))
    };
    (@call $v:ident eager { } ( $($a:ty),* ) [ $($p:tt)* ] [ ] [ ]
        ($args:ident, $iak:ident, $ctx:ident, $engine:ident, $arena:ident)) => {
        eager::call::<{ OpCode::$v as u8 }, ( $($a,)* ), _, _>($args, $ctx, $engine, $arena, $($p)*)
    };
    (@call $v:ident eager { } ( $($a:ty),* ) [ $($p:tt)* ] [ ] [ $k:expr ]
        ($args:ident, $iak:ident, $ctx:ident, $engine:ident, $arena:ident)) => {
        eager::call::<{ OpCode::$v as u8 }, ( $($a,)* ), _, _>($args, $ctx, $engine, $arena,
            eager::Bound($($p)*, $k))
    };
    (@call $v:ident eager { } ( $($a:ty),* ) [ $($p:tt)* ] [ $k:expr ] [ ]
        ($args:ident, $iak:ident, $ctx:ident, $engine:ident, $arena:ident)) => {
        eager::call::<{ OpCode::$v as u8 }, ( $($a,)* ), _, _>($args, $ctx, $engine, $arena,
            eager::Bound($($p)*, $k))
    };
    (@call $v:ident $($rest:tt)*) => {
        compile_error!(concat!(
            "operator row `", stringify!($v),
            "`: unknown shape, more than six eager arguments, more than one bound argument, `[arity]` on an eager row, or `each` without `[arity]`"
        ))
    };
}

operators! {
    family Core (all()) = PURE {
        // ── variable access ──────────────────────────────────────────────
        // `var` is accepted as a synonym of `val`; the compile hook reads
        // the source name, because `var`'s second argument is a default
        // and `val`'s is a path segment.
        Val ["val", "var"] => raw variable::evaluate_val {
            reads_context: true,
            compile: Some(CompileHook::Args(hooks::val)),
            literal_args: LiteralArgs::All,
        };
        // `{"var": [path, default]}` with a computed path. Internal: only
        // the `var` compile hook emits it; it renders back as `var`.
        VarDefault [] => raw variable::evaluate_var_default { reads_context: true, display: Some("var") };

        // ── comparison ───────────────────────────────────────────────────
        // The comparisons and `and` / `or` stay `raw`: as `eager` rows
        // (`eager(Any, Any, Rest<Any>)`) they measured 2 to 5% slower on
        // their suites and `macro/eligibility` (phase 2, P6).
        Equals ["=="] => raw[2..] comparison::equals @ Eq(EqOp::LOOSE);
        StrictEquals ["==="] => raw[2..] comparison::equals @ Eq(EqOp::STRICT);
        NotEquals ["!="] => raw[2] comparison::not_equals @ Eq(EqOp::LOOSE_NE);
        StrictNotEquals ["!=="] => raw[2] comparison::not_equals @ Eq(EqOp::STRICT_NE);
        GreaterThan [">"] => raw[2..] comparison::ordered @ Ord(OrdOp::Gt);
        GreaterThanEqual [">="] => raw[2..] comparison::ordered @ Ord(OrdOp::Ge);
        LessThan ["<"] => raw[2..] comparison::ordered @ Ord(OrdOp::Lt);
        LessThanEqual ["<="] => raw[2..] comparison::ordered @ Ord(OrdOp::Le);

        // ── logic and control (lazy) ─────────────────────────────────────
        Not ["!"] => eager(Opt<Truthy>) logical::truth @ Truth(Truth::Not);
        BoolCast ["!!"] => eager(Opt<Truthy>) logical::truth @ Truth(Truth::Bool);
        And ["and"] => raw[1..] logical::short_circuit @ Logic(Logic::And)
            { args_form: ArgsForm::ArrayOnly, on_missing: Miss::Return(singleton_null) };
        Or ["or"] => raw[1..] logical::short_circuit @ Logic(Logic::Or)
            { args_form: ArgsForm::ArrayOnly, on_missing: Miss::Return(singleton_null) };
        // `?:` is accepted as a synonym; the ternary is the 3-argument `if`.
        If ["if", "?:"] => raw[1..] control::evaluate_if
            { args_form: ArgsForm::ArrayOnly, on_missing: Miss::Return(singleton_null) };

        // ── arithmetic ───────────────────────────────────────────────────
        // `+`, `-` and `*` keep separate bodies (their 0-, 1- and n-argument
        // rules differ), so they declare their algebra without binding it.
        // A one-argument `+` / `*` / `max` / `min` rejects a literal array and
        // folds a computed one.
        Add ["+"] => raw arithmetic::evaluate_add
            { algebra: Some(Algebra::Arith(ArithOp::Add)), literal_args: LiteralArgs::Sole };
        Subtract ["-"] => raw[1..] arithmetic::evaluate_subtract
            { algebra: Some(Algebra::Arith(ArithOp::Sub)) };
        Multiply ["*"] => raw arithmetic::evaluate_multiply
            { algebra: Some(Algebra::Arith(ArithOp::Mul)), literal_args: LiteralArgs::Sole };
        Divide ["/"] => raw[1..] arithmetic::div_or_mod @ Div(DivOp::Divide);
        Modulo ["%"] => raw[1..] arithmetic::div_or_mod @ Div(DivOp::Modulo);
        // `max` / `min` / `merge` disambiguate a literal array from an
        // argument list at runtime (`{"max": [[1, 2]]}`), so folding their
        // static form would bake in the wrong reading.
        Max ["max"] => iter[1..] arithmetic::extremum @ Extremum(Extremum::Max)
            { fold: Fold::Never, literal_args: LiteralArgs::Sole };
        Min ["min"] => iter[1..] arithmetic::extremum @ Extremum(Extremum::Min)
            { fold: Fold::Never, literal_args: LiteralArgs::Sole };

        // ── strings ──────────────────────────────────────────────────────
        Concat ["cat"] => eager(Rest<Any>) string::concat { algebra: Some(Algebra::Concat), ..STRING };
        Substr ["substr"] => eager(Str, Lenient<Int>, Lenient<Int>) string::substr
            { on_missing: Miss::Return(singleton_empty_string), ..STRING };
        In ["in"] => eager(Any, Any) string::in_ { on_missing: Miss::Return(singleton_false), ..STRING };

        // ── arrays ───────────────────────────────────────────────────────
        // `merge` measured +3% slower through the eager adapter (phase 2, P6).
        Merge ["merge"] => raw array::evaluate_merge { fold: Fold::Never, cost: Cost::PerItem };
        // A null, missing or empty source: `[]` for `filter` / `map`; for
        // the quantifiers, `all` is deliberately not vacuously true.
        Filter ["filter"] => each[2] array::evaluate_filter
            { on_empty_source: Some(singleton_empty_array), ..ITERATOR };
        Map ["map"] => each[2] array::evaluate_map { on_empty_source: Some(singleton_empty_array), ..ITERATOR };
        // `reduce` evaluates its initial value before its source, and folds
        // `reduce(map(..))` without resolving the source, so it resolves
        // its own.
        Reduce ["reduce"] => iter[2..=3] array::evaluate_reduce { ..ITERATOR };
        All ["all"] => each[2] array::quantifier @ Quant(Quant::All)
            { on_empty_source: Some(singleton_false), ..ITERATOR };
        Some ["some"] => each[2] array::quantifier @ Quant(Quant::Some)
            { on_empty_source: Some(singleton_false), ..ITERATOR };
        None ["none"] => each[2] array::quantifier @ Quant(Quant::None)
            { on_empty_source: Some(singleton_true), ..ITERATOR };

        // ── missing values ───────────────────────────────────────────────
        Missing ["missing"] => eager(Rest<Any>) missing::missing
            { reads_context: true, compile: Some(CompileHook::Args(hooks::missing)) };
        MissingSome ["missing_some"] => eager(Any, Any) missing::missing_some {
            reads_context: true,
            on_missing: Miss::Return(singleton_empty_array),
            compile: Some(CompileHook::Args(hooks::missing_some)),
        };
    }

    family DateTime (feature = "datetime") = PURE {
        Datetime ["datetime"] => eager(Any) datetime::datetime
            { on_missing: Miss::Err("datetime requires an argument") };
        Timestamp ["timestamp"] => eager(Any) datetime::timestamp
            { on_missing: Miss::Err("timestamp requires an argument") };
        ParseDate ["parse_date"] => eager(Any, Any, Lazy) datetime::parse_date {
            on_missing: Miss::Err("parse_date requires date string and format"),
            compile: Some(CompileHook::Args(hooks::timezone_literal)),
        };
        FormatDate ["format_date"] => eager(Any, Any, Lazy) datetime::format_date {
            on_missing: Miss::Err("format_date requires datetime and format"),
            compile: Some(CompileHook::Args(hooks::timezone_literal)),
        };
        DateDiff ["date_diff"] => eager(Any, Any, Any) datetime::date_diff
            { on_missing: Miss::Err("date_diff requires two dates and a unit") };
        Now ["now"] => eager() datetime::now { effect: Effect::Clock };
    }

    family ExtString (feature = "ext-string") = STRING {
        Length ["length"] => eager(Any) array::length { cost: Cost::Node, on_extra: Extra::InvalidArgs };
        StartsWith ["starts_with"] => eager(Str, Str) string::starts_with;
        EndsWith ["ends_with"] => eager(Str, Str) string::ends_with;
        Upper ["upper"] => eager(Str) string::upper;
        Lower ["lower"] => eager(Str) string::lower;
        Trim ["trim"] => eager(Str) string::trim;
        Split ["split"] => eager(Str, Str) string::split;
    }

    // The extension iterators ignore extra arguments, unlike the JSONLogic
    // ones (`ITERATOR` rejects them).
    family ExtArray (feature = "ext-array") = ITERATOR {
        // `sort`'s `args[1]` is the scalar direction flag; the key
        // expression at `args[2]` runs per element. It resolves its own
        // source: a null source gives `null`, an empty array `[]`, and a
        // literal `null` is an error.
        Sort ["sort"] => iter[1..=3] array::evaluate_sort {
            frames: Frames::At(2),
            cost: Cost::NLogN,
            on_extra: Extra::Ignore,
            literal_args: LiteralArgs::At(0),
        };
        // Stays `raw`: a null collection returns null without evaluating its
        // bounds, which an `eager` row (every argument first) would not.
        Slice ["slice"] => raw[1..=4] array::evaluate_slice { cost: Cost::PerItem, ..PURE };
        GroupBy ["group_by"] => each[2] array::evaluate_group_by
            { on_extra: Extra::Ignore, on_empty_source: Some(singleton_empty_array) };
        // Without a key expression nothing runs under a frame, so
        // `distinct` folds like any pure operator.
        Distinct ["distinct"] => each[1..=2] array::evaluate_distinct {
            cost: Cost::Quadratic,
            on_extra: Extra::Ignore,
            on_empty_source: Some(singleton_empty_array),
        };
    }

    family ExtObject (feature = "ext-object") = PURE {
        Keys ["keys"] => eager(Nullable<Obj>) object::keys { cost: Cost::PerItem };
        Values ["values"] => eager(Nullable<Obj>) object::values { cost: Cost::PerItem };
        Entries ["entries"] => eager(Nullable<Obj>) object::entries { cost: Cost::PerItem };
    }

    family ExtControl (feature = "ext-control") = PURE {
        // No path names the current data itself, which always exists (the
        // compile hook resolves `{"exists": []}` that way before this row's
        // gate is reached).
        Exists ["exists"] => eager(Any, Rest<Any>) variable::exists {
            reads_context: true,
            on_missing: Miss::Return(singleton_true),
            compile: Some(CompileHook::Args(hooks::exists)),
        };
        Coalesce ["??"] => eager(Rest<Any>) control::coalesce;
        // Only a literal case table of literal pairs is read.
        Switch ["switch", "match"] => raw[2..=3] control::evaluate_switch
            { on_missing: Miss::Return(singleton_null), literal_args: LiteralArgs::At(1) };
        Type ["type"] => eager(Any) inspect::type_ { on_missing: Miss::Return(inspect::type_of_nothing) };
    }

    family ErrorHandling (feature = "error-handling") = PURE {
        Try ["try"] => raw[1..] error_handling::evaluate_try {
            effect: Effect::Catches,
            frames: Frames::LastIfMulti,
            on_missing: Miss::Return(singleton_null),
        };
        Throw ["throw"] => raw error_handling::evaluate_throw
            { effect: Effect::Throws, compile: Some(CompileHook::Args(hooks::throw_literal)) };
    }

    family ExtMath (feature = "ext-math") = PURE {
        Abs ["abs"] => eager(StrictNum, Rest<StrictNum>) arithmetic::unary_math(UnaryMathOp::Abs);
        Ceil ["ceil"] => eager(StrictNum, Rest<StrictNum>) arithmetic::unary_math(UnaryMathOp::Ceil);
        Floor ["floor"] => eager(StrictNum, Rest<StrictNum>) arithmetic::unary_math(UnaryMathOp::Floor);
    }

    family Tensor (feature = "tensor") = TENSOR {
        // Variant names carry a `Tensor` prefix so they never collide with
        // an existing opcode (`Concat` is `cat`, `Type` is `type`).
        TensorMake ["tensor"] => eager(Any, Lazy) tensor::tensor
            { compile: Some(CompileHook::Raw(hooks::tensor_wire_body)) };
        TensorZeros ["zeros"] => eager(ShapeArg, DTypeArg) tensor::zeros;
        TensorFull ["full"] => eager(ShapeArg, DTypeArg, Any) tensor::full;
        TensorScatter ["scatter"] => eager(Any, ShapeArg, DTypeArg, Opt<Any>) tensor::scatter;
        TensorRleExpand ["rle_expand"] => eager(Any, ShapeArg, DTypeArg) tensor::rle_expand;
        TensorOneHot ["one_hot"] => eager(Any, UsizeArg, DTypeArg) tensor::one_hot;
        TensorStack ["stack"] => eager(Any, I64Arg) tensor::stack;
        TensorConcat ["concat"] => eager(Any, I64Arg) tensor::concat;
        TensorUnstack ["unstack"] => eager(TensorArg, I64Arg) tensor::unstack;
        // `reshape` shares the input's payload and `shape` / `dtype` read the
        // header, so each charges a flat 1 rather than per element.
        TensorReshape ["reshape"] => eager(TensorArg, ShapeArg) tensor::reshape { cost: Cost::Node };
        TensorTranspose ["transpose"] => eager(TensorArg, Opt<ShapeArg>) tensor::transpose;
        TensorPad ["pad"] => eager(TensorArg, ShapeArg, ShapeArg, Opt<Any>) tensor::pad;
        TensorCrop ["crop"] => eager(TensorArg, ShapeArg, ShapeArg) tensor::crop;
        TensorCast ["cast"] => eager(TensorArg, DTypeArg) tensor::cast;
        TensorNormalize ["normalize"] => eager(TensorArg, F64Arg, Opt<F64Arg>) tensor::normalize;
        TensorArgmax ["argmax"] => eager(TensorArg, I64Arg) tensor::argmax;
        TensorGather ["gather"] => eager(TensorArg, I64List, Opt<I64Arg>) tensor::gather;
        TensorToList ["to_list"] => eager(TensorArg) tensor::to_list;
        TensorShape ["shape"] => eager(TensorArg) tensor::shape { cost: Cost::Node };
        TensorDtype ["dtype"] => eager(TensorArg) tensor::dtype { cost: Cost::Node };
    }

    family Flagd (feature = "flagd") = PURE {
        // Without a bucketing expression, `fractional` reads
        // `$flagd.flagKey` and `targetingKey` from the root data.
        // Never memoised: CSE was introduced with an explicit exclusion
        // list (fractional, sem_ver, now, try, throw) and no soundness
        // argument was made for the flagd pair; a flag rule calls each
        // once, so there is nothing to gain from relaxing it.
        // Stays `raw`: as `eager(Any, Rest<Any>)` it measured 9% slower on
        // `flagd/fractional.json` (phase 2, P6).
        Fractional ["fractional"] => raw[1..] flagd::evaluate_fractional
            { reads_context: true, cse: Cse::Never, on_missing: Miss::Return(singleton_null) };
        // Pure given its arguments, so a fully literal call folds. Never
        // memoised, for the same reason as `fractional`.
        SemVer ["sem_ver"] => eager(Any, Any, Any) flagd::sem_ver {
            on_missing: Miss::Return(singleton_null),
            on_extra: Extra::Return(singleton_null),
            cse: Cse::Never,
        };
    }
}

impl OpCode {
    /// The row whose `algebra` is `algebra`, if one is compiled in. For
    /// rewrites that produce an operator by what it computes (`!(!x)`
    /// becomes whichever row computes `Truth::Bool`). A linear scan, so for
    /// the compile-time passes only.
    pub(crate) fn with_algebra(algebra: Algebra) -> Option<OpCode> {
        OpCode::ALL
            .iter()
            .copied()
            .find(|op| op.algebra() == Some(algebra))
    }
}

/// Every name [`OpCode`]'s `from_str` accepts in this build: canonical
/// names and their aliases, in table order. Derived from the table, so it
/// cannot drift from dispatch.
#[cfg(test)]
pub(crate) fn builtin_operator_names() -> impl Iterator<Item = &'static str> {
    NAMES.iter().copied()
}

/// [`builtin_operator_names`] of the families in `families` (a set of
/// [`Family::bit`]s).
pub(crate) fn builtin_operator_names_in(families: u32) -> impl Iterator<Item = &'static str> {
    NAMES.iter().copied().filter(move |name| {
        name.parse::<OpCode>()
            .is_ok_and(|op| op.in_families(families))
    })
}
