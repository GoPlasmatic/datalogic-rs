//! Typed rows for fixed-arity operators.
//!
//! A table row such as `eager(Str, Str) string::ends_with` declares the
//! operator's signature as a list of argument extractors. The body is a
//! plain function over the extracted values:
//!
//! ```ignore
//! pub(crate) fn ends_with<'a>(cx: &mut Cx<'_, 'a>, text: &'a str, suffix: &'a str) -> Result<bool>
//! ```
//!
//! and the dispatch arm calls one of the `callN` adapters below, which
//! does the plumbing every such operator used to write by hand:
//!
//! The extractors and result conversions live in [`super::extract`].
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
use super::meta::{Extra, Miss, OpMeta};
use crate::arena::{ContextStack, DataValue};
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
    /// `raw` and `iter` rows: the body checks its own arguments.
    pub(crate) const ANY: Arity = Arity { min: 0, max: None };

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

/// Generate `callN`: the adapter for an `eager` row with `N` extractors.
///
/// `OP` is the row's opcode discriminant. The row's metadata and arity
/// are read through it in `const` blocks, so each adapter's policy checks
/// are compile-time constants whether or not the adapter is inlined.
macro_rules! eager_call {
    ($name:ident; $($A:ident $raw:ident $val:ident $i:tt),*) => {
        #[inline]
        pub(crate) fn $name<'a, const OP: u8, $($A: Extract<'a>,)* R: IntoValue<'a>, F>(
            args: &'a [CompiledNode],
            ctx: &mut ContextStack<'a>,
            engine: &Engine,
            arena: &'a Bump,
            body: F,
        ) -> Result<&'a DataValue<'a>>
        where
            F: for<'c> FnOnce(&mut Cx<'c, 'a>, $($A::Out),*) -> Result<R>,
        {
            let meta: &'static OpMeta = const { OpCode::ALL[OP as usize].meta() };
            let arity: Arity = const { OpCode::ALL[OP as usize].arity() };
            if let Some(early) = check_arity(args.len(), arity, meta)? {
                return Ok(early);
            }
            let mut cx = Cx::new(ctx, engine, arena);
            $( let $raw = $A::fetch(args, $i, &mut cx)?; )*
            $( let $val = $A::coerce($raw, &mut cx)?; )*
            let out = body(&mut cx, $($val),*)?;
            Ok(out.into_value(&mut cx))
        }
    };
}

eager_call!(call0;);
eager_call!(call1; A0 r0 v0 0);
eager_call!(call2; A0 r0 v0 0, A1 r1 v1 1);
eager_call!(call3; A0 r0 v0 0, A1 r1 v1 1, A2 r2 v2 2);
eager_call!(call4; A0 r0 v0 0, A1 r1 v1 1, A2 r2 v2 2, A3 r3 v3 3);
