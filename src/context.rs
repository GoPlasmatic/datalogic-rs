use serde_json::Value;
use std::borrow::Cow;
use std::collections::HashMap;

/// A single frame in the context stack
pub struct ContextFrame<'a> {
    /// The data value at this context level
    pub data: Cow<'a, Value>,
    /// Optional metadata for this frame (e.g., "index", "key" in map operations)
    pub metadata: Option<HashMap<String, Cow<'a, Value>>>,
}

/// Context stack for nested evaluations
pub struct ContextStack<'a> {
    /// Stack of context frames, with the root data at index 0
    frames: Vec<ContextFrame<'a>>,
}

impl<'a> ContextStack<'a> {
    /// Create a new context stack with root data
    pub fn new(root: Cow<'a, Value>) -> Self {
        Self {
            frames: vec![ContextFrame {
                data: root,
                metadata: None,
            }],
        }
    }

    /// Push a new context frame for nested evaluation
    pub fn push(&mut self, data: Cow<'a, Value>) {
        self.frames.push(ContextFrame {
            data,
            metadata: None,
        });
    }

    /// Push a frame with metadata (e.g., for map with index)
    pub fn push_with_metadata(
        &mut self,
        data: Cow<'a, Value>,
        metadata: HashMap<String, Cow<'a, Value>>,
    ) {
        self.frames.push(ContextFrame {
            data,
            metadata: Some(metadata),
        });
    }

    /// Pop the current context frame
    pub fn pop(&mut self) -> Option<ContextFrame<'a>> {
        // Never pop the root frame
        if self.frames.len() > 1 {
            self.frames.pop()
        } else {
            None
        }
    }

    /// Access data at a context level relative to current
    /// The sign is ignored - both positive and negative mean the same thing
    /// - 0: current context
    /// - 1 or -1: go up 1 level (parent)
    /// - 2 or -2: go up 2 levels (grandparent)
    /// - N or -N: go up N levels
    pub fn get_at_level(&self, level: isize) -> Option<&ContextFrame<'a>> {
        // Get absolute value - sign doesn't matter
        let levels_up = level.unsigned_abs();

        if levels_up == 0 {
            // 0 means current context
            return self.frames.last();
        }

        let current_index = self.frames.len() - 1;

        if levels_up > current_index {
            // Going up more levels than exist, return root
            return self.frames.first();
        }

        // Calculate target index by going up from current
        let target_index = current_index - levels_up;
        self.frames.get(target_index)
    }

    /// Get the current context frame (top of stack)
    pub fn current(&self) -> &ContextFrame<'a> {
        self.frames
            .last()
            .expect("Context stack should never be empty")
    }

    /// Get the root context frame
    pub fn root(&self) -> &ContextFrame<'a> {
        &self.frames[0]
    }

    /// Get the current depth (number of frames - 1)
    pub fn depth(&self) -> usize {
        self.frames.len() - 1
    }
}
