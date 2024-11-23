//! Implements a client type which can be used to insert data into the piece table
use std::sync::{Arc, RwLock};

use append_only_str::{slices::StrSlice, AppendOnlyStr};

use crate::{table::InnerTable, Piece};

/// A client which can input text into a `Piece`
pub struct Client {
    buffer: Arc<RwLock<AppendOnlyStr>>,
    slice: Option<InnerTable<StrSlice>>,
    bufnr: usize,
    is_first_insert: bool,
}

impl Client {
    /// Creates a new client.
    /// takes a buffer to write to as an input
    pub const fn new(buffer: Arc<RwLock<AppendOnlyStr>>, bufnr: usize) -> Self {
        Self {
            buffer,
            slice: None,
            is_first_insert: false,
            bufnr,
        }
    }

    /// appends a string at the current location
    /// # Panics
    /// - Insert mode isn't entered
    /// - We can't read our own buffer. This is most likely this crates fault
    pub fn push_str(&mut self, to_push: &str) {
        self.buffer.write().unwrap().push_str(to_push);
        let slice = self
            .slice
            .as_mut()
            .expect("Can only call push_str in insert mode");
        let mut a = slice.write().unwrap().1;
        let start = if self.is_first_insert {
            self.is_first_insert = false;
            self.buffer.read().unwrap().len() - to_push.len()
        } else {
            a.start()
        };
        *a = self.buffer.read().unwrap().str_slice(start..);
    }

    /// Allows for insertion.
    /// Takes an `InnerTable` as an argument as to where the text should be inserted
    pub fn enter_insert(&mut self, piece: &mut Piece, index: usize) {
        let inner_table = piece.insert_at(index, self.bufnr).unwrap();
        self.slice = Some(inner_table);
        self.is_first_insert = true;
    }
}
