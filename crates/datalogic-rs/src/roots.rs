//! [`Roots`]: several values evaluated as one top-level object, without
//! building that object first.

use bumpalo::Bump;
use datavalue::OwnedDataValue;

use crate::Result;
use crate::arena::DataValue;
use crate::eval_input::sealed;
use crate::{EvalInput, OwnedInput, ParsedData};

/// Several values evaluated as the fields of one top-level object, each
/// under its own name, without building that object.
///
/// A host that keeps a request's payload, its metadata and its claims in
/// separate values would otherwise copy all of them into one combined value
/// for every evaluation (`json!({"data": data, "metadata": metadata})`).
/// `Roots` borrows each part instead: a rule reads `{"var": "data.user"}`
/// or `{"var": "metadata.channel"}` exactly as it would from the combined
/// object, and the engine views each part in place the way it views a
/// single input of that type.
///
/// Accepted wherever the engine takes input, by reference or by value
/// ([`EvalInput`] and [`OwnedInput`]). Each root may be a
/// `&serde_json::Value` (`serde_json` feature), `&OwnedDataValue`,
/// `&ParsedData` or `&DataValue`, mixed freely.
///
/// Names keep the order they were first given in, which is the key order
/// of the object a rule sees (`{"var": ""}`). Giving a name again replaces
/// its value in place.
///
/// # Example
///
/// ```rust
/// # #[cfg(feature = "serde_json")] {
/// use datalogic_rs::{Engine, Roots};
/// use serde_json::json;
///
/// let data = json!({"user": {"name": "ana"}});
/// let metadata = json!({"channel": "web"});
///
/// let engine = Engine::new();
/// let rule = engine
///     .compile(r#"{"cat": [{"var": "data.user.name"}, "@", {"var": "metadata.channel"}]}"#)
///     .unwrap();
///
/// let roots = Roots::from([("data", &data), ("metadata", &metadata)]);
/// let mut session = engine.session();
/// assert_eq!(session.eval_str(&rule, &roots).unwrap(), r#""ana@web""#);
/// # }
/// ```
#[derive(Debug, Clone, Default)]
pub struct Roots<'r> {
    parts: Vec<(&'r str, RootValue<'r>)>,
}

/// One value a [`Roots`] holds. Built from a reference to any accepted
/// input type through `From`; there is nothing else to do with it.
#[derive(Debug, Clone, Copy)]
pub struct RootValue<'r>(Part<'r>);

#[derive(Debug, Clone, Copy)]
enum Part<'r> {
    Owned(&'r OwnedDataValue),
    #[cfg(feature = "serde_json")]
    Json(&'r serde_json::Value),
    Parsed(&'r ParsedData),
    Arena(&'r DataValue<'r>),
}

impl<'r> From<&'r OwnedDataValue> for RootValue<'r> {
    fn from(v: &'r OwnedDataValue) -> Self {
        RootValue(Part::Owned(v))
    }
}

#[cfg(feature = "serde_json")]
impl<'r> From<&'r serde_json::Value> for RootValue<'r> {
    fn from(v: &'r serde_json::Value) -> Self {
        RootValue(Part::Json(v))
    }
}

impl<'r> From<&'r ParsedData> for RootValue<'r> {
    fn from(v: &'r ParsedData) -> Self {
        RootValue(Part::Parsed(v))
    }
}

impl<'r> From<&'r DataValue<'r>> for RootValue<'r> {
    fn from(v: &'r DataValue<'r>) -> Self {
        RootValue(Part::Arena(v))
    }
}

impl<'r> RootValue<'r> {
    /// The part as an arena value: leaves borrowed, spines built in
    /// `arena`, exactly as the part's own input impl does it.
    fn view_in<'a>(self, arena: &'a Bump) -> DataValue<'a>
    where
        'r: 'a,
    {
        match self.0 {
            Part::Owned(v) => v.view_in(arena),
            #[cfg(feature = "serde_json")]
            Part::Json(v) => crate::arena::value_to_data(v, arena),
            Part::Parsed(v) => *v.value(),
            Part::Arena(v) => *v,
        }
    }

    /// [`Self::view_in`], keeping only what `under` reads.
    fn project_in<'a>(self, under: &crate::projection::Projection, arena: &'a Bump) -> DataValue<'a>
    where
        'r: 'a,
    {
        match self.0 {
            Part::Owned(v) => under.owned(v, arena),
            #[cfg(feature = "serde_json")]
            Part::Json(v) => under.serde(v, arena),
            // Already in an arena: kept as is, at no cost.
            Part::Parsed(v) => *v.value(),
            Part::Arena(v) => *v,
        }
    }

    fn to_owned_value(self) -> OwnedDataValue {
        match self.0 {
            Part::Owned(v) => v.clone(),
            #[cfg(feature = "serde_json")]
            Part::Json(v) => crate::serde_bridge::owned_from_serde(v),
            Part::Parsed(v) => v.value().to_owned(),
            Part::Arena(v) => v.to_owned(),
        }
    }
}

impl<'r> Roots<'r> {
    /// No roots: evaluates as an empty object.
    pub fn new() -> Self {
        Roots { parts: Vec::new() }
    }

    /// Add `value` under `name`, or replace the value already under it.
    #[must_use = "`root` returns the extended `Roots`"]
    pub fn root(mut self, name: &'r str, value: impl Into<RootValue<'r>>) -> Self {
        self.insert(name, value);
        self
    }

    /// [`Self::root`] in place.
    pub fn insert(&mut self, name: &'r str, value: impl Into<RootValue<'r>>) {
        let value = value.into();
        match self.parts.iter_mut().find(|(n, _)| *n == name) {
            Some((_, slot)) => *slot = value,
            None => self.parts.push((name, value)),
        }
    }

    /// The names, in order.
    pub fn names(&self) -> impl Iterator<Item = &'r str> + '_ {
        self.parts.iter().map(|(name, _)| *name)
    }

    /// Number of roots.
    pub fn len(&self) -> usize {
        self.parts.len()
    }

    /// Whether there are no roots.
    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }

    /// The combined object, built in `arena`. One object node plus each
    /// part's own view.
    fn view_in<'a>(&self, arena: &'a Bump) -> &'a DataValue<'a>
    where
        'r: 'a,
    {
        let fields = arena.alloc_slice_fill_iter(
            self.parts
                .iter()
                .map(|(name, value)| (*name, value.view_in(arena))),
        );
        arena.alloc(DataValue::Object(fields))
    }

    /// The view a rule compiled as `logic` evaluates on `engine`:
    /// projected when the rule's reads allow it, whole otherwise.
    #[inline]
    fn arena_for<'a>(
        &self,
        logic: &crate::Logic,
        engine: &crate::Engine,
        arena: &'a Bump,
    ) -> &'a DataValue<'a>
    where
        'r: 'a,
    {
        match logic.projection_for(engine) {
            Some(projection) => self.projected_in(projection, arena),
            None => self.view_in(arena),
        }
    }

    /// [`Self::view_in`] for a rule with read projection `projection`:
    /// only the roots it reads, each projected.
    fn projected_in<'a>(
        &self,
        projection: &crate::projection::Projection,
        arena: &'a Bump,
    ) -> &'a DataValue<'a>
    where
        'r: 'a,
    {
        let mut kept = bumpalo::collections::Vec::with_capacity_in(self.parts.len(), arena);
        for (name, value) in &self.parts {
            if let Some(under) = projection.under(name) {
                kept.push((*name, value.project_in(under, arena)));
            }
        }
        arena.alloc(DataValue::Object(kept.into_bump_slice()))
    }

    fn to_owned_value(&self) -> OwnedDataValue {
        OwnedDataValue::Object(
            self.parts
                .iter()
                .map(|(name, value)| (name.to_string(), value.to_owned_value()))
                .collect(),
        )
    }
}

impl<'r, V: Into<RootValue<'r>>, const N: usize> From<[(&'r str, V); N]> for Roots<'r> {
    fn from(parts: [(&'r str, V); N]) -> Self {
        parts.into_iter().collect()
    }
}

impl<'r, V: Into<RootValue<'r>>> FromIterator<(&'r str, V)> for Roots<'r> {
    fn from_iter<I: IntoIterator<Item = (&'r str, V)>>(iter: I) -> Self {
        let mut roots = Roots::new();
        for (name, value) in iter {
            roots.insert(name, value);
        }
        roots
    }
}

impl<'r, V: Into<RootValue<'r>>> Extend<(&'r str, V)> for Roots<'r> {
    fn extend<I: IntoIterator<Item = (&'r str, V)>>(&mut self, iter: I) {
        for (name, value) in iter {
            self.insert(name, value);
        }
    }
}

// ── input impls ─────────────────────────────────────────────────────────

impl sealed::Sealed for &Roots<'_> {}
impl<'a, 'r: 'a> EvalInput<'a> for &'a Roots<'r> {
    #[inline]
    fn into_arena_value(self, arena: &'a Bump) -> Result<&'a DataValue<'a>> {
        Ok(self.view_in(arena))
    }

    #[inline]
    fn into_arena_for(
        self,
        logic: &crate::Logic,
        engine: &crate::Engine,
        arena: &'a Bump,
    ) -> Result<&'a DataValue<'a>> {
        Ok(self.arena_for(logic, engine, arena))
    }
}

impl sealed::Sealed for Roots<'_> {}
impl<'a, 'r: 'a> EvalInput<'a> for Roots<'r> {
    #[inline]
    fn into_arena_value(self, arena: &'a Bump) -> Result<&'a DataValue<'a>> {
        Ok(self.view_in(arena))
    }

    #[inline]
    fn into_arena_for(
        self,
        logic: &crate::Logic,
        engine: &crate::Engine,
        arena: &'a Bump,
    ) -> Result<&'a DataValue<'a>> {
        Ok(self.arena_for(logic, engine, arena))
    }
}

impl sealed::LendArena for &Roots<'_> {
    #[inline]
    fn lend_arena<R>(
        self,
        f: impl for<'a> FnOnce(&'a DataValue<'a>, &'a Bump) -> Result<R>,
    ) -> Result<R> {
        let arena = sealed::one_shot_arena();
        let data = self.view_in(&arena);
        f(data, &arena)
    }
}
impl OwnedInput for &Roots<'_> {
    fn into_owned_input(self) -> Result<OwnedDataValue> {
        Ok(self.to_owned_value())
    }
}

impl sealed::LendArena for Roots<'_> {
    #[inline]
    fn lend_arena<R>(
        self,
        f: impl for<'a> FnOnce(&'a DataValue<'a>, &'a Bump) -> Result<R>,
    ) -> Result<R> {
        (&self).lend_arena(f)
    }
}
impl OwnedInput for Roots<'_> {
    fn into_owned_input(self) -> Result<OwnedDataValue> {
        Ok(self.to_owned_value())
    }
}
