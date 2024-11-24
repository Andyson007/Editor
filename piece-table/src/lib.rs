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
use table::{InnerTable, Table};
use utils::iters::{InnerIteratorExt, IteratorExt};

#[derive(Debug)]
pub struct Buffers {
    /// The original file content
    pub original: AppendOnlyStr,
    /// The appendbuffers for each of the clients
    pub clients: Vec<Arc<RwLock<AppendOnlyStr>>>,
}

/// A complete Piece table. It has support for handling multiple clients at the same time
#[derive(Debug)]
pub struct Piece {
    /// Holds the buffers that get modified when anyone inserts
    pub buffers: Buffers,
    /// stores the pieces to reconstruct the whole file
    pub piece_table: Table<StrSlice>,
}

impl Piece {
    #[must_use]
    /// Creates an empty piece table
    pub fn new() -> Self {
        let original: AppendOnlyStr = "".into();
        Self {
            piece_table: Table::from_iter(std::iter::once((None, original.str_slice(..)))),
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
            piece_table: Table::from_iter(iter::once((None, original.str_slice(..)))),
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
    #[allow(clippy::significant_drop_tightening)]
    pub fn insert_at(&mut self, pos: usize, bufnr: usize) -> Option<InnerTable<StrSlice>> {
        let binding = self.piece_table.write_full().unwrap();
        let mut to_split = binding.write();
        let mut cursor = to_split.cursor_front_mut();
        let mut curr_pos = cursor.current().unwrap().read().1.len();
        let is_end = loop {
            if curr_pos > pos {
                break false;
            }
            cursor.move_next();
            if let Some(x) = cursor.current() {
                curr_pos += x.read().1.len();
            } else {
                break true;
            }
        };

        if is_end {
            cursor.move_prev();
            cursor.insert_after(InnerTable::new(
                StrSlice::empty(),
                self.piece_table.state(),
                Some(bufnr),
            ));
        } else {
            let (bufnr, current) = {
                let current = cursor.current().unwrap().read();
                (current.0, current.1.clone())
            };
            let offset = pos - (curr_pos - current.len());

            if offset != 0 {
                cursor.insert_before(InnerTable::new(
                    current.subslice(..offset)?,
                    self.piece_table.state(),
                    bufnr,
                ));
            }

            cursor.insert_after(InnerTable::new(
                current.subslice(offset..)?,
                self.piece_table.state(),
                bufnr,
            ));
            *cursor.current().unwrap().write().unwrap().1 = StrSlice::empty();
        }
        Some(cursor.current().unwrap().clone())
    }

    pub fn read_full(&self) -> Result<table::TableReader<StrSlice>, ()> {
        self.piece_table.read_full()
    }

    pub fn write_full(&self) -> Result<table::TableWriter<StrSlice>, ()> {
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
            ret.extend(client.read().unwrap().str_slice(..).as_str().bytes());
        }
        // Might be useless, but it's a single byte
        ret.push_back(0xff);

        ret.extend((self.piece_table.read_full().unwrap().read().len() as u64).to_be_bytes());

        for piece in self.piece_table.read_full().unwrap().read().iter() {
            let piece = piece.read();
            // NOTE: This probably shouldn't use u64::MAX, but idk about a better way
            ret.extend((piece.0.map_or(u64::MAX, |x| x as u64)).to_be_bytes());
            ret.extend((piece.1.start() as u64).to_be_bytes());
            ret.extend((piece.1.end()).to_be_bytes());
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
            client_buffers.push(Arc::new(RwLock::new(
                iter.by_ref()
                    .take_while(|x| !(*x == 254 || *x == 255))
                    .collect::<AppendOnlyStr>(),
            )));
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
            .chunks::<{ 3 * mem::size_of::<u64>() }>()
            .take(piece_count)
            .map(|x| {
                let slices = x
                    .into_iter()
                    .chunks::<{ mem::size_of::<u64>() }>()
                    .collect::<Vec<_>>();
                let start = usize::try_from(u64::from_be_bytes(slices[1])).unwrap();
                let end = usize::try_from(u64::from_be_bytes(slices[2])).unwrap();
                match u64::from_be_bytes(slices[0]) {
                    u64::MAX => (None, original_buffer.str_slice(start..end)),
                    bufnr => {
                        let buf = usize::try_from(bufnr).unwrap();
                        (
                            Some(buf),
                            client_buffers[buf].read().unwrap().str_slice(start..end),
                        )
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
        assert_eq!(&**next.1, "test");
        assert_eq!(next.0, None);
        assert!(iter.next().is_none());
    }
}
