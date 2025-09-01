//! Arena reference type for owned or borrowed arenas.

use super::DataArena;

/// Reference to an arena that can be either owned or borrowed.
///
/// This allows DataLogic to either own its own arena (backward compatibility)
/// or borrow an external arena (for sharing compiled logic).
pub enum ArenaRef<'a> {
    /// Owned arena (backward compatibility)
    Owned(DataArena),
    /// Borrowed arena (for sharing)
    Borrowed(&'a DataArena),
}

impl<'a> ArenaRef<'a> {
    /// Get a reference to the arena with the proper lifetime
    /// For borrowed arenas, returns the borrowed lifetime 'a
    /// For owned arenas, returns the lifetime of self
    pub fn get(&self) -> &DataArena
    where
        Self: 'a,
    {
        match self {
            ArenaRef::Owned(arena) => arena,
            ArenaRef::Borrowed(arena) => arena,
        }
    }

    /// Get a reference to the arena (lifetime of &self)
    pub fn as_arena(&self) -> &DataArena {
        match self {
            ArenaRef::Owned(arena) => arena,
            ArenaRef::Borrowed(arena) => arena,
        }
    }

    /// Get a mutable reference (only for owned)
    pub fn as_mut(&mut self) -> Option<&mut DataArena> {
        match self {
            ArenaRef::Owned(arena) => Some(arena),
            ArenaRef::Borrowed(_) => None,
        }
    }

    /// Reset the arena if owned
    pub fn reset(&mut self) {
        if let ArenaRef::Owned(arena) = self {
            arena.reset();
        }
    }
}

impl<'a> std::ops::Deref for ArenaRef<'a> {
    type Target = DataArena;

    fn deref(&self) -> &Self::Target {
        self.as_arena()
    }
}

impl<'a> AsRef<DataArena> for ArenaRef<'a> {
    fn as_ref(&self) -> &DataArena {
        self.as_arena()
    }
}
