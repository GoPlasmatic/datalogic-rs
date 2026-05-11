use crate::arena::value::DataValue;

/// A single frame in the arena-mode context stack.
#[derive(Clone, Copy)]
pub(crate) enum ContextFrame<'a> {
    Indexed {
        data: &'a DataValue<'a>,
        index: usize,
    },
    Keyed {
        data: &'a DataValue<'a>,
        index: usize,
        key: &'a str,
    },
    Reduce {
        current: &'a DataValue<'a>,
        accumulator: &'a DataValue<'a>,
    },
    Data(&'a DataValue<'a>),
}

impl<'a> ContextFrame<'a> {
    #[inline]
    pub(crate) fn data(&self) -> &'a DataValue<'a> {
        match self {
            Self::Indexed { data, .. } | Self::Keyed { data, .. } | Self::Data(data) => data,
            Self::Reduce { current, .. } => current,
        }
    }

    #[inline]
    pub(crate) fn get_index(&self) -> Option<usize> {
        match self {
            Self::Indexed { index, .. } | Self::Keyed { index, .. } => Some(*index),
            _ => None,
        }
    }

    #[inline]
    pub(crate) fn get_key(&self) -> Option<&'a str> {
        match self {
            Self::Keyed { key, .. } => Some(key),
            _ => None,
        }
    }

    #[inline]
    pub(crate) fn get_reduce_current(&self) -> Option<&'a DataValue<'a>> {
        match self {
            Self::Reduce { current, .. } => Some(current),
            _ => None,
        }
    }

    #[inline]
    pub(crate) fn get_reduce_accumulator(&self) -> Option<&'a DataValue<'a>> {
        match self {
            Self::Reduce { accumulator, .. } => Some(accumulator),
            _ => None,
        }
    }
}
