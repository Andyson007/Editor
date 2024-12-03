//! A Piece table implementation with multiple clients
#![feature(linked_list_cursors)]
use std::{
    collections::VecDeque,
    io::{self, Read},
    iter, mem,
    sync::{Arc, RwLock},
};

pub mod iters;
pub mod table;

use append_only_str::{slices::StrSlice, AppendOnlyStr};
use btep::{Deserialize, Serialize};
use table::{InnerTable, LockError, Table};
use utils::{
    iters::{InnerIteratorExt, IteratorExt},
    other::AutoIncrementing,
};

/// A wrapper around all the buffers
/// This includes the append buffers for each of the clients, and the original buffer
#[derive(Debug)]
pub struct Buffers {
    /// The original file content
    pub original: AppendOnlyStr,
    /// The appendbuffers for each of the clients
    pub clients: Vec<(Arc<RwLock<AutoIncrementing>>, Arc<RwLock<AppendOnlyStr>>)>,
}

/// A complete Piece table. It has support for handling multiple clients at the same time
#[derive(Debug)]
pub struct Piece {
    /// Holds the buffers that get modified when anyone inserts
    pub buffers: Buffers,
    /// stores the pieces to reconstruct the whole file
    pub piece_table: Table<TableElem>,
}

#[derive(Debug)]
pub struct TableElem {
    pub bufnr: Option<usize>,
    pub id: usize,
    pub text: StrSlice,
}

impl Piece {
    #[must_use]
    /// Creates an empty piece table
    pub fn new() -> Self {
        let original: AppendOnlyStr = "".into();
        Self {
            piece_table: std::iter::once(TableElem {
                id: 0,
                bufnr: None,
                text: original.str_slice(..),
            })
            .collect(),
            buffers: Buffers {
                original,
                clients: vec![],
            },
        }
    }

    /// Creates a new Piece table from scratch with the initial value of the original buffer being
    /// read from somewhere.
    ///
    /// # Errors
    /// This function errors when the reader fails to read
    pub fn original_from_reader<T: Read>(mut read: T) -> io::Result<Self> {
        let mut string = String::new();
        read.read_to_string(&mut string)?;
        let original: AppendOnlyStr = string.into();

        Ok(Self {
            piece_table: iter::once(TableElem {
                bufnr: None,
                id: 0,
                text: original.str_slice(..),
            })
            .collect(),
            buffers: Buffers {
                original,
                clients: vec![],
            },
        })
    }

    /// Creates an `InnerTable` within the piece table.
    /// This allows the list to be mutated at that point.
    /// # Panics
    /// Shouldn't panic
    pub fn insert_at(
        &mut self,
        pos: usize,
        clientid: usize,
    ) -> Option<(Option<usize>, InnerTable<TableElem>)> {
        let binding = self.piece_table.write_full().unwrap();
        let mut to_split = binding.write();
        let mut cursor = to_split.cursor_front_mut();
        let mut curr_pos = cursor.current().unwrap().read().text.len();
        let is_end = loop {
            if curr_pos > pos {
                break false;
            }
            cursor.move_next();
            if let Some(x) = cursor.current() {
                curr_pos += x.read().text.len();
            } else {
                break true;
            }
        };

        let offset = if is_end {
            // NOTE: We rely on the cursors position later
            cursor.move_prev();
            let curr = self.buffers.clients[clientid].1.read().unwrap();
            cursor.insert_after(InnerTable::new(
                TableElem {
                    bufnr: Some(clientid),
                    text: curr.str_slice(curr.len()..),
                    id: self.buffers.clients[clientid].0.write().unwrap().get()
                        * self.buffers.clients.len()
                        + clientid,
                },
                self.piece_table.state(),
            ));
            None
        } else {
            let (buf_of_split, current) = {
                let current = cursor.current().unwrap().read();
                (current.bufnr, current.text.clone())
            };
            let offset = pos - (curr_pos - current.len());

            if offset != 0 {
                cursor.insert_before(InnerTable::new(
                    TableElem {
                        bufnr: buf_of_split,
                        text: current.subslice(..offset)?,
                        id: self.buffers.clients[clientid].0.write().unwrap().get()
                            * self.buffers.clients.len()
                            + clientid,
                    },
                    self.piece_table.state(),
                ));
            }

            cursor.insert_after(InnerTable::new(
                TableElem {
                    bufnr: buf_of_split,
                    text: current.subslice(offset..)?,
                    id: self.buffers.clients[clientid].0.write().unwrap().get()
                        * self.buffers.clients.len()
                        + clientid,
                },
                self.piece_table.state(),
            ));
            let curr = self.buffers.clients[clientid].1.read().unwrap();
            cursor.current().unwrap().write().unwrap().text = curr.str_slice(curr.len()..);
            Some(offset)
        };
        Some((offset, cursor.current().unwrap().clone()))
    }

    /// Locks down the full list for reading.
    /// This means that
    /// - No value within the list can be mutated
    /// - The order of elements cannot be changed
    /// # Errors
    /// - The state value is poisoned
    pub fn read_full(&self) -> Result<table::TableReader<TableElem>, LockError> {
        self.piece_table.read_full()
    }

    /// Locks down the full list for reading.
    /// This means that
    /// - This is the sole write lock on the full list
    /// - No reading lock can be made
    /// # Errors
    /// - The state value is poisoned
    pub fn write_full(&self) -> Result<table::TableWriter<TableElem>, LockError> {
        self.piece_table.write_full()
    }
}

impl Default for Piece {
    fn default() -> Self {
        Self::new()
    }
}

impl Serialize for &Piece {
    fn serialize(&self) -> std::collections::VecDeque<u8> {
        let mut ret = VecDeque::new();
        ret.extend(self.buffers.original.str_slice(..).as_str().bytes());
        for client in &self.buffers.clients {
            // 0xfe is used here because its not representable by utf8, and makes stuff easier to
            // parse. This is useful because the alternative is the specify the strings length,
            // which would take up at least as many bytes
            ret.push_back(0xfe);
            ret.extend((client.0.read().unwrap().peek() as u64).to_be_bytes());
            ret.extend(client.1.read().unwrap().str_slice(..).as_str().bytes());
        }
        // Might be useless, but it's a single byte
        ret.push_back(0xff);

        ret.extend((self.piece_table.read_full().unwrap().read().len() as u64).to_be_bytes());

        for piece in self.piece_table.read_full().unwrap().read().iter() {
            let piece = piece.read();
            // NOTE: This probably shouldn't use u64::MAX, but idk about a better way
            ret.extend((piece.bufnr.map_or(u64::MAX, |x| x as u64)).to_be_bytes());
            ret.extend((piece.id as u64).to_be_bytes());
            ret.extend((piece.text.start() as u64).to_be_bytes());
            ret.extend((piece.text.end()).to_be_bytes());
        }
        ret
    }
}

impl Deserialize for Piece {
    fn deserialize(data: &[u8]) -> Self {
        // `take_while_ref` requires a peekable wrapper
        #[allow(clippy::unused_peekable)]
        let mut iter = data.iter().copied().peekable();

        let original_buffer: AppendOnlyStr = String::from_utf8(
            iter.take_while_ref(|x| !(*x == 254 || *x == 255))
                .collect::<Vec<_>>(),
        )
        .unwrap()
        .into();

        let mut client_buffers = Vec::new();
        loop {
            if iter.next() == Some(255) {
                break;
            }
            let counter_start = u64::from_be_bytes(
                iter.by_ref()
                    .take(mem::size_of::<u64>())
                    .collect::<Vec<_>>()
                    .try_into()
                    .unwrap(),
            ) as usize;

            client_buffers.push((
                Arc::new(RwLock::new(AutoIncrementing::new_with_start(counter_start))),
                Arc::new(RwLock::new(
                    iter.by_ref()
                        .take_while(|x| !(*x == 254 || *x == 255))
                        .collect::<AppendOnlyStr>(),
                )),
            ));
        }

        let pieces: [u8; 8] = iter
            .by_ref()
            .take(mem::size_of::<u64>())
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();

        let piece_count = usize::try_from(u64::from_be_bytes(pieces)).unwrap();
        let table = iter
            .by_ref()
            .chunks::<{ 4 * mem::size_of::<u64>() }>()
            .take(piece_count)
            .map(|x| {
                let slices = x
                    .into_iter()
                    .chunks::<{ mem::size_of::<u64>() }>()
                    .collect::<Vec<_>>();
                let id = usize::try_from(u64::from_be_bytes(slices[1])).unwrap();
                let start = usize::try_from(u64::from_be_bytes(slices[2])).unwrap();
                let end = usize::try_from(u64::from_be_bytes(slices[3])).unwrap();
                match u64::from_be_bytes(slices[0]) {
                    u64::MAX => TableElem {
                        bufnr: None,
                        text: original_buffer.str_slice(start..end),
                        id,
                    },
                    bufnr => {
                        let buf = usize::try_from(bufnr).unwrap();
                        TableElem {
                            bufnr: Some(buf),
                            text: client_buffers[buf].1.read().unwrap().str_slice(start..end),
                            id,
                        }
                    }
                }
            })
            .collect();

        Self {
            buffers: Buffers {
                original: original_buffer,
                clients: client_buffers,
            },
            piece_table: table,
        }
    }
}

#[cfg(test)]
mod test {
    use std::io::BufReader;

    use crate::Piece;

    #[test]
    fn from_reader() {
        let mut bytes = &b"test"[..];
        let piece =
            Piece::original_from_reader(BufReader::with_capacity(bytes.len(), &mut bytes)).unwrap();
        let binding = piece.read_full().unwrap();
        let binding = binding.read();
        let mut iter = binding.iter();
        let next = iter.next().unwrap().read();
        assert_eq!(&*next.text, "test");
        assert_eq!(next.bufnr, None);
        assert!(iter.next().is_none());
    }
}
