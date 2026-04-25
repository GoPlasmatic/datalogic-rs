use serde_json::Value;
use std::sync::Arc;

/// A single frame in the context stack, optimized as an enum to minimize size.
///
/// Field usage analysis shows mutually exclusive patterns:
/// - 59% of frames use only data + index (array iteration)
/// - 18% use data + index + key (object iteration)
/// - 9% use reduce_current + reduce_accumulator (reduce operations)
/// - 14% use only data (error context, etc.)
pub enum ContextFrame {
    /// Array iteration frame — most common (data + index)
    Indexed { data: Value, index: usize },
    /// Object iteration frame (data + index + key)
    Keyed {
        data: Value,
        index: usize,
        key: String,
    },
    /// Reduce operation frame (current + accumulator)
    Reduce { current: Value, accumulator: Value },
    /// Simple data-only frame
    Data(Value),
}

impl ContextFrame {
    /// Get the data value for this frame
    #[inline]
    pub fn data(&self) -> &Value {
        match self {
            Self::Indexed { data, .. } | Self::Keyed { data, .. } | Self::Data(data) => data,
            Self::Reduce { current, .. } => current,
        }
    }

    /// Get the index if available
    #[inline]
    pub fn get_index(&self) -> Option<usize> {
        match self {
            Self::Indexed { index, .. } | Self::Keyed { index, .. } => Some(*index),
            _ => None,
        }
    }

    /// Get the key if available
    #[inline]
    pub fn get_key(&self) -> Option<&str> {
        match self {
            Self::Keyed { key, .. } => Some(key.as_str()),
            _ => None,
        }
    }

    /// Get the reduce "current" value if this is a reduce frame
    #[inline]
    pub fn get_reduce_current(&self) -> Option<&Value> {
        match self {
            Self::Reduce { current, .. } => Some(current),
            _ => None,
        }
    }

    /// Get the reduce "accumulator" value if this is a reduce frame
    #[inline]
    pub fn get_reduce_accumulator(&self) -> Option<&Value> {
        match self {
            Self::Reduce { accumulator, .. } => Some(accumulator),
            _ => None,
        }
    }
}

/// Reference to a context frame (either actual frame or root)
pub enum ContextFrameRef<'a> {
    /// Reference to an actual frame
    Frame(&'a ContextFrame),
    /// Reference to the root data
    Root(&'a Arc<Value>),
}

impl<'a> ContextFrameRef<'a> {
    /// Get the data value
    pub fn data(&self) -> &Value {
        match self {
            ContextFrameRef::Frame(frame) => frame.data(),
            ContextFrameRef::Root(root) => root,
        }
    }

    /// Get the index if available
    #[inline]
    pub fn get_index(&self) -> Option<usize> {
        match self {
            ContextFrameRef::Frame(frame) => frame.get_index(),
            ContextFrameRef::Root(_) => None,
        }
    }

    /// Get the key if available
    #[inline]
    pub fn get_key(&self) -> Option<&str> {
        match self {
            ContextFrameRef::Frame(frame) => frame.get_key(),
            ContextFrameRef::Root(_) => None,
        }
    }

    /// Get the reduce "current" value if this is a reduce frame
    #[inline]
    pub fn get_reduce_current(&self) -> Option<&Value> {
        match self {
            ContextFrameRef::Frame(frame) => frame.get_reduce_current(),
            ContextFrameRef::Root(_) => None,
        }
    }

    /// Get the reduce "accumulator" value if this is a reduce frame
    #[inline]
    pub fn get_reduce_accumulator(&self) -> Option<&Value> {
        match self {
            ContextFrameRef::Frame(frame) => frame.get_reduce_accumulator(),
            ContextFrameRef::Root(_) => None,
        }
    }

}

/// Context stack for nested evaluations
pub struct ContextStack {
    /// Immutable root data (never changes during evaluation)
    root: Arc<Value>,
    /// Stack of temporary frames (excludes root)
    frames: Vec<ContextFrame>,
    /// Breadcrumb of [`CompiledNode`](crate::CompiledNode) ids accumulated
    /// as errors unwind. The dispatch hub pushes the current node's id on
    /// each `Err` return; `try` snapshots and truncates the length around
    /// catches so swallowed errors don't pollute the trail.
    ///
    /// The path is leaf-first (deepest failure first, root last), which is
    /// a natural consequence of accumulating during unwind. Consumers that
    /// prefer root-first can reverse it.
    error_path: Vec<u32>,
}

impl ContextStack {
    /// Create a new context stack with Arc root data
    pub fn new(root: Arc<Value>) -> Self {
        Self {
            root,
            frames: Vec::new(),
            error_path: Vec::new(),
        }
    }

    /// Append `id` to the error breadcrumb. Called by the dispatch hub on
    /// every `Err` return so the trail grows from failure site up to root.
    #[inline]
    pub fn push_error_step(&mut self, id: u32) {
        self.error_path.push(id);
    }

    /// Snapshot the current breadcrumb length. Paired with
    /// [`truncate_error_path`] in catch-like operators (`try`) to drop any
    /// steps accumulated while evaluating an arm that ultimately succeeded.
    #[inline]
    pub fn error_path_len(&self) -> usize {
        self.error_path.len()
    }

    /// Truncate the breadcrumb back to the given length.
    #[inline]
    pub fn truncate_error_path(&mut self, len: usize) {
        self.error_path.truncate(len);
    }

    /// Take the accumulated breadcrumb, leaving the context with an empty
    /// trail. Used at public boundaries when attaching the path to a
    /// [`StructuredError`](crate::StructuredError).
    #[inline]
    pub fn take_error_path(&mut self) -> Vec<u32> {
        std::mem::take(&mut self.error_path)
    }

    /// Pushes a new context frame for nested evaluation (data only).
    pub fn push(&mut self, data: Value) {
        self.frames.push(ContextFrame::Data(data));
    }

    /// Pushes a frame with just an index (optimized path for array iteration).
    #[inline]
    pub fn push_with_index(&mut self, data: Value, index: usize) {
        self.frames.push(ContextFrame::Indexed { data, index });
    }

    /// Pushes a frame with both index and key (optimized path for object iteration).
    #[inline]
    pub fn push_with_key_index(&mut self, data: Value, index: usize, key: String) {
        self.frames.push(ContextFrame::Keyed { data, index, key });
    }

    /// Replaces data, index, and key in the top frame in-place (for object iteration).
    #[inline]
    pub fn replace_top_key_data(&mut self, data: Value, index: usize, key: String) {
        if let Some(frame) = self.frames.last_mut() {
            *frame = ContextFrame::Keyed { data, index, key };
        }
    }

    /// Takes the data from the top frame, replacing it with Null.
    #[inline]
    pub fn take_top_data(&mut self) -> Value {
        if let Some(frame) = self.frames.last_mut() {
            match frame {
                ContextFrame::Indexed { data, .. }
                | ContextFrame::Keyed { data, .. }
                | ContextFrame::Data(data) => std::mem::replace(data, Value::Null),
                ContextFrame::Reduce { current, .. } => std::mem::replace(current, Value::Null),
            }
        } else {
            Value::Null
        }
    }

    /// Replaces the data and index in the top frame in-place.
    #[inline]
    pub fn replace_top_data(&mut self, data: Value, index: usize) {
        if let Some(frame) = self.frames.last_mut() {
            *frame = ContextFrame::Indexed { data, index };
        }
    }

    /// Pushes a reduce frame with "current" and "accumulator" values.
    #[inline]
    pub fn push_reduce(&mut self, current: Value, accumulator: Value) {
        self.frames.push(ContextFrame::Reduce {
            current,
            accumulator,
        });
    }

    /// Replaces current and accumulator values in the top reduce frame in-place.
    #[inline]
    pub fn replace_reduce_data(&mut self, current: Value, accumulator: Value) {
        if let Some(frame) = self.frames.last_mut() {
            *frame = ContextFrame::Reduce {
                current,
                accumulator,
            };
        }
    }

    /// Pops the current context frame from the stack.
    pub fn pop(&mut self) -> Option<ContextFrame> {
        self.frames.pop()
    }

    /// Accesses data at a context level relative to current.
    ///
    /// # Arguments
    ///
    /// * `level` - The number of levels to traverse up the context stack
    ///   - 0: returns the current context
    ///   - N (positive or negative): goes up N levels from current
    pub fn get_at_level(&self, level: isize) -> Option<ContextFrameRef<'_>> {
        let levels_up = level.unsigned_abs();

        if levels_up == 0 {
            return Some(self.current());
        }

        let frame_count = self.frames.len();

        if levels_up >= frame_count {
            return Some(ContextFrameRef::Root(&self.root));
        }

        let target_index = frame_count - levels_up;
        self.frames.get(target_index).map(ContextFrameRef::Frame)
    }

    /// Get the current context frame (top of stack)
    pub fn current(&self) -> ContextFrameRef<'_> {
        if let Some(frame) = self.frames.last() {
            ContextFrameRef::Frame(frame)
        } else {
            ContextFrameRef::Root(&self.root)
        }
    }

    /// Get the root context frame
    pub fn root(&self) -> ContextFrameRef<'_> {
        ContextFrameRef::Root(&self.root)
    }

    /// Borrow the root data directly. Used by arena dispatch to obtain a
    /// `&Value` reference whose lifetime matches the context (and therefore
    /// the in-flight `Arc<Value>` it owns), avoiding a redundant `Arc::clone`.
    #[inline]
    pub(crate) fn root_data(&self) -> &Value {
        &self.root
    }

    /// Get the current depth (number of frames)
    pub fn depth(&self) -> usize {
        self.frames.len()
    }
}
