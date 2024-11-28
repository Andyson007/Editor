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
    pub(crate) prev: Option<InnerTable<StrSlice>>,
    pub(crate) bufnr: usize,
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
            prev: None,
            bufnr,
        }
    }

    pub fn backspace(&mut self) {
        let binding = self.slice.as_ref().unwrap();
        let (_, slice) = binding.read();
        if slice.is_empty() {
            let binding = &self.piece.write().unwrap().piece_table;
            let mut binding2 = binding.inner.write().unwrap();
            let mut cursor = binding2.cursor_front_mut();
            let delete_from = if let Some(prev) = self.prev.as_ref() {
                prev
            } else {
                loop {
                    if *cursor.current().unwrap().read().1 == *slice {
                        break;
                    }
                    cursor.move_next();
                }
                let Some(prev) = cursor.peek_prev() else {
                    return;
                };
                if prev.read().1.is_empty() {
                    self.prev = Some(prev.clone());
                    &*prev
                } else {
                    while cursor.current().as_ref().unwrap().read().1.is_empty() {
                        cursor.move_prev();
                    }
                    cursor.current().unwrap()
                }
            };
            drop(slice);
            let (_, mut slice) = delete_from.write().unwrap();
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
        self.prev = None;
        self.slice = Some(inner_table);
    }
}
