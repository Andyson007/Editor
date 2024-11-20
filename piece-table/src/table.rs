use std::{
    collections::LinkedList,
    fmt::Debug,
    ops::{Deref, DerefMut},
    sync::{Arc, RwLock, RwLockReadGuard},
};

pub struct Table<T> {
    #[allow(clippy::linkedlist)]
    inner: Arc<LinkedList<InnerTable<T>>>,
    state: Arc<RwLock<TableState>>,
}

impl<T> Debug for Table<T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Table")
            .field("inner", &self.inner.front().unwrap().read().unwrap().value)
            .field("state", &self.state)
            .finish()
    }
}

#[derive(Debug)]
enum TableState {
    Unshared,
    Shared(usize),
    Exclusive,
}

impl<T> Table<T> {
    pub fn read_full(&self) -> Result<TableReader<T>, ()> {
        match *self.state.write().unwrap() {
            TableState::Exclusive => return Err(()),
            ref mut x @ TableState::Unshared => *x = TableState::Shared(1),
            TableState::Shared(ref mut amount) => *amount += 1,
        };
        Ok(TableReader {
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
            inner: Arc::new(LinkedList::from_iter(
                iter.map(|x| InnerTable::new(x, Arc::clone(&state))),
            )),
            state,
        }
    }
}

pub struct TableReader<T> {
    val: Arc<LinkedList<InnerTable<T>>>,
    state: Arc<RwLock<TableState>>,
}

impl<T> Deref for TableReader<T> {
    type Target = LinkedList<InnerTable<T>>;

    fn deref(&self) -> &Self::Target {
        &self.val
    }
}

impl<T> Drop for TableReader<T> {
    fn drop(&mut self) {
        match *self.state.write().unwrap() {
            ref mut state @ TableState::Shared(1) => *state = TableState::Unshared,
            ref mut state @ TableState::Shared(val @ 2..) => *state = TableState::Shared(val - 1),
            TableState::Exclusive | TableState::Unshared | TableState::Shared(0) => unreachable!(),
        };
    }
}

pub struct TableLocker<T> {
    value: Arc<RwLock<T>>,
    state: Arc<RwLock<TableState>>,
}

impl<T> TableLocker<T> {
    pub fn new(value: T, state: Arc<RwLock<TableState>>) -> Self {
        Self {
            value: Arc::new(RwLock::new(value)),
            state,
        }
    }
}

impl<T> TableLocker<T> {
    pub fn read(&self) -> Result<TableLockReader<T>, ()> {
        match *self.state.write().unwrap() {
            ref mut state @ TableState::Unshared => *state = TableState::Shared(1),
            ref mut state @ TableState::Shared(val) => *state = TableState::Shared(val + 1),
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
            ref mut state @ TableState::Shared(1) => *state = TableState::Unshared,
            ref mut state @ TableState::Shared(val @ 2..) => *state = TableState::Shared(val - 1),
            TableState::Exclusive | TableState::Unshared | TableState::Shared(0) => unreachable!(),
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
        &mut self.value
    }
}

impl<T> Drop for TableLockWriter<'_, T> {
    fn drop(&mut self) {
        match *self.state.write().unwrap() {
            ref mut state @ TableState::Shared(1) => *state = TableState::Unshared,
            ref mut state @ TableState::Shared(val @ 2..) => *state = TableState::Shared(val - 1),
            TableState::Exclusive | TableState::Unshared | TableState::Shared(0) => unreachable!(),
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
            TableState::Exclusive => panic!(),
            ref mut state @ TableState::Unshared => *state = TableState::Shared(1),
            TableState::Shared(ref mut amount) => *amount += 1,
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
            ref mut state @ TableState::Unshared => *state = TableState::Shared(1),
            TableState::Shared(ref mut amount) => *amount += 1,
        };
        self.inner.read()
    }

    pub fn write(&self) -> Result<TableLockWriter<'_, T>, ()> {
        match *self.state.write().unwrap() {
            TableState::Unshared => *self.state.write().unwrap() = TableState::Exclusive,
            TableState::Exclusive => return Err(()),
            TableState::Shared(_) => return Err(()),
        };
        self.inner.write()
    }

    fn new(value: T, state: Arc<RwLock<TableState>>) -> Self {
        Self {
            inner: Arc::new(TableLocker::new(value, Arc::clone(&state))),
            state,
        }
    }
}

pub struct Borrow<'a, T> {
    value: &'a T,
}
