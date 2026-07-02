//! `PreLit` — literal values pre-built at compile time so the dispatch hot
//! path returns a borrow instead of re-converting the literal into the
//! per-evaluation arena on every call.
//!
//! Trivial literals (Null/Bool/Number/empty composites, and the datetime
//! scalars) are plain `DataValue<'static>` — no borrow, no allocation.
//! Non-trivial literals (non-empty Strings/Arrays/Objects) are the
//! interesting case: an arena-shaped [`DataValue`] view of them must borrow
//! its string bytes and slice spines from *somewhere that lives as long as
//! the compiled rule*. Embedding a `bumpalo::Bump` in [`super::Logic`] would
//! do that but costs `Sync` (`Bump` is `!Sync` by type, even when never
//! mutated after build), and `Logic` is shared across threads via
//! `Arc<Logic>`. Instead each composite literal owns its storage through a
//! pair of [`self_cell`] cells:
//!
//! 1. [`SpineCell`]: owner [`LitOwner`] (a clone of the owned literal plus
//!    pre-built child cells for nested composites) → dependent
//!    [`LitSpine`], the boxed element slice whose strings borrow from the
//!    owner.
//! 2. [`RootCell`]: owner [`SpineCell`] → dependent the root [`DataValue`]
//!    whose `Array`/`Object` payload borrows the spine.
//!
//! Two chained cells are required because a dependent may only borrow from
//! its owner, never from a sibling field — the root `DataValue` must
//! reference the spine, so the spine has to sit one ownership level below
//! it. `self_cell` keeps this sound without `unsafe` in this crate (the
//! crate is `#![forbid(unsafe_code)]`), and the generated cells are
//! `Send + Sync` whenever owner and dependent are, so `Logic` stays
//! shareable.
//!
//! Memory tradeoff: a prebuilt composite duplicates the literal (the
//! `value` field on `CompiledNode::Value` keeps the original owned form for
//! serialisation, folding, and operator fast paths that read it), and
//! nested composites additionally clone their sub-tree per nesting level.
//! Rule literals are typically small; the duplication buys the removal of a
//! deep-convert *per evaluation*.

use crate::arena::DataValue;
use datavalue::OwnedDataValue;
use self_cell::self_cell;

/// Storage owned by one composite-literal cell: the owned literal itself
/// (string bytes and element order/shape for (re)building the spine) plus
/// fully-built cells for every non-empty composite element, in walk order.
#[derive(Debug, Clone)]
struct LitOwner {
    /// The owned literal this cell mirrors. String elements of the spine
    /// borrow their bytes from here.
    value: OwnedDataValue,
    /// Prebuilt cells for nested non-empty Array/Object elements, in the
    /// order [`build_spine`] consumes them.
    children: Box<[PreLit]>,
}

/// The boxed element slice ("spine") of one composite level, borrowing
/// strings from the [`LitOwner`] and nested composite payloads from the
/// owner's `children` cells.
#[derive(Debug)]
enum LitSpine<'a> {
    /// No spine — the root borrows the owner directly (String literals).
    None,
    /// Array elements.
    Array(Box<[DataValue<'a>]>),
    /// Object entries; keys borrow from the owner's pairs.
    Object(Box<[(&'a str, DataValue<'a>)]>),
}

self_cell!(
    /// Owns [`LitOwner`] and the [`LitSpine`] borrowing from it.
    struct SpineCell {
        owner: LitOwner,
        #[covariant]
        dependent: LitSpine,
    }

    impl {Debug}
);

self_cell!(
    /// Owns a [`SpineCell`] and the root [`DataValue`] borrowing its spine.
    struct RootCell {
        owner: SpineCell,
        #[covariant]
        dependent: DataValue,
    }

    impl {Debug}
);

/// A literal pre-built at compile time, returned by reference from the
/// dispatch literal fast path (and from `evaluate_switch`'s folded-case
/// arms) without touching the per-evaluation arena.
///
/// Boxed to a single pointer so `Option<PreLit>` stays 8 bytes and
/// `CompiledNode::Value` keeps its size (guarded by the layout test in
/// [`super`]).
pub(crate) struct PreLit(Box<PreLitInner>);

/// The two prebuilt shapes behind [`PreLit`].
#[derive(Debug)]
enum PreLitInner {
    /// Trivial literal whose payload is `'static` — Null, Bool, Number,
    /// empty String/Array/Object, and the datetime scalars.
    Static(DataValue<'static>),
    /// Non-trivial composite (or non-empty String) pre-built through the
    /// self-referential cell chain.
    Cell(RootCell),
}

impl PreLit {
    /// Wrap a trivial `'static` literal. Used by
    /// [`super::populate::precompute_lit`] at node construction — cheap
    /// enough for the runtime `synthetic_value` wrappers.
    #[inline]
    pub(crate) fn from_static(dv: DataValue<'static>) -> Self {
        PreLit(Box::new(PreLitInner::Static(dv)))
    }

    /// Pre-build a non-trivial literal (non-empty String/Array/Object).
    /// Returns `None` for shapes [`super::populate::precompute_lit`]
    /// already covers (or would, at construction time) — callers use this
    /// from the post-compile populate pass only, so the build cost is paid
    /// once per compiled rule, never per evaluation.
    pub(crate) fn composite(value: &OwnedDataValue) -> Option<Self> {
        match value {
            OwnedDataValue::String(s) if !s.is_empty() => {}
            OwnedDataValue::Array(a) if !a.is_empty() => {}
            OwnedDataValue::Object(o) if !o.is_empty() => {}
            _ => return None,
        }
        Some(PreLit(Box::new(PreLitInner::Cell(build_cell(LitOwner {
            value: value.clone(),
            children: build_children(value),
        })))))
    }

    /// Borrow the prebuilt value at the caller's lifetime.
    ///
    /// The name is load-bearing: `evaluate_switch` (operators/control.rs)
    /// pattern-matches `Value { lit: Some(av), .. }` and calls
    /// `av.as_ref()` — this inherent method keeps that call site compiling
    /// unchanged while coupling the output lifetime to `&self`, which the
    /// `AsRef` trait cannot express (`AsRef<T>` fixes `T` independently of
    /// the `&self` borrow).
    #[inline]
    pub(crate) fn as_ref<'s>(&'s self) -> &'s DataValue<'s> {
        match &*self.0 {
            // `&'s DataValue<'static>` coerces to `&'s DataValue<'s>`
            // (DataValue is covariant in its lifetime).
            PreLitInner::Static(dv) => dv,
            PreLitInner::Cell(cell) => cell.borrow_dependent(),
        }
    }
}

impl Clone for PreLit {
    /// Rebuild rather than share: the cells own their storage, so a clone
    /// re-runs the (compile-time-only) builder against a clone of the
    /// owner. Keeps `CompiledNode: Clone` derivable.
    fn clone(&self) -> Self {
        match &*self.0 {
            PreLitInner::Static(dv) => PreLit(Box::new(PreLitInner::Static(*dv))),
            PreLitInner::Cell(cell) => PreLit(Box::new(PreLitInner::Cell(build_cell(
                cell.borrow_owner().borrow_owner().clone(),
            )))),
        }
    }
}

impl std::fmt::Debug for PreLit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Print the resolved value — the cell internals are noise.
        f.debug_tuple("PreLit").field(self.as_ref()).finish()
    }
}

/// Assemble the two-cell chain for one composite level.
fn build_cell(owner: LitOwner) -> RootCell {
    let spine = SpineCell::new(owner, build_spine);
    RootCell::new(spine, build_root)
}

/// Build the prebuilt child cells for every non-empty composite element of
/// `value`, in the exact order [`build_spine`] consumes them.
fn build_children(value: &OwnedDataValue) -> Box<[PreLit]> {
    fn is_child(v: &OwnedDataValue) -> bool {
        match v {
            OwnedDataValue::Array(a) => !a.is_empty(),
            OwnedDataValue::Object(o) => !o.is_empty(),
            _ => false,
        }
    }
    let mut children = Vec::new();
    match value {
        OwnedDataValue::Array(items) => {
            for it in items.iter().filter(|it| is_child(it)) {
                children.extend(PreLit::composite(it));
            }
        }
        OwnedDataValue::Object(pairs) => {
            for (_, v) in pairs.iter().filter(|(_, v)| is_child(v)) {
                children.extend(PreLit::composite(v));
            }
        }
        _ => {}
    }
    children.into_boxed_slice()
}

/// Build the element spine of one level, borrowing strings from the owner
/// and nested composite payloads from the owner's prebuilt children.
fn build_spine(owner: &LitOwner) -> LitSpine<'_> {
    let mut cursor = 0usize;
    match &owner.value {
        OwnedDataValue::Array(items) => LitSpine::Array(
            items
                .iter()
                .map(|it| element_dv(it, &owner.children, &mut cursor))
                .collect(),
        ),
        OwnedDataValue::Object(pairs) => LitSpine::Object(
            pairs
                .iter()
                .map(|(k, v)| (k.as_str(), element_dv(v, &owner.children, &mut cursor)))
                .collect(),
        ),
        // Strings (the only other shape `PreLit::composite` accepts) have
        // no spine; `build_root` borrows the owner's bytes directly.
        _ => LitSpine::None,
    }
}

/// Convert one element of the owned literal into its spine `DataValue`.
/// Non-empty composite elements consume the next prebuilt child cell.
fn element_dv<'a>(
    v: &'a OwnedDataValue,
    children: &'a [PreLit],
    cursor: &mut usize,
) -> DataValue<'a> {
    match v {
        OwnedDataValue::Null => DataValue::Null,
        OwnedDataValue::Bool(b) => DataValue::Bool(*b),
        OwnedDataValue::Number(n) => DataValue::Number(*n),
        OwnedDataValue::String(s) => DataValue::String(s.as_str()),
        OwnedDataValue::Array(a) if a.is_empty() => DataValue::Array(&[]),
        OwnedDataValue::Object(o) if o.is_empty() => DataValue::Object(&[]),
        OwnedDataValue::Array(_) | OwnedDataValue::Object(_) => {
            let child = children.get(*cursor);
            *cursor += 1;
            debug_assert!(child.is_some(), "spine walk out of sync with children");
            // `DataValue` is `Copy`; the copied enum's payload references
            // stay valid for `'a` because the child cell lives in the
            // owner. The `None` fallback is unreachable by construction
            // (`build_children` mirrors this walk exactly).
            child.map_or(DataValue::Null, |c| *c.as_ref())
        }
        #[cfg(feature = "datetime")]
        OwnedDataValue::DateTime(d) => DataValue::DateTime(*d),
        #[cfg(feature = "datetime")]
        OwnedDataValue::Duration(d) => DataValue::Duration(*d),
    }
}

/// Build the root `DataValue` for one level from its spine (or, for
/// strings, straight from the owner's bytes).
fn build_root<'a>(sc: &'a SpineCell) -> DataValue<'a> {
    match sc.borrow_dependent() {
        LitSpine::Array(v) => DataValue::Array(v),
        LitSpine::Object(v) => DataValue::Object(v),
        LitSpine::None => match &sc.borrow_owner().value {
            OwnedDataValue::String(s) => DataValue::String(s.as_str()),
            // Unreachable: `PreLit::composite` only builds cells for
            // non-empty String/Array/Object.
            _ => DataValue::Null,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owned(json: &str) -> OwnedDataValue {
        OwnedDataValue::from_json(json).unwrap()
    }

    /// `Logic` is shared via `Arc` across threads; the prebuilt cells must
    /// not cost `Send`/`Sync` (an embedded `Bump` would).
    #[test]
    fn prelit_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<PreLit>();
    }

    #[test]
    fn trivial_shapes_are_rejected() {
        for json in ["null", "true", "42", "\"\"", "[]", "{}"] {
            assert!(
                PreLit::composite(&owned(json)).is_none(),
                "expected no composite cell for {json}"
            );
        }
    }

    #[test]
    fn flat_array_prebuilds_and_borrows() {
        let value = owned(r#"[1, "two", 3.5, true, null, []]"#);
        let lit = PreLit::composite(&value).unwrap();
        let dv = lit.as_ref();
        let items = match dv {
            DataValue::Array(items) => *items,
            other => panic!("expected array, got {other:?}"),
        };
        assert_eq!(items.len(), 6);
        assert_eq!(items[0].as_i64(), Some(1));
        assert_eq!(items[1].as_str(), Some("two"));
        assert_eq!(items[2].as_f64(), Some(3.5));
        assert!(matches!(items[4], DataValue::Null));
        assert!(matches!(items[5], DataValue::Array(&[])));
    }

    #[test]
    fn nested_composites_round_trip() {
        let value = owned(r#"{"a": [[1, "x"], {"b": 2}], "c": "s"}"#);
        let lit = PreLit::composite(&value).unwrap();
        // The prebuilt view must equal what a fresh owned round-trip gives.
        assert_eq!(lit.as_ref().to_owned(), value);
    }

    #[test]
    fn clone_rebuilds_and_survives_the_original() {
        let value = owned(r#"[["deep", ["deeper"]], {"k": "v"}]"#);
        let lit = PreLit::composite(&value).unwrap();
        let cloned = lit.clone();
        drop(lit);
        drop(value);
        let moved = cloned; // the cells must tolerate moves
        assert_eq!(
            moved.as_ref().to_owned(),
            owned(r#"[["deep", ["deeper"]], {"k": "v"}]"#)
        );
    }

    #[test]
    fn string_literal_prebuilds() {
        let value = owned(r#""hello world""#);
        let lit = PreLit::composite(&value).unwrap();
        assert_eq!(lit.as_ref().as_str(), Some("hello world"));
    }
}
