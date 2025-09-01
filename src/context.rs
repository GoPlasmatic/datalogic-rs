use crate::arena::CustomOperatorRegistry;
use crate::value::DataValue;
use smallvec::SmallVec;

/// Metadata associated with each context level during iteration
#[derive(Clone)]
pub enum ContextMetadata<'a> {
    /// Array iteration index
    Index(usize),
    /// Object iteration key  
    Key(&'a str),
}

#[derive(Clone)]
pub struct EvalContext<'a> {
    context_stack: SmallVec<[&'a DataValue<'a>; 8]>,
    /// Stores iteration metadata (index for arrays, key for objects)
    metadata_stack: SmallVec<[Option<ContextMetadata<'a>>; 8]>,
    /// Registry of custom operators available during evaluation
    custom_operators: &'a CustomOperatorRegistry,
}

impl<'a> EvalContext<'a> {
    #[inline]
    pub fn new(root: &'a DataValue<'a>, custom_operators: &'a CustomOperatorRegistry) -> Self {
        let mut stack = SmallVec::new();
        stack.push(root);
        let mut metadata_stack = SmallVec::new();
        metadata_stack.push(None);
        Self {
            context_stack: stack,
            metadata_stack,
            custom_operators,
        }
    }

    #[inline]
    pub fn current(&self) -> &'a DataValue<'a> {
        self.context_stack
            .last()
            .expect("Context stack should never be empty")
    }

    #[inline]
    pub fn root(&self) -> &'a DataValue<'a> {
        self.context_stack
            .first()
            .expect("Context stack should never be empty")
    }

    #[inline]
    pub fn push(&self, value: &'a DataValue<'a>) -> Self {
        let mut new_stack = self.context_stack.clone();
        new_stack.push(value);
        let mut new_metadata_stack = self.metadata_stack.clone();
        new_metadata_stack.push(None);
        Self {
            context_stack: new_stack,
            metadata_stack: new_metadata_stack,
            custom_operators: self.custom_operators,
        }
    }

    #[inline]
    pub fn depth(&self) -> usize {
        self.context_stack.len()
    }

    #[inline]
    pub fn at_depth(&self, depth: usize) -> Option<&'a DataValue<'a>> {
        let len = self.context_stack.len();
        if depth >= len {
            return None;
        }
        Some(self.context_stack[len - 1 - depth])
    }

    #[inline]
    pub fn push_with_index(&self, value: &'a DataValue<'a>, index: usize) -> Self {
        let mut new_stack = self.context_stack.clone();
        new_stack.push(value);
        let mut new_metadata_stack = self.metadata_stack.clone();
        new_metadata_stack.push(Some(ContextMetadata::Index(index)));
        Self {
            context_stack: new_stack,
            metadata_stack: new_metadata_stack,
            custom_operators: self.custom_operators,
        }
    }

    #[inline]
    pub fn current_index(&self) -> Option<usize> {
        self.metadata_stack.last().and_then(|opt| {
            opt.as_ref().and_then(|meta| match meta {
                ContextMetadata::Index(idx) => Some(*idx),
                _ => None,
            })
        })
    }

    #[inline]
    pub fn index_at_depth(&self, depth: usize) -> Option<usize> {
        let len = self.metadata_stack.len();
        if depth >= len {
            return None;
        }
        self.metadata_stack[len - 1 - depth]
            .as_ref()
            .and_then(|meta| match meta {
                ContextMetadata::Index(idx) => Some(*idx),
                _ => None,
            })
    }

    #[inline]
    pub fn push_with_key(&self, value: &'a DataValue<'a>, key: &'a str) -> Self {
        let mut new_stack = self.context_stack.clone();
        new_stack.push(value);
        let mut new_metadata_stack = self.metadata_stack.clone();
        new_metadata_stack.push(Some(ContextMetadata::Key(key)));
        Self {
            context_stack: new_stack,
            metadata_stack: new_metadata_stack,
            custom_operators: self.custom_operators,
        }
    }

    #[inline]
    pub fn push_with_index_and_key(
        &self,
        value: &'a DataValue<'a>,
        _index: usize,
        key: &'a str,
    ) -> Self {
        // For object iteration, we only track the key (index is not needed)
        self.push_with_key(value, key)
    }

    #[inline]
    pub fn current_key(&self) -> Option<&'a str> {
        self.metadata_stack.last().and_then(|opt| {
            opt.as_ref().and_then(|meta| match meta {
                ContextMetadata::Key(k) => Some(*k),
                _ => None,
            })
        })
    }

    #[inline]
    pub fn key_at_depth(&self, depth: usize) -> Option<&'a str> {
        let len = self.metadata_stack.len();
        if depth >= len {
            return None;
        }
        self.metadata_stack[len - 1 - depth]
            .as_ref()
            .and_then(|meta| match meta {
                ContextMetadata::Key(k) => Some(*k),
                _ => None,
            })
    }

    #[inline]
    pub fn with_item(&self, item: &'a DataValue<'a>) -> Self {
        self.push(item)
    }

    #[inline]
    pub fn context_at_scope(&self, scope_jump: usize) -> &'a DataValue<'a> {
        self.at_depth(scope_jump).unwrap_or_else(|| self.root())
    }

    /// Get the custom operators registry
    #[inline]
    pub fn custom_operators(&self) -> &'a CustomOperatorRegistry {
        self.custom_operators
    }
}
