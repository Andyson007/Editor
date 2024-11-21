use std::{
    collections::LinkedList,
    fmt::Debug,
    ops::{Deref, DerefMut},
    sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard},
};

pub struct Table<T> {
    #[allow(clippy::linkedlist)]
    pub(crate) inner: Arc<RwLock<LinkedList<InnerTable<T>>>>,
    pub(crate) state: Arc<RwLock<TableState>>,
}

impl<T> Debug for Table<T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Table")
            .field(
                "inner",
                &self
                    .inner
                    .read()
                    .unwrap()
                    .front()
                    .unwrap()
                    .read()
                    .unwrap()
                    .value,
            )
            .field("state", &self.state)
            .finish()
    }
}

#[derive(Debug)]
pub(crate) enum TableState {
    /// There are no referenses to the list
    Unshared,
    /// The entire list only has immutable borrows. (full list borrows, single item immutable borrows)
    Shared((usize, usize)),
    /// The entire list isn't borrowed. (single item immutable bororws, single item mutable borrows)
    SharedMuts((usize, usize)),
    /// The entire list is mutably borrowed
    Exclusive,
}

impl<T> Table<T> {
    pub fn read_full(&self) -> Result<TableReader<T>, ()> {
        match *self.state.write().unwrap() {
            ref mut x @ TableState::Unshared => *x = TableState::Shared((1, 0)),
            TableState::Shared((ref mut amount, _)) => *amount += 1,
            TableState::Exclusive | TableState::SharedMuts(_) => return Err(()),
        };
        Ok(TableReader {
            val: Arc::clone(&self.inner),
            state: self.state.clone(),
        })
    }

    pub fn write_full(&self) -> Result<TableWriter<T>, ()> {
        match *self.state.write().unwrap() {
            ref mut x @ TableState::Unshared => *x = TableState::Exclusive,
            TableState::Exclusive | TableState::Shared(_) | TableState::SharedMuts(_) => {
                return Err(())
            }
        };
        Ok(TableWriter {
            val: Arc::clone(&self.inner),
            state: self.state.clone(),
        })
    }

    pub fn from_iter<I>(iter: I) -> Self
    where
        I: Iterator<Item = T>,
    {
        let state = Arc::new(RwLock::new(TableState::Unshared));
        Self {
            inner: Arc::new(RwLock::new(LinkedList::from_iter(
                iter.map(|x| InnerTable::new(x, Arc::clone(&state))),
            ))),
            state,
        }
    }

    pub fn state(&self) -> Arc<RwLock<TableState>> {
        Arc::clone(&self.state)
    }
}

pub struct TableWriter<T> {
    val: Arc<RwLock<LinkedList<InnerTable<T>>>>,
    state: Arc<RwLock<TableState>>,
}

impl<T> TableWriter<T> {
    pub fn read(&self) -> RwLockReadGuard<'_, LinkedList<InnerTable<T>>> {
        self.val.read().unwrap()
    }

    pub fn write(&self) -> RwLockWriteGuard<'_, LinkedList<InnerTable<T>>> {
        self.val.write().unwrap()
    }
}

impl<T> Drop for TableWriter<T> {
    fn drop(&mut self) {
        match *self.state.write().unwrap() {
            ref mut state @ TableState::Exclusive => *state = TableState::Unshared,
            TableState::SharedMuts(_) | TableState::Shared(_) | TableState::Unshared => {
                unreachable!()
            }
        };
    }
}

pub struct TableReader<T> {
    val: Arc<RwLock<LinkedList<InnerTable<T>>>>,
    state: Arc<RwLock<TableState>>,
}

impl<T> TableReader<T> {
    pub fn read(&self) -> RwLockReadGuard<'_, LinkedList<InnerTable<T>>> {
        self.val.read().unwrap()
    }
}

impl<T> Drop for TableReader<T> {
    fn drop(&mut self) {
        match *self.state.write().unwrap() {
            ref mut state @ TableState::Shared((1, 0)) => *state = TableState::Unshared,
            TableState::Shared((ref mut amount, _)) => *amount -= 1,
            TableState::Exclusive | TableState::Unshared | TableState::SharedMuts(_) => {
                unreachable!()
            }
        };
    }
}

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
    pub fn read(&self) -> Result<TableLockReader<T>, ()> {
        match *self.state.write().unwrap() {
            ref mut state @ TableState::Unshared => *state = TableState::Shared((1, 0)),
            TableState::Shared((_, ref mut refs)) => *refs += 1,
            TableState::SharedMuts((ref mut refs, _)) => *refs += 1,
            TableState::Exclusive => return Err(()),
        };
        Ok(TableLockReader {
            value: self.value.read().unwrap(),
            state: Arc::clone(&self.state),
        })
    }

    pub fn write(&self) -> Result<TableLockWriter<T>, ()> {
        todo!()
    }
}

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
            TableState::Shared((_, ref mut refs)) => *refs -= 1,
            TableState::SharedMuts((ref mut refs, _)) => *refs -= 1,
            TableState::Exclusive | TableState::Unshared => unreachable!(),
        };
    }
}

pub struct TableLockWriter<'a, T> {
    value: &'a mut T,
    state: Arc<RwLock<TableState>>,
}

impl<T> Deref for TableLockWriter<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.value
    }
}

impl<T> DerefMut for TableLockWriter<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.value
    }
}

impl<T> Drop for TableLockWriter<'_, T> {
    fn drop(&mut self) {
        match *self.state.write().unwrap() {
            // ref mut state @ TableState::Exclusive => *state = TableState::Unshared,
            // TableState::Unshared | TableState::Shared(_) => unreachable!(),
            ref mut state @ TableState::SharedMuts((0, 1)) => *state = TableState::Unshared,
            TableState::SharedMuts((_, ref mut amount)) => *amount -= 1,
            TableState::Exclusive | TableState::Unshared | TableState::Shared(_) => unreachable!(),
        };
    }
}

pub struct InnerTable<T> {
    inner: Arc<TableLocker<T>>,
    state: Arc<RwLock<TableState>>,
}

impl<T> Clone for InnerTable<T> {
    fn clone(&self) -> Self {
        match *self.state.write().unwrap() {
            ref mut state @ TableState::Unshared => *state = TableState::Shared((0, 1)),
            TableState::Shared((_, ref mut amount)) => *amount += 1,
            TableState::SharedMuts((ref mut amount, _)) => *amount += 1,
            TableState::Exclusive => unreachable!(),
        };
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
            .field("inner", &self.inner.read().unwrap().value)
            .field("state", &self.state)
            .finish()
    }
}

impl<T> InnerTable<T> {
    pub fn read(&self) -> Result<TableLockReader<T>, ()> {
        match *self.state.write().unwrap() {
            TableState::Exclusive => return Err(()),
            ref mut state @ TableState::Unshared => *state = TableState::Shared((0, 1)),
            TableState::SharedMuts((ref mut amount, _)) => *amount += 1,
            TableState::Shared((_, ref mut amount)) => *amount += 1,
        };
        self.inner.read()
    }

    pub fn write(&self) -> Result<TableLockWriter<'_, T>, ()> {
        match *dbg!(self.state.write().unwrap()) {
            TableState::Unshared => *self.state.write().unwrap() = TableState::SharedMuts((0, 1)),
            ref mut state @ TableState::Shared((0, amount)) => {
                *state = TableState::SharedMuts((amount, 1))
            }
            TableState::SharedMuts((_, ref mut amount)) => *amount += 1,
            TableState::Exclusive | TableState::Shared((1.., _)) => return Err(()),
        };
        self.inner.write()
    }

    pub(crate) fn new(value: T, state: Arc<RwLock<TableState>>) -> Self {
        Self {
            inner: Arc::new(TableLocker::new(value, Arc::clone(&state))),
            state,
        }
    }
}
