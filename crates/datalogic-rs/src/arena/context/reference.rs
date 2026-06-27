use super::frame::ContextFrame;
use crate::arena::value::DataValue;

/// Reference to an arena context frame (either a stack frame or the root).
pub(crate) enum ContextRef<'a, 'ctx> {
    Frame(&'ctx ContextFrame<'a>),
    /// Root carries the original input as `&'a DataValue<'a>`, deep-converted
    /// from a `&Value` at API entry or supplied directly by arena-native
    /// callers.
    Root(&'a DataValue<'a>),
}

impl<'a, 'ctx> ContextRef<'a, 'ctx> {
    /// The data value this reference points at — the frame's data for a
    /// `Frame`, or the root input for `Root`. Collapses the `Frame`/`Root`
    /// match that call sites would otherwise hand-roll.
    #[inline]
    pub(crate) fn data(&self) -> &'a DataValue<'a> {
        match self {
            Self::Frame(f) => f.data(),
            Self::Root(av) => av,
        }
    }

    #[inline]
    pub(crate) fn get_index(&self) -> Option<usize> {
        match self {
            Self::Frame(f) => f.get_index(),
            _ => None,
        }
    }

    #[inline]
    pub(crate) fn get_key(&self) -> Option<&'a str> {
        match self {
            Self::Frame(f) => f.get_key(),
            _ => None,
        }
    }

    #[cfg(all(test, feature = "serde_json"))]
    #[inline]
    pub(super) fn root_data(&self) -> Option<&'a DataValue<'a>> {
        match self {
            Self::Root(av) => Some(*av),
            Self::Frame(_) => None,
        }
    }

    #[cfg(all(test, feature = "serde_json"))]
    #[inline]
    pub(super) fn frame_data(&self) -> Option<&'a DataValue<'a>> {
        match self {
            Self::Frame(f) => Some(f.data()),
            Self::Root(_) => None,
        }
    }
}
