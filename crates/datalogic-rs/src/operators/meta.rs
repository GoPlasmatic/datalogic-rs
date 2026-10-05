//! Declared facts about each built-in operator, and the rules that derive
//! every optimizer classification from them.
//!
//! Each row of the operator table ([`super::table`]) carries one [`OpMeta`]:
//! its family's preset (`PURE`, `STRING`, `ITERATOR`, `TENSOR`), with the
//! fields the row writes in braces overriding it
//! (`{ cost: Cost::Node, on_extra: Extra::InvalidArgs }`). The fields are *declared* facts
//! (does it read the context? push a frame? have an effect?). Nothing here
//! is a per-operator list: the scope pass, the CSE pass and constant folding
//! all ask the derivation methods on [`OpMeta`], so a new row is classified
//! by what it declares and nothing else.
//!
//! Derive by default, override explicitly. Where an operator deliberately
//! differs from what its facts would derive (`merge`/`min`/`max` never fold,
//! `fractional` and `sem_ver` are never memoised, the tensor family opts
//! out of both), the row carries the override with the reason next to it.

// Which variants, extractors and adapters a build uses depends on which
// operator families it compiles in; with every family on, all of them are
// used and dead code is still reported.
#![cfg_attr(not(feature = "all-operators"), allow(dead_code, unused_imports))]

use datavalue::OwnedDataValue;

use crate::arena::DataValue;
use crate::node::{CompileCtx, CompiledNode, node_is_static};

/// The facts a table row declares about its operator.
#[derive(Clone, Copy)]
pub(crate) struct OpMeta {
    /// Which argument spellings the operator accepts.
    pub args_form: ArgsForm,
    /// Reads the data context (`val`, `missing`, `exists`, `fractional`).
    /// A context reader is never folded.
    pub reads_context: bool,
    /// What evaluating the operator does besides computing a value.
    pub effect: Effect,
    /// Which argument positions run under a pushed context frame.
    pub frames: Frames,
    /// Override for constant folding.
    pub fold: Fold,
    /// Override for common-subexpression memoisation.
    pub cse: Cse,
    /// What the operator's work is proportional to. Documentation for the
    /// budget audit: the operator body does its own charging.
    pub cost: Cost,
    /// Rows with a declared arity (`eager(..)`, `raw[..]`, `iter[..]`): what
    /// happens when a required argument is absent.
    pub on_missing: Miss,
    /// Rows with a declared maximum: what happens when there are more
    /// arguments than the row reads.
    pub on_extra: Extra,
    /// `each` rows: the result for a null, missing or empty-array source,
    /// returned without running the body.
    pub on_empty_source: Option<Singleton>,
    /// What the operator computes, for the fast paths that specialise on it
    /// (`map` / `reduce` arithmetic bodies, `filter` comparisons, constant
    /// folding's associative set).
    pub algebra: Option<Algebra>,
    /// A specialised compiled form, built instead of the generic
    /// `BuiltinOperator` node when it applies.
    pub compile: Option<CompileHook>,
    /// Render name for an internal opcode that no operator name maps to.
    pub display: Option<&'static str>,
}

/// Which argument spellings the operator accepts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ArgsForm {
    /// An array is the argument list; anything else is a single argument.
    Any,
    /// Only an argument array. Anything else compiles to an
    /// `InvalidArgs` node that raises "Invalid Arguments" at evaluation
    /// (`and`, `or`, `if`).
    ArrayOnly,
}

/// What evaluating an operator does besides computing a value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Effect {
    /// A function of its arguments (and, if `reads_context`, the data).
    Pure,
    /// Reads the wall clock (`now`).
    Clock,
    /// Raises a user error (`throw`).
    Throws,
    /// Recovers from errors raised by its arguments (`try`).
    Catches,
}

/// Which argument positions run under a pushed context frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Frames {
    /// No argument runs under a pushed frame.
    None,
    /// The argument at this index runs once per element, under a frame
    /// (an iterator body or key expression).
    At(u8),
    /// The last argument of a call with two or more runs under the
    /// caught-error frame (`try`'s catch arm).
    LastIfMulti,
}

/// Override for constant folding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Fold {
    /// Fold when [`OpMeta::can_fold`] derives it.
    Default,
    /// Never fold, even with static arguments.
    Never,
}

/// Override for common-subexpression memoisation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Cse {
    /// Memoise when [`OpMeta::cse_pure`] derives it.
    Default,
    /// Never memoise.
    Never,
}

/// What an operator's work is proportional to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Cost {
    /// Constant per call; the dispatcher's one unit covers it.
    Node,
    /// The bytes of its string arguments.
    Bytes,
    /// The items of its collection argument.
    PerItem,
    /// `n log n` in the items (`sort`).
    NLogN,
    /// Quadratic in the items (`distinct`'s pairwise equality).
    Quadratic,
    /// The tensor elements it reads or produces.
    Elements,
}

impl Cost {
    /// Lower-case name used in the operator catalogue.
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Cost::Node => "node",
            Cost::Bytes => "bytes",
            Cost::PerItem => "per_item",
            Cost::NLogN => "n_log_n",
            Cost::Quadratic => "quadratic",
            Cost::Elements => "elements",
        }
    }
}

impl Effect {
    /// Lower-case name used in the operator catalogue.
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Effect::Pure => "pure",
            Effect::Clock => "clock",
            Effect::Throws => "throws",
            Effect::Catches => "catches",
        }
    }
}

/// A static singleton returned in place of evaluating.
pub(crate) type Singleton = fn() -> &'static DataValue<'static>;

/// What happens when a required argument is absent.
#[derive(Clone, Copy)]
pub(crate) enum Miss {
    /// `InvalidArguments("Invalid Arguments")`.
    InvalidArgs,
    /// `InvalidArguments(msg)`. The message is the serialised error `type`,
    /// so it is part of the observable contract.
    Err(&'static str),
    /// Return this value without evaluating anything.
    Return(Singleton),
}

/// What happens when there are more arguments than the row reads.
#[derive(Clone, Copy)]
pub(crate) enum Extra {
    /// Extra arguments are never evaluated.
    Ignore,
    /// `InvalidArguments("Invalid Arguments")`.
    InvalidArgs,
    /// `InvalidArguments(msg)`.
    Err(&'static str),
    /// Return this value without evaluating anything.
    Return(Singleton),
}

/// What an operator computes.
///
/// Read by the fast paths that specialise on it (`map` / `reduce`
/// arithmetic bodies, `filter` comparisons, constant folding's associative
/// set), and, for a row written `@ Kind(payload)`, passed to the row's body
/// as its last argument, so one body serves every row of the kind.
///
/// The payload types live here as a shared vocabulary; behaviour that only
/// one operator module needs is an inherent `impl` in that module.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Algebra {
    /// `+`, `-`, `*`.
    Arith(ArithOp),
    /// `/`, `%`.
    Div(DivOp),
    /// `>`, `>=`, `<`, `<=`.
    Ord(OrdOp),
    /// `==`, `===`, `!=`, `!==`.
    Eq(EqOp),
    /// `and`, `or`.
    Logic(Logic),
    /// `max`, `min`.
    Extremum(Extremum),
    /// `all`, `some`, `none`.
    Quant(Quant),
    /// `!`, `!!`.
    Truth(Truth),
    /// `cat`: string concatenation, associative, so adjacent literal
    /// arguments may be joined at compile time.
    Concat,
}

/// An equality comparison.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct EqOp {
    /// `===` / `!==`: no type coercion.
    pub strict: bool,
    /// `!=` / `!==`: the negated result.
    pub negate: bool,
}

impl EqOp {
    /// `==`.
    pub(crate) const LOOSE: EqOp = EqOp {
        strict: false,
        negate: false,
    };
    /// `===`.
    pub(crate) const STRICT: EqOp = EqOp {
        strict: true,
        negate: false,
    };
    /// `!=`.
    pub(crate) const LOOSE_NE: EqOp = EqOp {
        strict: false,
        negate: true,
    };
    /// `!==`.
    pub(crate) const STRICT_NE: EqOp = EqOp {
        strict: true,
        negate: true,
    };
}

/// `/` or `%`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DivOp {
    Divide,
    Modulo,
}

/// A short-circuiting boolean chain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Logic {
    And,
    Or,
}

impl Logic {
    /// The truthiness that ends the chain and becomes its result: falsy
    /// for `and`, truthy for `or`.
    #[inline(always)]
    pub(crate) const fn absorbing(self) -> bool {
        matches!(self, Logic::Or)
    }
}

/// A truthiness operator: `!` (negated) or `!!` (as is).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Truth {
    Not,
    Bool,
}

impl Truth {
    /// The argument's truthiness, negated for `!`.
    #[inline(always)]
    pub(crate) const fn apply(self, truthy: bool) -> bool {
        match self {
            Truth::Not => !truthy,
            Truth::Bool => truthy,
        }
    }

    /// `self(inner(x))` as one operator: it negates when exactly one of
    /// the two does (`!(!x)` is `!!x`, `!(!!x)` is `!x`).
    #[inline]
    pub(crate) const fn compose(self, inner: Truth) -> Truth {
        if matches!(self, Truth::Not) != matches!(inner, Truth::Not) {
            Truth::Not
        } else {
            Truth::Bool
        }
    }
}

/// `max` or `min`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Extremum {
    Max,
    Min,
}

/// A quantifier over a collection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Quant {
    All,
    Some,
    None,
}

/// A binary arithmetic operation with an exact integer form.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ArithOp {
    Add,
    Sub,
    Mul,
}

impl ArithOp {
    /// The integer result, or `None` on overflow (the caller promotes to
    /// `f64`).
    #[inline(always)]
    pub(crate) fn checked_i64(self, a: i64, b: i64) -> Option<i64> {
        match self {
            ArithOp::Add => a.checked_add(b),
            ArithOp::Sub => a.checked_sub(b),
            ArithOp::Mul => a.checked_mul(b),
        }
    }

    /// The floating-point result.
    #[inline(always)]
    pub(crate) fn apply_f64(self, a: f64, b: f64) -> f64 {
        match self {
            ArithOp::Add => a + b,
            ArithOp::Sub => a - b,
            ArithOp::Mul => a * b,
        }
    }

    /// Whether static arguments may be regrouped and folded together
    /// (`{"+": [1, x, 2]}` → `{"+": [3, x]}`).
    #[inline]
    pub(crate) const fn is_associative(self) -> bool {
        matches!(self, ArithOp::Add | ArithOp::Mul)
    }

    /// The `e` with `x op e == x`: the start of a fold over no operands.
    #[inline(always)]
    pub(crate) const fn right_identity(self) -> i64 {
        match self {
            ArithOp::Add | ArithOp::Sub => 0,
            ArithOp::Mul => 1,
        }
    }
}

/// An ordering comparison.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OrdOp {
    Gt,
    Ge,
    Lt,
    Le,
}

impl OrdOp {
    /// Whether `a op b` holds.
    #[inline(always)]
    pub(crate) fn holds<T: PartialOrd + ?Sized>(self, a: &T, b: &T) -> bool {
        match self {
            OrdOp::Gt => a > b,
            OrdOp::Ge => a >= b,
            OrdOp::Lt => a < b,
            OrdOp::Le => a <= b,
        }
    }

    /// Apply the comparison to two `f64`s.
    #[inline(always)]
    pub(crate) fn cmp_f64(self, a: f64, b: f64) -> bool {
        match self {
            OrdOp::Gt => a > b,
            OrdOp::Ge => a >= b,
            OrdOp::Lt => a < b,
            OrdOp::Le => a <= b,
        }
    }
}

/// What a compile hook was handed, for the hooks that run on compiled
/// arguments.
pub(crate) struct HookArgs<'h> {
    pub opcode: crate::OpCode,
    /// The operator name as written in the rule (`var` and `val` share an
    /// opcode but not an argument shape).
    pub op_name: &'h str,
    /// The raw, uncompiled argument value.
    pub args_value: &'h OwnedDataValue,
    pub ctx: &'h mut CompileCtx,
}

/// A compile hook's answer.
pub(crate) enum Hooked {
    /// The specialised node replaces the generic one.
    Node(CompiledNode),
    /// The hook does not apply: the arguments go back to the generic path.
    Generic(Box<[CompiledNode]>),
}

/// A specialised compiled form for one operator.
#[derive(Clone, Copy)]
pub(crate) enum CompileHook {
    /// Runs on the raw argument value, before any argument is compiled.
    /// For forms whose arguments would not compile as a rule
    /// (`tensor`'s wire body).
    Raw(fn(&OwnedDataValue, &mut CompileCtx) -> Option<CompiledNode>),
    /// Runs on the compiled arguments.
    Args(fn(Box<[CompiledNode]>, HookArgs<'_>) -> Hooked),
}

/// The default row: a pure function of its arguments.
pub(crate) const PURE: OpMeta = OpMeta {
    args_form: ArgsForm::Any,
    reads_context: false,
    effect: Effect::Pure,
    frames: Frames::None,
    fold: Fold::Default,
    cse: Cse::Default,
    cost: Cost::Node,
    on_missing: Miss::InvalidArgs,
    on_extra: Extra::Ignore,
    on_empty_source: None,
    algebra: None,
    compile: None,
    display: None,
};

/// A string operator: its work is proportional to the bytes it reads.
pub(crate) const STRING: OpMeta = OpMeta {
    cost: Cost::Bytes,
    ..PURE
};

/// An iterator: `args[0]` is the source and `args[1]` the per-element
/// body, run under a pushed frame. The JSONLogic iterators reject extra
/// arguments; the extension ones (`sort`, `group_by`, `distinct`) override
/// that with [`Extra::Ignore`].
pub(crate) const ITERATOR: OpMeta = OpMeta {
    frames: Frames::At(1),
    cost: Cost::PerItem,
    on_extra: Extra::InvalidArgs,
    ..PURE
};

/// The tensor family. Pure, but never folded and never memoised: folding
/// `zeros([128,128], "f32")` would bake a 64 KB literal into the `Logic`,
/// a memo would pin a large buffer for the whole evaluation, and folded or
/// memoised work is never charged, so a budgeted operation count would
/// depend on which subtrees the optimizer happened to recognise. Every
/// operator takes a fixed positional argument list and rejects extras.
pub(crate) const TENSOR: OpMeta = OpMeta {
    fold: Fold::Never,
    cse: Cse::Never,
    cost: Cost::Elements,
    on_missing: Miss::Err("missing argument"),
    on_extra: Extra::Err("too many arguments"),
    ..PURE
};

impl OpMeta {
    /// Whether a call with these arguments may be evaluated once at compile
    /// time and replaced by its result.
    ///
    /// Only the children that are actually present count: `distinct`
    /// without a key expression has no child at index 1, so nothing runs
    /// under a frame and it folds like any pure operator.
    pub(crate) fn can_fold(&self, args: &[CompiledNode]) -> bool {
        !self.reads_context
            && self.effect == Effect::Pure
            && self.fold == Fold::Default
            && (0..args.len()).all(|i| self.frames_for(i, args.len()) == 0)
            && args.iter().all(node_is_static)
    }

    /// Whether the operator itself may be served from the CSE memo
    /// (its arguments are checked separately).
    pub(crate) fn cse_pure(&self) -> bool {
        self.effect == Effect::Pure && self.cse == Cse::Default
    }

    /// Whether the operator runs a per-element body (an iterator).
    /// `try`'s catch frame does not count: it runs once.
    pub(crate) fn is_iterator(&self) -> bool {
        matches!(self.frames, Frames::At(_))
    }

    /// Number of context frames pushed around child `index` of a call with
    /// `len` arguments.
    pub(crate) fn frames_for(&self, index: usize, len: usize) -> u32 {
        match self.frames {
            Frames::None => 0,
            Frames::At(at) => u32::from(index == at as usize),
            Frames::LastIfMulti => u32::from(len >= 2 && index == len - 1),
        }
    }
}
