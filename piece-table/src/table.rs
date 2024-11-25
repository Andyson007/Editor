use std::{
    collections::LinkedList,
    fmt::Debug,
    ops::{Deref, DerefMut},
    sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

pub struct Table<T> {
    #[allow(clippy::linkedlist)]
    pub inner: Arc<RwLock<LinkedList<InnerTable<T>>>>,
    pub state: Arc<RwLock<TableState>>,
}

impl<T> Debug for Table<T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Table")
            .field(
                "inner",
                &self.inner.read().unwrap().front().unwrap().read().1.value,
            )
            .field("state", &self.state)
            .finish()
    }
}

#[derive(Debug)]
pub enum LockError {
    FailedLock,
}

impl<T> Table<T> {
    /// Returns a reading lock on the entire linked list
    /// This means that
    /// - Elements of the list cannot be modified
    /// - The order of listelements cannot be modified
    /// # Panics
    /// - The state has been poisoned
    /// # Errors
    /// - There is already a mutable lock on an element
    /// - There is already a mutable lock on the full list
    pub fn read_full(&self) -> Result<TableReader<T>, LockError> {
        match *self.state.write().unwrap() {
            ref mut x @ TableState::Unshared => *x = TableState::Shared((1, 0)),
            TableState::Shared((ref mut amount, _)) => *amount += 1,
            ref mut state @ TableState::SharedMuts((amount, 0)) => {
                *state = TableState::Shared((1, amount));
            }
            TableState::Exclusive(_) | TableState::SharedMuts(_) => {
                return Err(LockError::FailedLock)
            }
        };
        Ok(TableReader {
            val: Arc::clone(&self.inner),
            state: self.state.clone(),
        })
    }

    /// Returns a writing lock on the order of the linked list
    /// This menas that
    /// - Elemens of the list *can* still be modified
    /// - No reading lock can be made on the entire linked list
    /// # Panics
    /// - The state has been poisoned
    /// # Errors
    /// - There is already a writing lock on the list
    /// - There is already a reading lock on the list
    pub fn write_full(&self) -> Result<TableWriter<T>, LockError> {
        match *self.state.write().unwrap() {
            ref mut state @ TableState::Unshared => *state = TableState::Exclusive((0, 0)),
            ref mut state @ TableState::SharedMuts((immuts, muts)) => {
                *state = TableState::Exclusive((immuts, muts));
            }
            TableState::Exclusive(_) | TableState::Shared(_) => return Err(LockError::FailedLock),
        };
        Ok(TableWriter {
            val: Arc::clone(&self.inner),
            state: self.state.clone(),
        })
    }

    /// Creates a clone of the state of this `Table`
    #[must_use]
    pub(crate) fn state(&self) -> Arc<RwLock<TableState>> {
        Arc::clone(&self.state)
    }
}

impl<T> FromIterator<(Option<usize>, T)> for Table<T> {
    fn from_iter<I: IntoIterator<Item = (Option<usize>, T)>>(iter: I) -> Self {
        let state = Arc::new(RwLock::new(TableState::Unshared));
        Self {
            inner: Arc::new(RwLock::new(
                iter.into_iter()
                    .map(|(i, x)| InnerTable::new(x, Arc::clone(&state), i))
                    .collect(),
            )),
            state,
        }
    }
}

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
        match *self.state.write().unwrap() {
            ref mut state @ TableState::Exclusive((0, 0)) => *state = TableState::Unshared,
            ref mut state @ TableState::Exclusive((immuts, muts)) => {
                *state = TableState::SharedMuts((immuts, muts));
            }
            TableState::SharedMuts(_) | TableState::Shared(_) | TableState::Unshared => {
                unreachable!()
            }
        };
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
        match *self.state.write().unwrap() {
            ref mut state @ TableState::Shared((1, 0)) => *state = TableState::Unshared,
            TableState::Shared((ref mut amount, _)) => *amount -= 1,
            TableState::Exclusive(_) | TableState::Unshared | TableState::SharedMuts(_) => {
                unreachable!()
            }
        };
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
    /// # Panics
    /// - The state is poisoned
    /// - The value you are trying to access is poisoned
    #[must_use]
    pub fn read(&self) -> TableLockReader<T> {
        match *self.state.write().unwrap() {
            ref mut state @ TableState::Unshared => *state = TableState::Shared((1, 0)),
            TableState::Shared((_, ref mut refs))
            | TableState::SharedMuts((ref mut refs, _))
            | TableState::Exclusive((ref mut refs, _)) => *refs += 1,
        };
        TableLockReader {
            value: self.value.read().unwrap(),
            state: Arc::clone(&self.state),
        }
    }

    /// Actually locks the item and returns a Writer
    /// # Panics
    /// - The state is poisoned
    /// - The value you are trying to access is poisoned
    pub fn write(&self) -> Result<TableLockWriter<T>, LockError> {
        match *self.state.write().unwrap() {
            ref mut state @ TableState::Unshared => *state = TableState::SharedMuts((0, 1)),
            ref mut state @ TableState::Shared((0, refs)) => {
                *state = TableState::SharedMuts((refs, 1));
            }
            TableState::Exclusive((_, ref mut refs))
            | TableState::SharedMuts((_, ref mut refs)) => *refs += 1,
            TableState::Shared((1.., _)) => return Err(LockError::FailedLock),
        };
        Ok(TableLockWriter {
            value: self.value.write().unwrap(),
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
        match *self.state.write().unwrap() {
            ref mut state @ TableState::Shared((1, 0)) => *state = TableState::Unshared,
            TableState::Shared((_, ref mut refs))
            | TableState::SharedMuts((ref mut refs, _))
            | TableState::Exclusive((ref mut refs, _)) => *refs -= 1,
            TableState::Unshared => unreachable!(),
        };
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
        match *self.state.write().unwrap() {
            // ref mut state @ TableState::Exclusive => *state = TableState::Unshared,
            // TableState::Unshared | TableState::Shared(_) => unreachable!(),
            ref mut state @ TableState::SharedMuts((0, 1)) => *state = TableState::Unshared,
            TableState::Exclusive((_, ref mut amount))
            | TableState::SharedMuts((_, ref mut amount)) => *amount -= 1,
            TableState::Unshared | TableState::Shared(_) => unreachable!(),
        };
    }
}

pub struct InnerTable<T> {
    inner: Arc<TableLocker<T>>,
    /// The client whichs buffer is being referred to a `None` value signifies that it is from the
    /// original buffer
    bufnr: Option<usize>,
    state: Arc<RwLock<TableState>>,
}

impl<T> Clone for InnerTable<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            state: Arc::clone(&self.state),
            bufnr: self.bufnr,
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
    pub fn read(&self) -> (Option<usize>, TableLockReader<T>) {
        (self.bufnr, self.inner.read())
    }

    pub fn write(&self) -> Result<(Option<usize>, TableLockWriter<'_, T>), LockError> {
        Ok((self.bufnr, self.inner.write()?))
    }

    pub(crate) fn new(value: T, state: Arc<RwLock<TableState>>, bufnr: Option<usize>) -> Self {
        Self {
            inner: Arc::new(TableLocker::new(value, Arc::clone(&state))),
            state,
            bufnr,
        }
    }
}
