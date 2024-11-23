//! A Piece table implementation with multiple clients
#![feature(linked_list_cursors)]
use std::{
    collections::VecDeque,
    io::{self, Read},
    iter, mem,
    sync::{Arc, RwLock},
};

pub mod client;
pub mod iters;
mod table;

use append_only_str::{slices::StrSlice, AppendOnlyStr};
use btep::{Deserialize, Serialize};
use client::Client;
use table::{InnerTable, Table};
use utils::iters::{InnerIteratorExt, IteratorExt};

#[derive(Debug)]
struct Buffers {
    /// The original file content
    original: AppendOnlyStr,
    /// The appendbuffers for each of the clients
    clients: Vec<Arc<RwLock<AppendOnlyStr>>>,
}

/// A complete Piece table. It has support for handling multiple clients at the same time
#[derive(Debug)]
pub struct Piece {
    /// Holds the buffers that get modified when anyone inserts
    buffers: Buffers,
    /// stores the pieces to reconstruct the whole file
    #[allow(clippy::linkedlist)]
    piece_table: Table<StrSlice>,
}

impl Piece {
    #[must_use]
    /// Creates an empty piece table
    pub fn new() -> Self {
        let original: AppendOnlyStr = "".into();
        Self {
            piece_table: Table::from_iter(std::iter::once(original.str_slice(..))),
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
            piece_table: Table::from_iter(iter::once(original.str_slice(..))),
            buffers: Buffers {
                original,
                clients: vec![],
            },
        })
    }

    /// Creates a `Client` with an attached buffer
    pub fn add_client(&mut self) -> Client {
        let buf = Arc::new(RwLock::new(AppendOnlyStr::new()));
        self.buffers.clients.push(Arc::clone(&buf));
        Client::new(buf)
    }

    /// Creates an `InnerTable` within the piece table.
    /// This allows the list to be mutated at that point.
    /// # Panics
    /// Shouldn't panic
    #[allow(clippy::significant_drop_tightening)]
    pub fn insert_at(&mut self, pos: usize) -> Option<InnerTable<StrSlice>> {
        let binding = self.piece_table.write_full().unwrap();
        let mut to_split = binding.write();
        let mut cursor = to_split.cursor_front_mut();
        let mut curr_pos = cursor.current().unwrap().read().len();
        let is_end = loop {
            if curr_pos > pos {
                break false;
            }
            cursor.move_next();
            if let Some(x) = cursor.current() {
                curr_pos += x.read().len();
            } else {
                break true;
            }
        };

        if is_end {
            cursor.move_prev();
            cursor.insert_after(InnerTable::new(StrSlice::empty(), self.piece_table.state()));
        } else {
            let current = cursor.current().unwrap().read().clone();
            let offset = pos - (curr_pos - current.len());

            if offset != 0 {
                cursor.insert_before(InnerTable::new(
                    dbg!(current.subslice(..offset))?,
                    self.piece_table.state(),
                ));
            }

            cursor.insert_after(InnerTable::new(
                current.subslice(offset..)?,
                self.piece_table.state(),
            ));
            *cursor.current().unwrap().write().unwrap() = StrSlice::empty();
        }
        Some(cursor.current().unwrap().clone())
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
        ret.extend(self.buffers.original.slice(..).iter());
        for client in &self.buffers.clients {
            // 0xfe is used here because its not representable by utf8, and makes stuff easier to
            // parse. This is useful because the alternative is the specify the strings length,
            // which would take up at least as many bytes
            ret.push_back(0xfe);
            ret.extend(client.read().unwrap().slice(..).iter());
        }
        // Might be useless, but it's a single byte
        ret.push_back(0xff);

        ret.extend((self.piece_table.read_full().unwrap().read().len() as u64).to_be_bytes());

        for piece in self.piece_table.read_full().unwrap().read().iter() {
            let piece = piece.read();
            ret.extend((piece.start() as u64).to_be_bytes());
            ret.extend((piece.end()).to_be_bytes());
            ret.extend((piece.len() as u64).to_be_bytes());
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
            // This should be take while in order to actually consume the next value. This is
            // expected because it allows  for disambiguation between a value and and a control
            // value
            .chunks::<{ 3 * mem::size_of::<u64>() }>()
            .take(piece_count)
            .map(|x| {
                let _slices = x
                    .into_iter()
                    .chunks::<{ mem::size_of::<u64>() }>()
                    .collect::<Vec<_>>();
                // Arc::new(Range {
                //     buf: usize::try_from(u64::from_be_bytes(slices[0])).unwrap(),
                //     start: usize::try_from(u64::from_be_bytes(slices[1])).unwrap(),
                //     len: usize::try_from(u64::from_be_bytes(slices[2])).unwrap(),
                // })
                // StrSlice::empty()
                todo!()
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
    use crate::Piece;
    use std::io::BufReader;

    #[test]
    fn from_reader() {
        let mut bytes = &b"test"[..];
        let piece =
            Piece::original_from_reader(BufReader::with_capacity(bytes.len(), &mut bytes)).unwrap();
        assert!(&*piece.buffers.original.str_slice(..) == "test");
        assert_eq!(piece.buffers.clients.len(), 0);
        let binding = piece.piece_table.read_full().unwrap();
        let binding = binding.read();
        let mut iter = binding.iter();
        assert_eq!(&**iter.next().unwrap().read(), "test");
        assert!(iter.next().is_none());
    }

    #[test]
    fn insert() {
        let mut piece = Piece::new();
        let mut client = piece.add_client();

        client.enter_insert(piece.insert_at(0).unwrap());
        client.push_str("andy");

        let mut iter = piece.lines();
        assert_eq!(iter.next(), Some("andy".into()));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn two_clients_non_overlapping() {
        let mut text = Piece::new();
        let mut client = text.add_client();
        let mut client2 = text.add_client();

        client.enter_insert(text.insert_at(0).unwrap());
        client.push_str("andy");

        client2.enter_insert(text.insert_at(2).unwrap());
        client2.push_str("andy");

        let mut iter = text.lines();
        assert_eq!(iter.next(), Some("anandydy".into()));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn multiple_clients_lines() {
        let mut text = Piece::new();
        let mut client = text.add_client();
        let mut client2 = text.add_client();

        client.enter_insert(text.insert_at(0).unwrap());
        client.push_str("andy");

        let mut client3 = text.add_client();

        client2.enter_insert(text.insert_at(2).unwrap());
        client3.enter_insert(text.insert_at(4).unwrap());
        client2.push_str("andy");

        client3.push_str("\n\na");

        assert_eq!(
            text.piece_table
                .read_full()
                .unwrap()
                .read()
                .iter()
                .map(|x| x.read().as_str().to_string())
                .collect::<Vec<_>>(),
            vec!["an", "andy", "dy", "\n\na", ""]
        );
        let mut iter = text.lines();
        assert_eq!(iter.next(), Some("anandydy".into()));
        assert_eq!(iter.next(), Some("".into()));
        assert_eq!(iter.next(), Some("a".into()));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn multiple_inserts_single_client() {
        let mut text = Piece::new();
        let mut client = text.add_client();
        client.enter_insert(text.insert_at(0).unwrap());
        client.push_str("Hello");

        client.enter_insert(text.insert_at(5).unwrap());
        client.push_str("world!");

        client.enter_insert(text.insert_at(5).unwrap());
        client.push_str(" ");

        println!(
            "{:?}",
            text.piece_table
                .read_full()
                .unwrap()
                .read()
                .iter()
                .map(|x| x.read().as_str().to_string())
                .collect::<Vec<_>>()
        );
        let mut iter = text.lines();
        assert_eq!(iter.next(), Some("Hello world!".to_string()));
        assert_eq!(iter.next(), None);
    }
}
