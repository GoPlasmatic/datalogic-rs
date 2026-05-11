use serde::ser::{Serialize, Serializer};

/// Internal storage for the breadcrumb on [`crate::Error`]. Hidden from the
/// public surface so the layout (currently a plain `Vec<u32>`) can evolve
/// (smallvec, inline buffer, deferred-grow) without an API change.
#[derive(Debug, Clone, Default)]
pub(crate) struct ErrorPath {
    inner: Vec<u32>,
}

impl ErrorPath {
    #[inline]
    pub(crate) fn new() -> Self {
        Self::default()
    }

    #[inline]
    pub(crate) fn as_slice(&self) -> &[u32] {
        &self.inner
    }
}

impl From<Vec<u32>> for ErrorPath {
    #[inline]
    fn from(inner: Vec<u32>) -> Self {
        Self { inner }
    }
}

impl PartialEq for ErrorPath {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl Eq for ErrorPath {}

impl Serialize for ErrorPath {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.inner.serialize(serializer)
    }
}
