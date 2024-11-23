//! Implements a client type which can be used to insert data into the piece table
use std::{
    collections::VecDeque,
    sync::{Arc, RwLock},
};

use append_only_str::{slices::StrSlice, AppendOnlyStr};
use btep::Serialize;

use piece_table::{table::InnerTable, Piece};

/// A client which can input text into a `Piece`
pub struct Client {
    piece: Arc<RwLock<Piece>>,
    buffer: Arc<RwLock<AppendOnlyStr>>,
    slice: Option<InnerTable<StrSlice>>,
    bufnr: usize,
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
        *inner_table.write().unwrap().1 = self
            .buffer
            .read()
            .unwrap()
            .str_slice(self.buffer.read().unwrap().len()..);
        self.slice = Some(inner_table);
    }
}

impl Serialize for Client {
    fn serialize(&self) -> std::collections::VecDeque<u8> {
        let mut ret = VecDeque::new();
        ret.extend((self.bufnr as u64).to_be_bytes());
        if let Some(x) = &self.slice {
            ret.push_back(1);
            ret.extend((x.read().1.start() as u64).to_be_bytes());
            ret.extend((x.read().1.end() as u64).to_be_bytes());
        } else {
            ret.push_back(0);
        }
        ret
    }
}
