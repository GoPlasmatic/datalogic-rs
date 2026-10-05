//! Argument extractors and result conversions for `eager` rows.
//!
//! Each position of an `eager(..)` row names an extractor: a type that
//! says how that argument is read. The adapters in [`super::eager`]
//! fetch every argument first and coerce them second (see
//! [`Extract`]), then turn the body's return value into a
//! [`DataValue`] through [`IntoValue`].
//!
//! | Extractor | Body receives | Behaviour |
//! |-----------|---------------|-----------|
//! | [`Any`] | `&DataValue` | evaluated, not coerced |
//! | [`Str`] | `&str` | `data_to_str` coercion |
//! | [`Int`] | `i64` | an integer, otherwise `InvalidArguments` |
//! | [`StrictNum`] | `f64` | a number or a numeric string, otherwise `InvalidArguments` |
//! | [`Truthy`] | `bool` | the engine's truthiness rule |
//! | [`Obj`] | `&[(&str, DataValue)]` | an object, otherwise `InvalidArguments` |
//! | [`Nullable<T>`] | `Option<T>` | `null` becomes `None`, anything else must coerce |
//! | [`Opt<T>`] | `Option<T>` | absent becomes `None`; a present argument must coerce |
//! | [`Lenient<T>`] | `Option<T>` | absent **or not coercible** becomes `None` |
//! | [`Lazy`] | `Option<&CompiledNode>` | not evaluated; the body evaluates it if it needs it |
//! | [`Rest<T>`] | [`RestArgs`] | every remaining argument, evaluated and coerced one at a time on request |
//!
//! The tensor family adds its own (`TensorArg`, `ShapeArg`, ...) in
//! `operators::tensor`, with that family's error messages. Extractors are
//! added when a row needs one: a numeric coercion under the engine's
//! `NumericCoercionConfig` and an array extractor are not here yet,
//! because every operator that reads those has variadic or
//! operator-specific rules that keep it a `raw` row.

// Which extractors a build uses depends on which operator families it
// compiles in; with every family on, all of them are used and dead code
// is still reported.
#![cfg_attr(not(feature = "all-operators"), allow(dead_code, unused_imports))]

use datavalue::NumberValue;

use super::eager::Cx;
use crate::arena::{DataValue, data_to_str, truthy_arena};
use crate::{CompiledNode, Error, Result};

// ---------------------------------------------------------------------------
// Argument extractors
// ---------------------------------------------------------------------------

/// An argument extractor: how one position of an `eager` row is read.
///
/// Extraction runs in two phases so the adapter can evaluate every
/// argument before coercing any: [`fetch`](Extract::fetch) produces the
/// raw argument (evaluated, or left as a node for [`Lazy`]), then
/// [`coerce`](Extract::coerce) turns it into what the body receives.
pub(crate) trait Extract<'a> {
    /// What phase 1 produces.
    type Raw;
    /// What the operator body receives.
    type Out;
    /// `(min, max)` arguments this extractor consumes.
    const SPAN: (u8, Option<u8>);
    /// Phase 1: fetch argument `i`. A required position is always present:
    /// the adapter has checked arity, so `args[i]` is in bounds.
    fn fetch(args: &'a [CompiledNode], i: usize, cx: &mut Cx<'_, 'a>) -> Result<Self::Raw>;
    /// Phase 2: coerce it.
    fn coerce(raw: Self::Raw, cx: &mut Cx<'_, 'a>) -> Result<Self::Out>;
}

/// A required argument read from its evaluated value. Every type that
/// implements this is an [`Extract`] with span `(1, 1)`, and can be
/// wrapped in [`Opt`] or [`Lenient`].
pub(crate) trait Coerce<'a> {
    type Out;
    fn coerce(value: &'a DataValue<'a>, cx: &mut Cx<'_, 'a>) -> Result<Self::Out>;
}

impl<'a, T: Coerce<'a>> Extract<'a> for T {
    type Raw = &'a DataValue<'a>;
    type Out = T::Out;
    const SPAN: (u8, Option<u8>) = (1, Some(1));
    #[inline(always)]
    fn fetch(args: &'a [CompiledNode], i: usize, cx: &mut Cx<'_, 'a>) -> Result<Self::Raw> {
        cx.eval(&args[i])
    }
    #[inline(always)]
    fn coerce(raw: Self::Raw, cx: &mut Cx<'_, 'a>) -> Result<Self::Out> {
        <T as Coerce<'a>>::coerce(raw, cx)
    }
}

/// The evaluated argument, uncoerced.
pub(crate) struct Any;

impl<'a> Coerce<'a> for Any {
    type Out = &'a DataValue<'a>;
    #[inline(always)]
    fn coerce(value: &'a DataValue<'a>, _cx: &mut Cx<'_, 'a>) -> Result<Self::Out> {
        Ok(value)
    }
}

/// The argument's string form ([`data_to_str`] coercion).
pub(crate) struct Str;

impl<'a> Coerce<'a> for Str {
    type Out = &'a str;
    #[inline(always)]
    fn coerce(value: &'a DataValue<'a>, cx: &mut Cx<'_, 'a>) -> Result<Self::Out> {
        Ok(data_to_str(value, cx.arena))
    }
}

/// An integer, read with `as_i64`. Only meaningful inside [`Lenient`]
/// today, where a non-integer becomes `None`.
pub(crate) struct Int;

impl<'a> Coerce<'a> for Int {
    type Out = i64;
    #[inline(always)]
    fn coerce(value: &'a DataValue<'a>, _cx: &mut Cx<'_, 'a>) -> Result<Self::Out> {
        value.as_i64().ok_or_else(Error::invalid_args)
    }
}

/// A number or a numeric string, as `f64`; anything else (including
/// `true` and `null`) is `InvalidArguments` (`abs` / `ceil` / `floor`).
pub(crate) struct StrictNum;

impl<'a> Coerce<'a> for StrictNum {
    type Out = f64;
    #[inline(always)]
    fn coerce(value: &'a DataValue<'a>, _cx: &mut Cx<'_, 'a>) -> Result<Self::Out> {
        match value {
            DataValue::Number(n) => Ok(n.as_f64()),
            DataValue::String(s) => s.parse().map_err(|_| Error::invalid_args()),
            _ => Err(Error::invalid_args()),
        }
    }
}

/// The argument's truthiness, under the engine's configured rule.
pub(crate) struct Truthy;

impl<'a> Coerce<'a> for Truthy {
    type Out = bool;
    #[inline(always)]
    fn coerce(value: &'a DataValue<'a>, cx: &mut Cx<'_, 'a>) -> Result<Self::Out> {
        Ok(truthy_arena(value, cx.engine))
    }
}

/// An object's key/value pairs; anything else is `InvalidArguments`.
pub(crate) struct Obj;

impl<'a> Coerce<'a> for Obj {
    type Out = &'a [(&'a str, DataValue<'a>)];
    #[inline(always)]
    fn coerce(value: &'a DataValue<'a>, _cx: &mut Cx<'_, 'a>) -> Result<Self::Out> {
        match value {
            DataValue::Object(pairs) => Ok(pairs),
            _ => Err(Error::invalid_args()),
        }
    }
}

/// `null` becomes `None`; any other value must coerce as `T`
/// (`keys` / `values` / `entries` treat a null object as empty).
pub(crate) struct Nullable<T>(std::marker::PhantomData<T>);

impl<'a, T: Coerce<'a>> Coerce<'a> for Nullable<T> {
    type Out = Option<T::Out>;
    #[inline(always)]
    fn coerce(value: &'a DataValue<'a>, cx: &mut Cx<'_, 'a>) -> Result<Self::Out> {
        match value {
            DataValue::Null => Ok(None),
            value => T::coerce(value, cx).map(Some),
        }
    }
}

/// An optional trailing argument: absent becomes `None`; a present one
/// must coerce.
pub(crate) struct Opt<T>(std::marker::PhantomData<T>);

impl<'a, T: Coerce<'a>> Extract<'a> for Opt<T> {
    type Raw = Option<&'a DataValue<'a>>;
    type Out = Option<T::Out>;
    const SPAN: (u8, Option<u8>) = (0, Some(1));
    #[inline(always)]
    fn fetch(args: &'a [CompiledNode], i: usize, cx: &mut Cx<'_, 'a>) -> Result<Self::Raw> {
        // A `match`, not `map(|n| cx.eval(n))`: the closure is not
        // inlined, which puts the child's dispatch behind an extra call.
        match args.get(i) {
            Some(node) => Ok(Some(cx.eval(node)?)),
            None => Ok(None),
        }
    }
    #[inline(always)]
    fn coerce(raw: Self::Raw, cx: &mut Cx<'_, 'a>) -> Result<Self::Out> {
        raw.map(|v| T::coerce(v, cx)).transpose()
    }
}

/// An optional trailing argument that also swallows coercion failures:
/// absent **or not coercible** becomes `None` (`substr`'s start and
/// length).
pub(crate) struct Lenient<T>(std::marker::PhantomData<T>);

impl<'a, T: Coerce<'a>> Extract<'a> for Lenient<T> {
    type Raw = Option<&'a DataValue<'a>>;
    type Out = Option<T::Out>;
    const SPAN: (u8, Option<u8>) = (0, Some(1));
    #[inline(always)]
    fn fetch(args: &'a [CompiledNode], i: usize, cx: &mut Cx<'_, 'a>) -> Result<Self::Raw> {
        // A `match`, not `map(|n| cx.eval(n))`: the closure is not
        // inlined, which puts the child's dispatch behind an extra call.
        match args.get(i) {
            Some(node) => Ok(Some(cx.eval(node)?)),
            None => Ok(None),
        }
    }
    #[inline(always)]
    fn coerce(raw: Self::Raw, cx: &mut Cx<'_, 'a>) -> Result<Self::Out> {
        Ok(raw.and_then(|v| T::coerce(v, cx).ok()))
    }
}

/// An optional trailing argument the body evaluates itself, if and when
/// it needs it (`format_date` / `parse_date`'s timezone, which is only
/// read once the date has parsed; `tensor`'s dtype, which only a
/// nested-array input reads).
pub(crate) struct Lazy;

impl<'a> Extract<'a> for Lazy {
    type Raw = Option<&'a CompiledNode>;
    type Out = Option<&'a CompiledNode>;
    const SPAN: (u8, Option<u8>) = (0, Some(1));
    #[inline(always)]
    fn fetch(args: &'a [CompiledNode], i: usize, _cx: &mut Cx<'_, 'a>) -> Result<Self::Raw> {
        Ok(args.get(i))
    }
    #[inline(always)]
    fn coerce(raw: Self::Raw, _cx: &mut Cx<'_, 'a>) -> Result<Self::Out> {
        Ok(raw)
    }
}

/// Every remaining argument (a variadic tail). Must be the row's last
/// extractor. Nothing is evaluated up front: the body evaluates and
/// coerces each argument when it asks for it, in the order it asks, so a
/// failure on one argument stops before the next is evaluated.
pub(crate) struct Rest<T>(std::marker::PhantomData<T>);

impl<'a, T: Coerce<'a>> Extract<'a> for Rest<T> {
    type Raw = &'a [CompiledNode];
    type Out = RestArgs<'a, T>;
    const SPAN: (u8, Option<u8>) = (0, None);
    #[inline(always)]
    fn fetch(args: &'a [CompiledNode], i: usize, _cx: &mut Cx<'_, 'a>) -> Result<Self::Raw> {
        Ok(args.get(i..).unwrap_or(&[]))
    }
    #[inline(always)]
    fn coerce(raw: Self::Raw, _cx: &mut Cx<'_, 'a>) -> Result<Self::Out> {
        Ok(RestArgs {
            nodes: raw,
            _coerce: std::marker::PhantomData,
        })
    }
}

/// What a [`Rest<T>`] position hands the body: the unevaluated tail.
pub(crate) struct RestArgs<'a, T> {
    nodes: &'a [CompiledNode],
    _coerce: std::marker::PhantomData<T>,
}

impl<'a, T: Coerce<'a>> RestArgs<'a, T> {
    /// How many arguments the tail holds.
    #[inline(always)]
    pub(crate) fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether the tail is empty.
    #[inline(always)]
    pub(crate) fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Evaluate and coerce argument `i` of the tail.
    #[inline(always)]
    pub(crate) fn get(&self, i: usize, cx: &mut Cx<'_, 'a>) -> Result<T::Out> {
        let value = cx.eval(&self.nodes[i])?;
        T::coerce(value, cx)
    }
}

// ---------------------------------------------------------------------------
// Result conversion
// ---------------------------------------------------------------------------

/// How an operator body's return value becomes the dispatch result.
pub(crate) trait IntoValue<'a> {
    fn into_value(self, cx: &mut Cx<'_, 'a>) -> &'a DataValue<'a>;
}

impl<'a> IntoValue<'a> for &'a DataValue<'a> {
    #[inline(always)]
    fn into_value(self, _cx: &mut Cx<'_, 'a>) -> &'a DataValue<'a> {
        self
    }
}

impl<'a> IntoValue<'a> for DataValue<'a> {
    #[inline(always)]
    fn into_value(self, cx: &mut Cx<'_, 'a>) -> &'a DataValue<'a> {
        cx.alloc(self)
    }
}

impl<'a> IntoValue<'a> for bool {
    #[inline(always)]
    fn into_value(self, _cx: &mut Cx<'_, 'a>) -> &'a DataValue<'a> {
        crate::arena::singletons::singleton_bool(self)
    }
}

impl<'a> IntoValue<'a> for &'a str {
    #[inline(always)]
    fn into_value(self, cx: &mut Cx<'_, 'a>) -> &'a DataValue<'a> {
        if self.is_empty() {
            crate::arena::singletons::singleton_empty_string()
        } else {
            cx.alloc(DataValue::String(self))
        }
    }
}

impl<'a> IntoValue<'a> for i64 {
    #[inline(always)]
    fn into_value(self, cx: &mut Cx<'_, 'a>) -> &'a DataValue<'a> {
        match crate::arena::singletons::singleton_small_int(self) {
            Some(v) => v,
            None => cx.alloc(DataValue::from_i64(self)),
        }
    }
}

impl<'a> IntoValue<'a> for NumberValue {
    #[inline(always)]
    fn into_value(self, cx: &mut Cx<'_, 'a>) -> &'a DataValue<'a> {
        cx.alloc(DataValue::Number(self))
    }
}

impl<'a> IntoValue<'a> for f64 {
    #[inline(always)]
    fn into_value(self, cx: &mut Cx<'_, 'a>) -> &'a DataValue<'a> {
        NumberValue::from_f64(self).into_value(cx)
    }
}

impl<'a, T: IntoValue<'a>> IntoValue<'a> for Option<T> {
    #[inline(always)]
    fn into_value(self, cx: &mut Cx<'_, 'a>) -> &'a DataValue<'a> {
        match self {
            Some(v) => v.into_value(cx),
            None => crate::arena::singletons::singleton_null(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Engine;
    use crate::arena::ContextStack;
    use bumpalo::Bump;

    /// Coerce `value` as `T` in a throwaway context.
    fn coerce<T>(value: &DataValue<'static>) -> Result<String>
    where
        T: for<'a> Coerce<'a>,
        for<'a> <T as Coerce<'a>>::Out: std::fmt::Debug,
    {
        let engine = Engine::new();
        let arena = Bump::new();
        let root = arena.alloc(DataValue::Null);
        let mut ctx = ContextStack::new(root, false);
        let mut cx = Cx::new(&mut ctx, &engine, &arena);
        let value: &DataValue<'_> = arena.alloc(*value);
        T::coerce(value, &mut cx).map(|out| format!("{out:?}"))
    }

    #[test]
    fn strict_num_takes_numbers_and_numeric_strings_only() {
        assert_eq!(
            coerce::<StrictNum>(&DataValue::from_i64(-3)).unwrap(),
            "-3.0"
        );
        assert_eq!(
            coerce::<StrictNum>(&DataValue::String("2.5")).unwrap(),
            "2.5"
        );
        for bad in [
            DataValue::String("x"),
            DataValue::Bool(true),
            DataValue::Null,
        ] {
            assert!(coerce::<StrictNum>(&bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn truthy_follows_the_engine_rule() {
        assert_eq!(coerce::<Truthy>(&DataValue::from_i64(0)).unwrap(), "false");
        assert_eq!(coerce::<Truthy>(&DataValue::String("a")).unwrap(), "true");
        assert_eq!(coerce::<Truthy>(&DataValue::Array(&[])).unwrap(), "false");
    }

    #[test]
    fn nullable_obj_admits_objects_and_null_only() {
        assert_eq!(coerce::<Nullable<Obj>>(&DataValue::Null).unwrap(), "None");
        assert_eq!(
            coerce::<Nullable<Obj>>(&DataValue::Object(&[])).unwrap(),
            "Some([])"
        );
        assert!(coerce::<Nullable<Obj>>(&DataValue::Array(&[])).is_err());
        assert!(coerce::<Obj>(&DataValue::Null).is_err());
    }
}
