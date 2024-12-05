//! Implements a client type which can be used to insert data into the piece table
use std::{
    fmt::Debug,
    sync::{Arc, RwLock},
};

use append_only_str::AppendOnlyStr;
use piece_table::{table::InnerTable, Piece, TableElem};
use utils::other::{AutoIncrementing, CursorPos};

/// A client which can input text into a `Piece`
#[derive(Debug)]
pub struct Client {
    pub(crate) piece: Arc<RwLock<Piece>>,
    pub(crate) buffer: Arc<RwLock<AppendOnlyStr>>,
    pub(crate) id_counter: Arc<RwLock<AutoIncrementing>>,
    pub(crate) bufnr: usize,
    pub data: Option<Insertdata>,
}

#[derive(Debug)]
pub struct Insertdata {
    pub(crate) slice: InnerTable<TableElem>,
    pub pos: CursorPos,
    /// Stores whether its safe to insert a chracter again
    /// # Necessity
    /// This is required because pressing backspace and writing the character again cannot be
    /// represented (effeciently) in an append-only buffer.
    pub(crate) has_deleted: bool,
}

impl Client {
    /// Creates a new client.
    /// takes a buffer to write to as an input
    pub const fn new(
        piece: Arc<RwLock<Piece>>,
        buffer: Arc<RwLock<AppendOnlyStr>>,
        bufnr: usize,
        id_counter: Arc<RwLock<AutoIncrementing>>,
    ) -> Self {
        Self {
            piece,
            buffer,
            bufnr,
            id_counter,
            data: None,
        }
    }

    /// Handles a backspace press by the client
    /// # Panics
    /// - function called without ever entering insert mode
    ///
    /// this function will probably only panic when there are locking errors though
    pub fn backspace(&mut self) -> Option<char> {
        let ret;
        let binding = self.data.as_mut().unwrap();
        let slice = binding.slice.read();
        if slice.text.is_empty() {
            let binding = self
                .piece
                .write()
                .unwrap()
                .piece_table
                .write_full()
                .unwrap();

            let binding2 = binding.write();
            let mut cursor = binding2.cursor_front();
            while cursor.current().unwrap().read().text != slice.text {
                cursor.move_next();
            }
            while cursor.current().unwrap().read().text.is_empty() {
                cursor.move_prev();
            }
            let prev = cursor.current()?;
            drop(slice);
            let slice = &mut prev.write().unwrap();
            ret = slice.text.chars().last().unwrap();
            slice.text = slice
                .text
                .subslice(0..slice.text.len() - slice.text.chars().last().unwrap().len_utf8())
                .unwrap();
        } else {
            drop(slice);
            let slice = &mut binding.slice.write().unwrap();
            ret = slice.text.chars().last().unwrap();
            slice.text = slice
                .text
                .subslice(0..slice.text.len() - slice.text.chars().last().unwrap().len_utf8())
                .unwrap();
        }
        binding.has_deleted = true;
        Some(ret)
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
            self.data.is_some(),
            "You can only push stuff after entering insert mode"
        );
        if to_push.is_empty() {
            return;
        }

        if self.data.as_ref().unwrap().has_deleted {
            let slice = &self.data.as_mut().unwrap().slice;

            let client_count = self.piece.read().unwrap().buffers.clients.len();
            let binding = &self.piece.write().unwrap().piece_table;
            let binding2 = binding.write_full().unwrap();
            let mut binding3 = binding2.write();
            let mut cursor = binding3.cursor_front_mut();
            while cursor.current().unwrap().read().text != slice.read().text {
                cursor.move_next();
            }
            cursor.insert_after(InnerTable::new(
                TableElem {
                    bufnr: Some(self.bufnr),
                    text: self.buffer.read().unwrap().str_slice_end(),
                    id: self.id_counter.write().unwrap().get() * client_count + self.bufnr,
                },
                binding.state(),
                // self.id_counter.write().unwrap().get() * client_count + self.bufnr,
            ));
            self.data = Some(Insertdata {
                slice: cursor.peek_next().unwrap().clone(),
                pos: todo!(),
                has_deleted: false,
            });
        }

        let slice = &self.data.as_mut().unwrap().slice;

        self.buffer.write().unwrap().push_str(to_push);
        let a = &mut slice.write().unwrap().text;
        *a = self.buffer.read().unwrap().str_slice(a.start()..);
    }

    /// Allows for insertion.
    /// Takes an `InnerTable` as an argument as to where the text should be inserted
    /// # Return
    /// It returns the id of the buffer that got split
    /// # Panics
    /// probably only failed locks
    pub fn enter_insert(&mut self, pos: CursorPos) -> (Option<usize>, usize) {
        let (offset, inner_table) = self
            .piece
            .write()
            .unwrap()
            .insert_at(pos, self.bufnr)
            .unwrap();
        let idx = inner_table.read().id;
        // FIXME: The reason this is here is to fix a stupid bug. I should use std::pin::Pin to fix
        // this.
        // The issue is that entering insert mode and immediately exiting insert mode results in
        // two identical slices. They will have the same start, end **and** pointers. We push an
        // arbitrary string here to offset this pointer.
        self.buffer.write().unwrap().push_str("\0");
        inner_table.write().unwrap().text = self
            .buffer
            .read()
            .unwrap()
            .str_slice(self.buffer.read().unwrap().len()..);
        self.data = Some(Insertdata {
            slice: inner_table,
            has_deleted: false,
            pos,
        });
        (offset, idx)
    }
}
