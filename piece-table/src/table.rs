//! Provides an implementation for `Table`.
//! The `Table` is responsible for regulating the access to the values stored
use std::{
    collections::LinkedList,
    fmt::Debug,
    ops::{Deref, DerefMut},
    sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

/// A wrapper struct around a type T
/// It allows for a list of T's to be read and mutated concurrently and has methods for locking
/// down the entire method for reading and reordering purposes
pub struct Table<T> {
    #[allow(clippy::linkedlist)]
    inner: Arc<RwLock<LinkedList<InnerTable<T>>>>,
    state: Arc<RwLock<TableState>>,
}

impl<T> Debug for Table<T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Table")
            .field("inner", &*self.inner.read().unwrap())
            .field("state", &self.state)
            .finish()
    }
}

/// The error type for locking operations
#[derive(Debug)]
pub enum LockError {
    /// There is already an incompatible lock on the element you want to lock
    FailedLock,
    /// There is already an incompatible lock on the element you want to lock
    Poisoned,
}

impl<T> From<std::sync::PoisonError<T>> for LockError {
    fn from(value: std::sync::PoisonError<T>) -> Self {
        println!("{value:?}");
        Self::Poisoned
    }
}

impl<T> Table<T> {
    #[must_use]
    /// creates a new Table from a builder
    pub fn new(builder: InnerTableBuilder<T>) -> Self {
        let (inner, state) = builder.build();
        Self {
            inner: Arc::new(RwLock::new(inner)),
            state,
        }
    }
    /// Returns a reading lock on the entire linked list
    /// This means that
    /// - Elements of the list cannot be modified
    /// - The order of listelements cannot be modified
    /// # Errors
    /// - There is already a mutable lock on an element
    /// - There is already a mutable lock on the full list
    pub fn read_full(&self) -> Result<TableReader<T>, LockError> {
        self.state.write()?.lock_full()?;
        Ok(TableReader {
            val: Arc::clone(&self.inner),
            state: self.state.clone(),
        })
    }

    /// Returns a writing lock on the order of the linked list
    /// This menas that
    /// - Elemens of the list *can* still be modified
    /// - No reading lock can be made on the entire linked list
    /// # Errors
    /// - There is already a writing lock on the list
    /// - There is already a reading lock on the list
    pub fn write_full(&self) -> Result<TableWriter<T>, LockError> {
        self.state.write()?.lock_full_mut()?;
        Ok(TableWriter {
            val: Arc::clone(&self.inner),
            state: self.state.clone(),
        })
    }

    /// Creates a clone of the state of this `Table`
    #[must_use]
    pub fn state(&self) -> Arc<RwLock<TableState>> {
        Arc::clone(&self.state)
    }
}

impl<T> FromIterator<T> for Table<T> {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let state = Arc::new(RwLock::new(TableState::Unshared));
        Self {
            inner: Arc::new(RwLock::new(
                iter.into_iter()
                    .map(|x| InnerTable::new(x, Arc::clone(&state)))
                    .collect(),
            )),
            state,
        }
    }
}

/// A state machine to control what kinds of locks can be made at what time
#[derive(Debug)]
pub enum TableState {
    /// There are no referenses to the list
    Unshared,
    /// The entire list only has immutable borrows. (full list borrows, single item immutable borrows)
    Shared((usize, usize)),
    /// The entire list isn't borrowed. (single item immutable bororws, single item mutable borrows)
    SharedMuts((usize, usize)),
    /// The entire list is mutably borrowed (immutable element borrows, mutable element borrows)
    Exclusive((usize, usize)),
}

impl TableState {
    pub(crate) fn lock_single(&mut self) {
        match self {
            Self::Shared((0, _)) => unreachable!(),

            Self::Unshared => *self = Self::SharedMuts((1, 0)),

            Self::Shared((1.., ref mut immuts))
            | Self::Exclusive((ref mut immuts, _))
            | Self::SharedMuts((ref mut immuts, _)) => *immuts += 1,
        }
    }

    pub(crate) fn drop_single(&mut self) {
        match self {
            Self::Unshared
            | Self::Shared((0, _) | (_, 0))
            | Self::SharedMuts((0, _))
            | Self::Exclusive((0, _)) => unreachable!(),

            Self::SharedMuts((ref mut immuts @ 1.., _))
            | Self::Exclusive((ref mut immuts @ 1.., _))
            | Self::Shared((1.., ref mut immuts @ 1..)) => *immuts -= 1,
        }
    }

    pub(crate) fn lock_single_mut(&mut self) -> Result<(), LockError> {
        match self {
            Self::Shared((0, _)) => unreachable!(),

            Self::Unshared => *self = Self::SharedMuts((0, 1)),
            Self::Shared(_) => return Err(LockError::FailedLock),
            Self::SharedMuts((_, ref mut muts)) | Self::Exclusive((_, ref mut muts)) => *muts += 1,
        };
        Ok(())
    }

    pub(crate) fn drop_single_mut(&mut self) {
        match self {
            Self::SharedMuts((_, 0))
            | Self::Exclusive((_, 0))
            | Self::Unshared
            | Self::Shared(_) => unreachable!(),

            Self::SharedMuts((_, ref mut muts @ 1..))
            | Self::Exclusive((_, ref mut muts @ 1..)) => *muts -= 1,
        };
    }

    pub(crate) fn lock_full(&mut self) -> Result<(), LockError> {
        match self {
            Self::Shared((0, _)) => unreachable!(),

            Self::SharedMuts((_, 1..)) | Self::Exclusive(_) => return Err(LockError::FailedLock),

            Self::Unshared => *self = Self::Shared((1, 0)),
            Self::Shared((ref mut amount @ 1.., _)) => *amount += 1,
            Self::SharedMuts((amount, _)) => *self = Self::Shared((1, *amount)),
        };
        Ok(())
    }

    pub(crate) fn drop_full(&mut self) {
        match self {
            Self::Shared((0, _)) | Self::Unshared | Self::Exclusive(_) | Self::SharedMuts(_) => {
                unreachable!()
            }

            Self::Shared((1, 0)) => *self = Self::Unshared,
            Self::Shared((1, amount)) => *self = Self::SharedMuts((*amount, 0)),
            Self::Shared((ref mut amount @ 1.., _)) => *amount -= 1,
        };
    }

    pub(crate) fn lock_full_mut(&mut self) -> Result<(), LockError> {
        match self {
            Self::Unshared => *self = Self::Exclusive((0, 0)),
            Self::SharedMuts((immuts, muts)) => *self = Self::Exclusive((*immuts, *muts)),
            Self::Shared((1.., _)) | Self::Exclusive((_, _)) => return Err(LockError::FailedLock),
            Self::Shared((0, _)) => unreachable!(),
        };
        Ok(())
    }

    pub(crate) fn drop_full_mut(&mut self) {
        match self {
            Self::Unshared | Self::SharedMuts(_) | Self::Shared(_) => unreachable!(),
            Self::Exclusive((immuts, muts)) => *self = Self::SharedMuts((*immuts, *muts)),
        }
    }
}

/// Represents a mutable lock on order of the full list
/// This means that
/// - No readable lock can be created on the entire list
/// - Elements within the list can still be mutated
/// - Elements within the list can still be read
pub struct TableWriter<T> {
    #[allow(clippy::linkedlist)]
    val: Arc<RwLock<LinkedList<InnerTable<T>>>>,
    state: Arc<RwLock<TableState>>,
}

impl<T> TableWriter<T> {
    /// locks down the list for reordering purposes
    /// This means that
    /// - You can't lock the entire list for reading/writing
    /// # Panics
    /// - The `RwLock` is poisoned
    pub fn write(&self) -> RwLockWriteGuard<'_, LinkedList<InnerTable<T>>> {
        self.val.write().unwrap()
    }
}

impl<T> Drop for TableWriter<T> {
    fn drop(&mut self) {
        self.state.write().unwrap().drop_full_mut();
    }
}

/// Represents a Lock on the entire list.
/// This means that
/// - No element of the list can be mutated
/// - The order of the elements cannot be mutated
pub struct TableReader<T> {
    #[allow(clippy::linkedlist)]
    val: Arc<RwLock<LinkedList<InnerTable<T>>>>,
    state: Arc<RwLock<TableState>>,
}

impl<T> TableReader<T> {
    #[allow(clippy::linkedlist)]
    /// Returns a reading lock on the List.
    /// Read the docs for `TableReader` for more
    /// # Panics
    /// the lock around the list has been poisoned
    pub fn read(&self) -> RwLockReadGuard<'_, LinkedList<InnerTable<T>>> {
        self.val.read().unwrap()
    }
}

impl<T> Drop for TableReader<T> {
    fn drop(&mut self) {
        self.state.write().unwrap().drop_full();
    }
}

/// Represents something that can lock an item within the `Table`
pub struct TableLocker<T> {
    value: Arc<RwLock<T>>,
    state: Arc<RwLock<TableState>>,
}

impl<T> TableLocker<T> {
    fn new(value: T, state: Arc<RwLock<TableState>>) -> Self {
        Self {
            value: Arc::new(RwLock::new(value)),
            state,
        }
    }
}

impl<T> TableLocker<T> {
    /// Actually locks the item and returns a Reader
    /// This means that:
    /// - no one can mutate the value you are reading
    /// # Panics
    /// - The state is poisoned
    /// - The value you are trying to access is poisoned
    #[must_use]
    pub fn read(&self) -> TableLockReader<T> {
        self.state.write().unwrap().lock_single();
        TableLockReader {
            value: self.value.read().unwrap(),
            state: Arc::clone(&self.state),
        }
    }

    /// Actually locks the item and returns a `TableLockWriter`
    /// This means that:
    /// - no one can read the value you are reading
    /// # Errors
    /// - There is already a reading lock on the list
    pub fn write(&self) -> Result<TableLockWriter<T>, LockError> {
        self.state.write()?.lock_single_mut()?;
        Ok(TableLockWriter {
            value: self.value.write()?,
            state: Arc::clone(&self.state),
        })
    }
}

/// Represents a reading lock on an item within the linked list
/// This means that
/// - This item cannot be mutated by anything else
pub struct TableLockReader<'a, T> {
    value: RwLockReadGuard<'a, T>,
    state: Arc<RwLock<TableState>>,
}

impl<T> Deref for TableLockReader<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> Drop for TableLockReader<'_, T> {
    fn drop(&mut self) {
        self.state.write().unwrap().drop_single();
    }
}

/// Represents a write-lock on an item within the list.
/// This means that
/// - This item cannot be mutated by anything else
/// - This item cannot be read by anything else
/// - The entire list cannot be read
pub struct TableLockWriter<'a, T> {
    value: RwLockWriteGuard<'a, T>,
    state: Arc<RwLock<TableState>>,
}

impl<T> Deref for TableLockWriter<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> DerefMut for TableLockWriter<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

impl<T> Drop for TableLockWriter<'_, T> {
    fn drop(&mut self) {
        self.state.write().unwrap().drop_single_mut();
    }
}

/// The Inner values of the linked list. These are effectively wrappers around &strs.
pub struct InnerTable<T> {
    inner: Arc<TableLocker<T>>,
    /// The client whichs buffer is being referred to a `None` value signifies that it is from the
    /// original buffer
    state: Arc<RwLock<TableState>>,
}

impl<T> Clone for InnerTable<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            state: Arc::clone(&self.state),
        }
    }
}

impl<T> Debug for InnerTable<T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InnerTable")
            .field("inner", &self.inner.read().value)
            .field("state", &self.state)
            .finish()
    }
}

impl<T> InnerTable<T> {
    #[must_use]
    /// creates a builder for an innertable
    pub fn builder() -> InnerTableBuilder<T> {
        InnerTableBuilder {
            inner: LinkedList::new(),
            state: Arc::new(RwLock::new(TableState::Unshared)),
        }
    }
    #[must_use]
    /// locks down the value within this `InnerLock` for writing
    pub fn read(&self) -> TableLockReader<T> {
        self.inner.read()
    }

    /// locks down the value within this `InnerLock` for writing
    /// # Errors
    /// - There is already a reading lock on this value
    /// - There is already a reading lock on the full list
    pub fn write(&self) -> Result<TableLockWriter<'_, T>, LockError> {
        self.inner.write()
    }

    /// Creates a new `InnerTable`. This can be used for insertion after having used `read_full`
    #[must_use]
    pub fn new(value: T, state: Arc<RwLock<TableState>>) -> Self {
        Self {
            inner: Arc::new(TableLocker::new(value, Arc::clone(&state))),
            state,
        }
    }
}

/// A builder for a `Table`
pub struct InnerTableBuilder<T> {
    /// The inner table being modified
    inner: LinkedList<InnerTable<T>>,
    /// The shared state
    state: Arc<RwLock<TableState>>,
}

impl<T> InnerTableBuilder<T> {
    /// returns its values so that it can be converted to a `Table`
    #[must_use]
    pub(crate) fn build(self) -> (LinkedList<InnerTable<T>>, Arc<RwLock<TableState>>) {
        (self.inner, self.state)
    }

    /// Appends a value to the linkedlist
    pub fn push(&mut self, x: T) {
        self.inner
            .push_back(InnerTable::new(x, Arc::clone(&self.state)));
    }
}
