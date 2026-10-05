//! Row adapters: typed rows for fixed-arity operators, the arity gate for
//! `raw[..]` / `iter[..]` rows, and source resolution for `each[..]` rows.
//!
//! A table row such as `eager(Str, Str) string::ends_with` declares the
//! operator's signature as a list of argument extractors. The body is a
//! plain function over the extracted values:
//!
//! ```ignore
//! pub(crate) fn ends_with<'a>(cx: &mut Cx<'_, 'a>, text: &'a str, suffix: &'a str) -> Result<bool>
//! ```
//!
//! and the dispatch arm calls the generic [`call`] adapter below with the
//! extractors as a tuple ([`ExtractList`]) and the function as its
//! [`Body`]. The adapter does the plumbing every such operator used to
//! write by hand (the extractors and result conversions live in
//! [`super::extract`]):
//!
//! 1. **Arity.** The row's `(min, max)` comes from the extractors' spans
//!    ([`Arity::of`]). Too few arguments applies the row's
//!    [`Miss`] policy, too many its [`Extra`] policy, both before anything
//!    is evaluated.
//! 2. **Evaluate, then coerce.** Every present argument is evaluated, in
//!    order, before any is coerced, so a coercion failure on argument 0
//!    never beats an error raised while evaluating argument 1.
//! 3. **Convert the result** through [`IntoValue`].
//!
//! The adapters are generic functions, monomorphised per row, so each
//! row's policy `match` folds against its constant metadata once inlined.

// Which variants, extractors and adapters a build uses depends on which
// operator families it compiles in; with every family on, all of them are
// used and dead code is still reported.
#![cfg_attr(not(feature = "all-operators"), allow(dead_code, unused_imports))]

use bumpalo::Bump;

use super::extract::{Extract, IntoValue};
use super::meta::{Extra, Miss, OpMeta, Singleton};
use crate::arena::{ContextStack, DataValue};
use crate::operators::array::{Items, IterArgKind, ResolvedInput, resolve_iter_input};
use crate::{CompiledNode, Engine, Error, OpCode, Result};

// ---------------------------------------------------------------------------
// The evaluation context an operator body sees
// ---------------------------------------------------------------------------

/// What an operator body works with: the context stack, the engine and
/// the arena, bundled into one value. Built on the stack by the adapter,
/// so with the body inlined it costs nothing.
pub(crate) struct Cx<'c, 'a> {
    pub(crate) ctx: &'c mut ContextStack<'a>,
    pub(crate) engine: &'c Engine,
    pub(crate) arena: &'a Bump,
}

impl<'c, 'a> Cx<'c, 'a> {
    #[inline(always)]
    pub(crate) fn new(ctx: &'c mut ContextStack<'a>, engine: &'c Engine, arena: &'a Bump) -> Self {
        Cx { ctx, engine, arena }
    }

    /// Evaluate a child node.
    #[inline(always)]
    pub(crate) fn eval(&mut self, node: &'a CompiledNode) -> Result<&'a DataValue<'a>> {
        self.engine.dispatch_node(node, self.ctx, self.arena)
    }

    /// Charge `n` operations against the evaluation budget.
    #[inline(always)]
    pub(crate) fn charge(&mut self, n: u64) -> Result<()> {
        self.ctx.charge(n)
    }

    /// Charge the work of scanning `bytes` bytes.
    #[inline(always)]
    pub(crate) fn charge_bytes(&mut self, bytes: usize) -> Result<()> {
        self.ctx.charge_bytes(bytes)
    }

    /// Move a value into the arena.
    #[inline(always)]
    pub(crate) fn alloc<T>(&self, value: T) -> &'a mut T {
        self.arena.alloc(value)
    }

    /// Copy a string into the arena.
    #[inline(always)]
    pub(crate) fn alloc_str(&self, s: &str) -> &'a str {
        self.arena.alloc_str(s)
    }
}

// ---------------------------------------------------------------------------
// Arity
// ---------------------------------------------------------------------------

/// How many arguments an operator reads: at least `min`, at most `max`
/// (`None` = unbounded).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Arity {
    pub min: u8,
    pub max: Option<u8>,
}

impl Arity {
    /// Any number of arguments: a bare `raw` or `iter` row, whose body
    /// takes every count (it may still branch on it).
    pub(crate) const ANY: Arity = Arity { min: 0, max: None };

    /// Exactly `n` (`raw[n]`).
    pub(crate) const fn exactly(n: u8) -> Arity {
        Arity {
            min: n,
            max: Some(n),
        }
    }

    /// `min` or more (`raw[min..]`).
    pub(crate) const fn at_least(min: u8) -> Arity {
        Arity { min, max: None }
    }

    /// `min` to `max` inclusive (`raw[min..=max]`).
    pub(crate) const fn between(min: u8, max: u8) -> Arity {
        assert!(min <= max, "an empty arity range");
        Arity {
            min,
            max: Some(max),
        }
    }

    /// Sum the spans of an `eager` row's extractors. Optional extractors
    /// must come after the required ones, and an unbounded one
    /// ([`super::extract::Rest`]) last, so that a call's argument count
    /// alone decides which positions are present; a row that breaks this
    /// fails to compile.
    pub(crate) const fn of(spans: &[(u8, Option<u8>)]) -> Arity {
        let mut min = 0u8;
        let mut max = Some(0u8);
        let mut optional_seen = false;
        let mut i = 0;
        while i < spans.len() {
            let (lo, hi) = spans[i];
            if lo == 0 {
                optional_seen = true;
            } else if optional_seen {
                panic!("a required extractor follows an optional one");
            }
            min += lo;
            max = match (max, hi) {
                (Some(max), Some(hi)) => Some(max + hi),
                (Some(_), None) if i + 1 == spans.len() => None,
                (Some(_), None) => panic!("an unbounded extractor must be the last one"),
                (None, _) => unreachable!(),
            };
            i += 1;
        }
        Arity { min, max }
    }
}

// ---------------------------------------------------------------------------
// Adapters
// ---------------------------------------------------------------------------

/// Apply the row's arity policy. `Ok(Some(v))` short-circuits with `v`
/// (a [`Miss::Return`] / [`Extra::Return`] row).
#[inline(always)]
fn check_arity(
    len: usize,
    arity: Arity,
    meta: &OpMeta,
) -> Result<Option<&'static DataValue<'static>>> {
    if len < arity.min as usize {
        return match meta.on_missing {
            Miss::InvalidArgs => Err(Error::invalid_args()),
            Miss::Err(msg) => Err(Error::invalid_arguments(msg)),
            Miss::Return(value) => Ok(Some(value())),
        };
    }
    if let Some(max) = arity.max
        && len > max as usize
    {
        return match meta.on_extra {
            Extra::Ignore => Ok(None),
            Extra::InvalidArgs => Err(Error::invalid_args()),
            Extra::Err(msg) => Err(Error::invalid_arguments(msg)),
            Extra::Return(value) => Ok(Some(value())),
        };
    }
    Ok(None)
}

/// An `eager` row's extractors as a tuple, `(Str, Lenient<Int>)`: fetch
/// every argument, in order, then coerce every one, so a coercion failure on
/// argument 0 never beats an error raised while evaluating argument 1.
pub(crate) trait ExtractList<'a> {
    /// Every position's fetched form.
    type Raw;
    /// What the body receives, one element per position.
    type Out;
    fn fetch(args: &'a [CompiledNode], cx: &mut Cx<'_, 'a>) -> Result<Self::Raw>;
    fn coerce(raw: Self::Raw, cx: &mut Cx<'_, 'a>) -> Result<Self::Out>;
}

/// An `eager` row's body for extractors `L`: any
/// `fn(&mut Cx, L0::Out, L1::Out, ...) -> Result<R>`, or one with a bound
/// trailing argument ([`Bound`]).
pub(crate) trait Body<'a, L: ExtractList<'a>, R> {
    fn call(self, cx: &mut Cx<'_, 'a>, args: L::Out) -> Result<R>;
}

/// A body with one bound trailing argument: the row
/// `eager(StrictNum, Rest<StrictNum>) arithmetic::unary_math(UnaryMathOp::Abs)`
/// calls `unary_math(cx, v0, v1, UnaryMathOp::Abs)`. A struct rather than a
/// closure, so nothing depends on LLVM inlining one.
pub(crate) struct Bound<F, K>(pub(crate) F, pub(crate) K);

/// Implement [`ExtractList`] and [`Body`] for one tuple arity.
macro_rules! arity_impls {
    ($($A:ident $raw:ident $val:ident $i:tt),*) => {
        impl<'a, $($A: Extract<'a>),*> ExtractList<'a> for ($($A,)*) {
            type Raw = ($($A::Raw,)*);
            type Out = ($($A::Out,)*);
            #[inline(always)]
            #[allow(unused_variables)]
            fn fetch(args: &'a [CompiledNode], cx: &mut Cx<'_, 'a>) -> Result<Self::Raw> {
                Ok(($($A::fetch(args, $i, cx)?,)*))
            }
            #[inline(always)]
            #[allow(unused_variables)]
            fn coerce(raw: Self::Raw, cx: &mut Cx<'_, 'a>) -> Result<Self::Out> {
                let ($($raw,)*) = raw;
                Ok(($($A::coerce($raw, cx)?,)*))
            }
        }

        impl<'a, F, R, $($A: Extract<'a>),*> Body<'a, ($($A,)*), R> for F
        where
            F: for<'c> FnOnce(&mut Cx<'c, 'a>, $($A::Out),*) -> Result<R>,
        {
            #[inline(always)]
            fn call(self, cx: &mut Cx<'_, 'a>, args: <($($A,)*) as ExtractList<'a>>::Out) -> Result<R> {
                let ($($val,)*) = args;
                self(cx, $($val),*)
            }
        }

        impl<'a, F, K, R, $($A: Extract<'a>),*> Body<'a, ($($A,)*), R> for Bound<F, K>
        where
            F: for<'c> FnOnce(&mut Cx<'c, 'a>, $($A::Out,)* K) -> Result<R>,
        {
            #[inline(always)]
            fn call(self, cx: &mut Cx<'_, 'a>, args: <($($A,)*) as ExtractList<'a>>::Out) -> Result<R> {
                let ($($val,)*) = args;
                (self.0)(cx, $($val,)* self.1)
            }
        }
    };
}

arity_impls!();
arity_impls!(A0 r0 v0 0);
arity_impls!(A0 r0 v0 0, A1 r1 v1 1);
arity_impls!(A0 r0 v0 0, A1 r1 v1 1, A2 r2 v2 2);
arity_impls!(A0 r0 v0 0, A1 r1 v1 1, A2 r2 v2 2, A3 r3 v3 3);
arity_impls!(A0 r0 v0 0, A1 r1 v1 1, A2 r2 v2 2, A3 r3 v3 3, A4 r4 v4 4);
arity_impls!(A0 r0 v0 0, A1 r1 v1 1, A2 r2 v2 2, A3 r3 v3 3, A4 r4 v4 4, A5 r5 v5 5);

/// The adapter for an `eager(..)` row: apply the row's arity policy,
/// fetch and coerce its arguments through `L`, run the body, and convert
/// the result.
///
/// `OP` is the row's opcode discriminant. The row's metadata and arity are
/// read through it in `const` blocks, so each adapter's policy checks are
/// compile-time constants whether or not the adapter is inlined.
#[inline]
pub(crate) fn call<'a, const OP: u8, L, R, B>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
    body: B,
) -> Result<&'a DataValue<'a>>
where
    L: ExtractList<'a>,
    R: IntoValue<'a>,
    B: Body<'a, L, R>,
{
    let meta: &'static OpMeta = const { OpCode::ALL[OP as usize].meta() };
    let arity: Arity = const { OpCode::ALL[OP as usize].arity() };
    if let Some(early) = check_arity(args.len(), arity, meta)? {
        return Ok(early);
    }
    let mut cx = Cx::new(ctx, engine, arena);
    let raw = L::fetch(args, &mut cx)?;
    let values = L::coerce(raw, &mut cx)?;
    let out = body.call(&mut cx, values)?;
    Ok(out.into_value(&mut cx))
}

/// The adapter for a `raw[..]` row: apply the row's declared arity and
/// policy, then run the body, which evaluates its own arguments and may
/// index any position below the declared minimum.
#[inline]
pub(crate) fn raw<'a, const OP: u8, F>(
    args: &'a [CompiledNode],
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
    body: F,
) -> Result<&'a DataValue<'a>>
where
    F: for<'c> FnOnce(
        &'a [CompiledNode],
        &'c mut ContextStack<'a>,
        &'c Engine,
        &'a Bump,
    ) -> Result<&'a DataValue<'a>>,
{
    let meta: &'static OpMeta = const { OpCode::ALL[OP as usize].meta() };
    let arity: Arity = const { OpCode::ALL[OP as usize].arity() };
    if let Some(early) = check_arity(args.len(), arity, meta)? {
        return Ok(early);
    }
    body(args, ctx, engine, arena)
}

/// The adapter for an `iter[..]` row: [`raw`], passing the iteration-source
/// classification through.
#[inline]
pub(crate) fn iter<'a, const OP: u8, F>(
    args: &'a [CompiledNode],
    iter_arg_kind: IterArgKind,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
    body: F,
) -> Result<&'a DataValue<'a>>
where
    F: for<'c> FnOnce(
        &'a [CompiledNode],
        IterArgKind,
        &'c mut ContextStack<'a>,
        &'c Engine,
        &'a Bump,
    ) -> Result<&'a DataValue<'a>>,
{
    let meta: &'static OpMeta = const { OpCode::ALL[OP as usize].meta() };
    let arity: Arity = const { OpCode::ALL[OP as usize].arity() };
    if let Some(early) = check_arity(args.len(), arity, meta)? {
        return Ok(early);
    }
    body(args, iter_arg_kind, ctx, engine, arena)
}

/// The adapter for an `each[..]` row: apply the declared arity, resolve
/// `args[0]` as the iteration source, answer a null, missing or empty-array
/// source with the row's `on_empty_source`, and hand the body the rest.
///
/// The source is resolved (and its items charged) before the body runs,
/// exactly as the iterator bodies did themselves.
#[inline]
pub(crate) fn each<'a, const OP: u8, F>(
    args: &'a [CompiledNode],
    iter_arg_kind: IterArgKind,
    ctx: &mut ContextStack<'a>,
    engine: &Engine,
    arena: &'a Bump,
    body: F,
) -> Result<&'a DataValue<'a>>
where
    F: for<'c> FnOnce(
        Items<'a>,
        &'a [CompiledNode],
        &'c mut ContextStack<'a>,
        &'c Engine,
        &'a Bump,
    ) -> Result<&'a DataValue<'a>>,
{
    let meta: &'static OpMeta = const { OpCode::ALL[OP as usize].meta() };
    let arity: Arity = const {
        let arity = OpCode::ALL[OP as usize].arity();
        assert!(arity.min >= 1, "an `each` row reads at least its source");
        arity
    };
    let on_empty: Singleton = const {
        match OpCode::ALL[OP as usize].meta().on_empty_source {
            Some(value) => value,
            None => panic!("an `each` row must declare `on_empty_source`"),
        }
    };
    if let Some(early) = check_arity(args.len(), arity, meta)? {
        return Ok(early);
    }
    let items = match resolve_iter_input(&args[0], iter_arg_kind, ctx, engine, arena)? {
        ResolvedInput::Iterable(src) if !src.is_empty() => Items::Array(src),
        ResolvedInput::Iterable(_) | ResolvedInput::Empty => return Ok(on_empty()),
        ResolvedInput::Bridge(DataValue::Object(pairs)) => Items::Object(pairs),
        ResolvedInput::Bridge(value) => Items::Scalar(value),
    };
    body(items, args, ctx, engine, arena)
}
