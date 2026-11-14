use std::{fmt, marker::PhantomData};

mod sealed {
    pub trait Sealed {}
}

/// An arena index, local to its containing module or function, not a runtime ID.
pub trait ArenaId: sealed::Sealed + Copy {
    fn from_u32(index: u32) -> Self;
    fn as_u32(self) -> u32;
}

macro_rules! ids {
    ($($name:ident),+ $(,)?) => {$(
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u32);

        impl $name {
            /// Construct an index, including forward or deliberately invalid references.
            pub const fn from_u32(index: u32) -> Self { Self(index) }
            pub const fn as_u32(self) -> u32 { self.0 }
        }

        impl sealed::Sealed for $name {}
        impl ArenaId for $name {
            fn from_u32(index: u32) -> Self { Self(index) }
            fn as_u32(self) -> u32 { self.0 }
        }
    )+};
}

// Module-wide IDs, including ScopeId used by the shared type table.
ids!(FuncId, TypeId, DeclId, ScopeId, FileId, SummaryId, ProtocolId);
// Function-local IDs.
ids!(CleanupCloseId, CleanupPlanId, CleanupScheduleId, RootMapId, CleanupOrderProofId);
ids!(BlockId, InstId, ValueId, BindingId, BoundaryId, DepExprId, KeyExprId, SelectionId, CreationSiteId);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArenaError {
    CapacityExceeded,
    InvalidIndex { index: u32, len: usize },
}

impl fmt::Display for ArenaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CapacityExceeded => f.write_str("arena exceeds the u32 index space"),
            Self::InvalidIndex { index, len } => {
                write!(f, "arena index {index} is out of bounds for length {len}")
            }
        }
    }
}

impl std::error::Error for ArenaError {}

/// Append-only storage: indices are never removed, compacted or reused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Arena<I, T> {
    items: Vec<T>,
    marker: PhantomData<fn() -> I>,
}

impl<I, T> Default for Arena<I, T> {
    fn default() -> Self {
        Self { items: Vec::new(), marker: PhantomData }
    }
}

impl<I: ArenaId, T> Arena<I, T> {
    pub fn new() -> Self { Self::default() }

    pub fn alloc(&mut self, value: T) -> Result<I, ArenaError> {
        self.alloc_with_limit(value, u64::from(u32::MAX) + 1)
    }

    // A small limit lets unit tests exercise the same guard without huge allocations.
    fn alloc_with_limit(&mut self, value: T, limit: u64) -> Result<I, ArenaError> {
        if self.items.len() as u64 >= limit {
            return Err(ArenaError::CapacityExceeded);
        }
        let index = u32::try_from(self.items.len()).map_err(|_| ArenaError::CapacityExceeded)?;
        self.items.push(value);
        Ok(I::from_u32(index))
    }

    pub fn get(&self, id: I) -> Result<&T, ArenaError> {
        self.items.get(id.as_u32() as usize).ok_or(ArenaError::InvalidIndex {
            index: id.as_u32(), len: self.len(),
        })
    }

    pub fn get_mut(&mut self, id: I) -> Result<&mut T, ArenaError> {
        let len = self.len();
        self.items.get_mut(id.as_u32() as usize).ok_or(ArenaError::InvalidIndex {
            index: id.as_u32(), len,
        })
    }

    pub fn len(&self) -> usize { self.items.len() }
    pub fn is_empty(&self) -> bool { self.items.is_empty() }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = (I, &T)> {
        // Allocation guarantees that every occupied index fits in u32.
        self.items.iter().enumerate().map(|(i, value)| (I::from_u32(i as u32), value))
    }

    pub fn iter_mut(&mut self) -> impl ExactSizeIterator<Item = (I, &mut T)> {
        self.items.iter_mut().enumerate().map(|(i, value)| (I::from_u32(i as u32), value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocation_and_checked_access() -> Result<(), ArenaError> {
        let mut arena = Arena::<ValueId, _>::new();
        assert!(arena.is_empty());
        let first = arena.alloc("first")?;
        let second = arena.alloc("second")?;
        assert_eq!((first.as_u32(), second.as_u32()), (0, 1));
        assert_eq!(arena.get(first)?, &"first");
        *arena.get_mut(second)? = "changed";
        assert_eq!(arena.get(second)?, &"changed");
        let bad = ValueId::from_u32(2);
        let error = ArenaError::InvalidIndex { index: 2, len: 2 };
        assert_eq!(arena.get(bad), Err(error));
        assert_eq!(arena.get_mut(bad), Err(error));
        assert!(arena.get(ValueId::from_u32(u32::MAX)).is_err());
        assert_eq!(arena.alloc("third")?.as_u32(), 2);
        assert_eq!(arena.iter().map(|(id, _)| id.as_u32()).collect::<Vec<_>>(), [0, 1, 2]);
        Ok(())
    }

    #[test]
    fn overflow_does_not_allocate_or_reuse_indices() -> Result<(), ArenaError> {
        let mut arena = Arena::<BlockId, _>::new();
        assert_eq!(arena.alloc_with_limit(10, 0), Err(ArenaError::CapacityExceeded));
        assert!(arena.is_empty());
        assert_eq!(arena.alloc_with_limit(10, 2)?.as_u32(), 0);
        assert_eq!(arena.alloc_with_limit(20, 2)?.as_u32(), 1);
        assert_eq!(arena.alloc_with_limit(30, 2), Err(ArenaError::CapacityExceeded));
        assert_eq!(arena.len(), 2);
        assert_eq!(*arena.get(BlockId::from_u32(1))?, 20);
        assert_eq!(arena.alloc(40)?.as_u32(), 2);
        Ok(())
    }
}
