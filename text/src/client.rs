//! Implements a client type which can be used to insert data into the piece table
use std::{
    fmt::Debug,
    sync::{Arc, RwLock},
};

use append_only_str::{slices::StrSlice, AppendOnlyStr};

use piece_table::{table::InnerTable, Piece};

/// A client which can input text into a `Piece`
#[derive(Debug)]
pub struct Client {
    pub(crate) piece: Arc<RwLock<Piece>>,
    pub(crate) buffer: Arc<RwLock<AppendOnlyStr>>,
    pub(crate) slice: Option<InnerTable<StrSlice>>,
    pub(crate) bufnr: usize,
    /// Stores whether its safe to insert a chracter again
    /// # Necessity
    /// This is required because pressing backspace and writing the character again cannot be
    /// represented (effeciently) in an append_only buffer.
    pub(crate) has_deleted: bool,
}

impl Client {
    /// Creates a new client.
    /// takes a buffer to write to as an input
    pub const fn new(
        piece: Arc<RwLock<Piece>>,
        buffer: Arc<RwLock<AppendOnlyStr>>,
        bufnr: usize,
    ) -> Self {
        Self {
            piece,
            buffer,
            slice: None,
            bufnr,
            has_deleted: false,
        }
    }

    pub fn backspace(&mut self) {
        let binding = self.slice.as_ref().unwrap();
        let (_, slice) = binding.read();
        if slice.is_empty() {
            let binding = &self.piece.write().unwrap().piece_table;
            let mut binding2 = binding.inner.write().unwrap();
            let mut cursor = binding2.cursor_front_mut();
            while *cursor.current().unwrap().read().1 != *slice {
                cursor.move_next();
            }
            while cursor.current().unwrap().read().1.is_empty() {
                cursor.move_prev();
            }
            drop(slice);
            let Some(prev) = cursor.current() else {
                return;
            };
            let (_, mut slice) = prev.write().unwrap();
            *slice = slice
                .subslice(0..slice.len() - slice.chars().last().unwrap().len_utf8())
                .unwrap();
        } else {
            drop(slice);
            let (_, mut slice) = binding.write().unwrap();
            *slice = slice
                .subslice(0..slice.len() - slice.chars().last().unwrap().len_utf8())
                .unwrap()
        }
        self.has_deleted = true;
    }

    /// appends a char at the current location
    /// # Panics
    /// - Insert mode isn't entered
    /// - We can't read our own buffer. This is most likely this crates fault
    pub fn push_char(&mut self, to_push: char) {
        self.push_str(&to_push.to_string());
    }

    /// appends a string at the current location
    /// # Panics
    /// - Insert mode isn't entered
    /// - We can't read our own buffer. This is most likely this crates fault
    pub fn push_str(&mut self, to_push: &str) {
        assert!(
            self.slice.is_some(),
            "You can only push stuff after entering insert mode"
        );
        if to_push.is_empty() {
            return;
        }

        if self.has_deleted {
            let slice = self.slice.as_mut().unwrap();

            let binding = &self.piece.write().unwrap().piece_table;
            let mut binding2 = binding.inner.write().unwrap();
            let mut cursor = binding2.cursor_front_mut();
            while *cursor.current().unwrap().read().1 != *slice.read().1 {
                cursor.move_next();
            }
            cursor.insert_after(InnerTable::new(
                self.buffer.read().unwrap().str_slice_end(),
                binding.state(),
                Some(self.bufnr),
            ));
            self.slice = Some(cursor.peek_next().unwrap().clone());
            self.has_deleted = false;
        }

        let slice = self.slice.as_mut().unwrap();

        self.buffer.write().unwrap().push_str(to_push);
        let mut a = slice.write().unwrap().1;
        *a = self.buffer.read().unwrap().str_slice(a.start()..);
    }

    /// Allows for insertion.
    /// Takes an `InnerTable` as an argument as to where the text should be inserted
    pub fn enter_insert(&mut self, index: usize) {
        let inner_table = self
            .piece
            .write()
            .unwrap()
            .insert_at(index, self.bufnr)
            .unwrap();
        // FIXME: The reason this is here is to fix a stupid bug. I should use std::pin::Pin to fix
        // this.
        // The issue is that entering insert mode and immediately exiting insert mode results in
        // two identical slices. They will have the same start, end **and** pointers. We push an
        // arbitrary string here to offset this pointer.
        self.buffer.write().unwrap().push_str("\0");
        *inner_table.write().unwrap().1 = self
            .buffer
            .read()
            .unwrap()
            .str_slice(self.buffer.read().unwrap().len()..);
        self.slice = Some(inner_table);
    }
}
