//! A Piece table implementation with multiple clients
#![feature(linked_list_cursors)]
use std::{
    collections::{LinkedList, VecDeque},
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
    piece_table: PieceTable,
}

#[derive(Debug)]
struct PieceTable {
    #[allow(clippy::linkedlist)]
    table: Table<StrSlice>,
    cursors: Vec<Cursor>,
}

#[derive(Debug)]
struct Cursor {
    /// The buffer that this client owns. It's an Arc because it refers to one of Buffers' clients'
    /// element
    buffer: Arc<RwLock<AppendOnlyStr>>,
    location: Option<InnerTable<StrSlice>>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Range {
    pub buf: usize,
    pub start: usize,
    pub len: usize,
}

impl Piece {
    #[must_use]
    pub fn new() -> Self {
        let original: AppendOnlyStr = "".into();
        Self {
            piece_table: PieceTable {
                table: Table::from_iter(std::iter::once(original.str_slice(..))),
                cursors: vec![],
            },
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
            piece_table: PieceTable {
                table: Table::from_iter(iter::once(original.str_slice(..))),
                cursors: vec![],
            },
            buffers: Buffers {
                original,
                clients: vec![],
            },
        })
    }

    pub fn add_client(&mut self) -> Client {
        let append_only = AppendOnlyStr::new();
        let buf = Arc::new(RwLock::new(append_only));
        self.piece_table.cursors.push(Cursor {
            buffer: Arc::clone(&buf),
            location: None,
        });
        self.buffers.clients.push(Arc::clone(&buf));
        Client::new(buf)
    }

    pub fn insert_at(&mut self, pos: usize) -> InnerTable<StrSlice> {
        let (curr_pos, nodenr) = {
println!("[{}:{}]: {:?}", file!(), line!(), self.piece_table.table.state.read());
            let binding = self.piece_table.table.write_full().unwrap();
println!("[{}:{}]: {:?}", file!(), line!(), self.piece_table.table.state.read());
            let mut to_split = binding.write();
            let mut cursor = to_split.cursor_front_mut();
            let mut curr_pos = 0;
            let mut nodenr = 0;
            loop {
                if curr_pos >= pos {
                    break;
                }
                curr_pos += cursor.current().unwrap().read().unwrap().len();
                cursor.move_next();
                nodenr += 1;
            }

            cursor.insert_before(InnerTable::new(
                StrSlice::empty(),
                self.piece_table.table.state(),
            ));
            (curr_pos, nodenr)
        };
        let offset = curr_pos - pos;
        let binding = &self.piece_table.table.get(nodenr + 1);
        let mut x = binding.write().unwrap();
        *x = x.subslice(offset..);
        self.piece_table.table.get(nodenr)
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

        ret.extend((self.piece_table.table.read_full().unwrap().read().len() as u64).to_be_bytes());

        for piece in self.piece_table.table.read_full().unwrap().read().iter() {
            let piece = piece.read().unwrap();
            todo!("find the current buffer");
            ret.extend((5u64).to_be_bytes());
            ret.extend((piece.start() as u64).to_be_bytes());
            ret.extend((piece.len() as u64).to_be_bytes());
        }

        for cursor in &self.piece_table.cursors {
            let Some(ref current) = cursor.location else {
                ret.push_back(0);
                continue;
            };
            ret.push_back(1);
            // ret.extend((current.buf as u64).to_be_bytes());
            // ret.extend((current.start as u64).to_be_bytes());
            // ret.extend((current.len as u64).to_be_bytes());
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
        let table: LinkedList<Arc<Range>> = iter
            .by_ref()
            // This should be take while in order to actually consume the next value. This is
            // expected because it allows  for disambiguation between a value and and a control
            // value
            .chunks::<{ 3 * mem::size_of::<u64>() }>()
            .take(piece_count)
            .map(|x| {
                let slices = x
                    .into_iter()
                    .chunks::<{ mem::size_of::<u64>() }>()
                    .collect::<Vec<_>>();
                Arc::new(Range {
                    buf: usize::try_from(u64::from_be_bytes(slices[0])).unwrap(),
                    start: usize::try_from(u64::from_be_bytes(slices[1])).unwrap(),
                    len: usize::try_from(u64::from_be_bytes(slices[2])).unwrap(),
                })
            })
            .collect();

        let mut cursors = Vec::with_capacity(client_buffers.len());
        while let Some(cursor) = iter.next() {
            if cursor == 0 {
                cursors.push(Cursor {
                    buffer: Arc::clone(&client_buffers[cursors.len()]),
                    location: None,
                });
                continue;
            }
            debug_assert_eq!(cursor, 1);
            let chunks = iter
                .by_ref()
                .chunks::<{ 3 * mem::size_of::<u64>() }>()
                .take(3)
                .flat_map(|x| x.into_iter().chunks::<{ mem::size_of::<u64>() }>())
                .collect::<Vec<_>>();

            cursors.push(Cursor {
                buffer: Arc::clone(&client_buffers[cursors.len()]),
                location: Some(todo!()), // location: Some(Arc::new(Range {
                                         //     buf: usize::try_from(u64::from_be_bytes(chunks[0])).unwrap(),
                                         //     start: usize::try_from(u64::from_be_bytes(chunks[1])).unwrap(),
                                         //     len: usize::try_from(u64::from_be_bytes(chunks[2])).unwrap(),
                                         // })),
            });
        }

        Self {
            buffers: Buffers {
                original: original_buffer,
                clients: client_buffers,
            },
            piece_table: PieceTable {
                table: todo!(),
                cursors,
            },
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
        let binding = piece.piece_table.table.read_full().unwrap();
        let binding = binding.read();
        let mut iter = binding.iter();
        assert_eq!(&**iter.next().unwrap().read().unwrap(), "test");
        assert!(iter.next().is_none());
        assert_eq!(piece.piece_table.cursors.len(), 0);
    }

    #[test]
    fn insert() {
        let mut piece = Piece::new();
        let mut client = piece.add_client();

        client.enter_insert(piece.insert_at(0));
        client.push_str("andy");

        let mut iter = piece.lines();
        assert_eq!(iter.next(), Some("andy".into()));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn multiple_clients() {
        let mut piece = Piece::new();
        let mut client = piece.add_client();
        let mut client2 = piece.add_client();
        let inner_table = piece.insert_at(0);

        client.enter_insert(inner_table);
        client.push_str("andy");
        client2.enter_insert(piece.insert_at(2));
        client2.push_str("andy");

        let mut iter = piece.lines();
        assert_eq!(iter.next(), Some("andy".into()));
        assert_eq!(iter.next(), None);
    }
}
