use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

// Interned strings for common metadata keys
static INDEX_KEY: OnceLock<String> = OnceLock::new();
static KEY_KEY: OnceLock<String> = OnceLock::new();

pub fn index_key() -> &'static String {
    INDEX_KEY.get_or_init(|| "index".to_string())
}

pub fn key_key() -> &'static String {
    KEY_KEY.get_or_init(|| "key".to_string())
}

/// A single frame in the context stack (for temporary/nested contexts)
pub struct ContextFrame {
    /// The data value at this context level
    pub data: Value,
    /// Optional metadata for this frame (e.g., "index", "key" in map operations)
    pub metadata: Option<HashMap<String, Value>>,
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
            ContextFrameRef::Frame(frame) => &frame.data,
            ContextFrameRef::Root(root) => root,
        }
    }

    /// Get the metadata (only available for frames)
    pub fn metadata(&self) -> Option<&HashMap<String, Value>> {
        match self {
            ContextFrameRef::Frame(frame) => frame.metadata.as_ref(),
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
}

impl ContextStack {
    /// Create a new context stack with Arc root data
    pub fn new(root: Arc<Value>) -> Self {
        Self {
            root,
            frames: Vec::new(),
        }
    }

    /// Pushes a new context frame for nested evaluation.
    ///
    /// Used by operators that need to create a nested data context without metadata.
    ///
    /// # Arguments
    ///
    /// * `data` - The data value for the new context frame
    pub fn push(&mut self, data: Value) {
        self.frames.push(ContextFrame {
            data,
            metadata: None,
        });
    }

    /// Pushes a frame with metadata for iteration operations.
    ///
    /// Used by array operators like `map`, `filter`, and `reduce` to provide
    /// iteration context including index and key information.
    ///
    /// # Arguments
    ///
    /// * `data` - The current item being processed
    /// * `metadata` - Iteration metadata (typically includes "index" and optionally "key")
    ///
    /// # Example
    ///
    /// During array iteration:
    /// ```rust,ignore
    /// let mut metadata = HashMap::new();
    /// metadata.insert("index".to_string(), json!(0));
    /// context.push_with_metadata(item_value, metadata);
    /// ```
    pub fn push_with_metadata(&mut self, data: Value, metadata: HashMap<String, Value>) {
        self.frames.push(ContextFrame {
            data,
            metadata: Some(metadata),
        });
    }

    /// Pops the current context frame from the stack.
    ///
    /// Restores the previous context after nested evaluation completes.
    /// Returns None if there are no frames to pop (root is never popped).
    ///
    /// # Returns
    ///
    /// The popped context frame, or None if the stack is empty.
    pub fn pop(&mut self) -> Option<ContextFrame> {
        // Only pop if there are frames (root is separate)
        self.frames.pop()
    }

    /// Accesses data at a context level relative to current.
    ///
    /// This method enables access to parent contexts during nested operations,
    /// which is essential for complex JSONLogic expressions.
    ///
    /// # Arguments
    ///
    /// * `level` - The number of levels to traverse up the context stack
    ///   - 0: returns the current context
    ///   - N (positive or negative): goes up N levels from current
    ///
    /// # Returns
    ///
    /// A reference to the context frame at the specified level,
    /// or None if the level exceeds the stack depth.
    ///
    /// # Note
    ///
    /// The sign of the level is ignored; both positive and negative values
    /// traverse up the stack the same way. This maintains backward compatibility
    /// with existing usage patterns.
    ///
    /// # Returns
    /// The context frame at the specified level, or the root if level exceeds stack depth
    pub fn get_at_level(&self, level: isize) -> Option<ContextFrameRef<'_>> {
        // Get absolute value - sign doesn't matter (for backward compatibility)
        let levels_up = level.unsigned_abs();

        if levels_up == 0 {
            // 0 means current context
            return Some(self.current());
        }

        let frame_count = self.frames.len();

        // If going up more levels than or equal to the total frame count,
        // we reach the root (since root is not in frames)
        if levels_up >= frame_count {
            return Some(ContextFrameRef::Root(&self.root));
        }

        // Calculate target index by going up from current
        let target_index = frame_count - levels_up;
        self.frames.get(target_index).map(ContextFrameRef::Frame)
    }

    /// Get the current context frame (top of stack)
    /// Returns a temporary frame for root if no frames are pushed
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

    /// Get the current depth (number of frames)
    pub fn depth(&self) -> usize {
        self.frames.len()
    }
}
