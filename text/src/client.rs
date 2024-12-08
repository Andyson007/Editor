//! Implements a client type which can be used to insert data into the piece table
use std::{
    collections::linked_list::CursorMut,
    fmt::Debug,
    sync::{Arc, RwLock},
};

use append_only_str::AppendOnlyStr;
use piece_table::{table::InnerTable, Piece, TableElem};
use utils::other::{AutoIncrementing, CursorPos};

/// A client which can input text into a `Piece`
#[derive(Debug)]
pub struct Client {
    /// The full piece table we are editing inside of
    pub(crate) piece: Arc<RwLock<Piece>>,
    /// The current buffer we are editing
    pub(crate) buffer: Arc<RwLock<AppendOnlyStr>>,
    /// A conuter used to generate unique ids
    pub(crate) id_counter: Arc<RwLock<AutoIncrementing>>,
    /// The id of the buffer this client is editing
    pub(crate) bufnr: usize,
    /// None -> You are currently not in insert mode
    pub data: Option<Insertdata>,
}

/// Stores data related to being in insert mode
#[derive(Debug)]
pub struct Insertdata {
    /// The slice being edited
    pub(crate) slice: InnerTable<TableElem>,
    /// The cursors location
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
    /// 0:
    /// Returns None if the client couldn't press backspace
    /// This happens when
    /// - The cursor is at the first byte in the file
    /// - A different client is editing right in front of this cursor
    /// Returns some when a character was deleted
    /// 1: The amount of swaps
    /// that were made
    /// # Panics
    /// - function called without ever entering insert mode
    ///
    /// this function will probably only panic when there are locking errors though
    pub fn backspace(&mut self) -> (Option<char>, usize) {
        let binding = self.data.as_mut().unwrap();
        let slice = binding.slice.read();
        let ret = if slice.text.is_empty() {
            let binding = self
                .piece
                .write()
                .unwrap()
                .piece_table
                .write_full()
                .unwrap();

            let mut binding2 = binding.write();
            let mut cursor = binding2.cursor_front_mut();
            while cursor.current().unwrap().read().text != slice.text {
                cursor.move_next();
            }
            Self::delete_from_cursor(&mut cursor)
        } else {
            drop(slice);
            (Self::foo(&mut binding.slice), 0)
        };
        binding.has_deleted = true;
        ret
    }

    fn delete_from_cursor(cursor: &mut CursorMut<'_, InnerTable<TableElem>>) -> (Option<char>, usize) {
        let mut swap_count = 0;
        loop {
            cursor.move_prev();
            if cursor.current().unwrap().read().text.is_empty() {
                swap_count += 1;
                let curr = cursor.remove_current().unwrap();
                cursor.insert_after(curr);
            } else {
                break;
            }
        }
        let Some(prev) = cursor.current() else {
            return (None, swap_count);
        };
        (Self::foo(prev), swap_count)
    }

    fn foo(binding: &mut InnerTable<TableElem>) -> Option<char> {
        let slice = &mut binding.write().unwrap();
        let ret = slice.text.chars().last();
        debug_assert!(!slice.text.is_empty());
        slice.text = slice
            .text
            .subslice(0..slice.text.len() - slice.text.chars().last().unwrap().len_utf8())
            .unwrap();
        ret
    }

    pub fn backspace_with_swaps(&mut self, swaps: usize) -> Option<char> {
        if swaps == 0 {
            let (ret, swaps) = self.backspace();
            debug_assert_eq!(swaps, 0);
            ret
        } else {
            let binding = self.data.as_mut().unwrap();
            let slice = binding.slice.read();

            let binding = self
                .piece
                .write()
                .unwrap()
                .piece_table
                .write_full()
                .unwrap();

            let mut binding2 = binding.write();
            let mut cursor = binding2.cursor_front_mut();
            while cursor.current().unwrap().read().text != slice.text {
                cursor.move_next();
            }
            for _ in 0..swaps {
                cursor.move_prev();
                if cursor.current().unwrap().read().text.is_empty() {
                    let curr = cursor.remove_current().unwrap();
                    cursor.insert_after(curr);
                } else {
                    break;
                }
            }
            todo!()
        }
    }

    /// appends a char at the current location
    /// # Panics
    /// - Insert mode isn't entered
    /// - We can't read our own buffer. This is most likely this crates fault
    pub fn push_char(&mut self, to_push: char) {
        self.push_str(&to_push.to_string());
    }

    /// Exits insert mode
    pub fn exit_insert(&mut self) {
        self.data = None;
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
                pos: self.data.as_ref().unwrap().pos + (0, 1),
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
