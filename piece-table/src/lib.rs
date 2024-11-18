use std::{
    collections::{LinkedList, VecDeque},
    io::{self, Read},
    mem, str,
    sync::Arc,
};

pub mod iters;

use append_only_str::AppendOnlyStr;
use btep::{Deserialize, Serialize};
use utils::iters::{InnerIteratorExt, IteratorExt};

#[derive(Debug)]
struct Buffers {
    original: Box<str>,
    clients: Vec<Arc<AppendOnlyStr>>,
}

#[derive(Debug)]
pub struct Piece {
    buffers: Buffers,
    piece_table: PieceTable,
}

#[derive(Debug)]
struct PieceTable {
    table: LinkedList<Range>,
    cursors: Vec<Arc<AppendOnlyStr>>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Range {
    buf: usize,
    start: usize,
    len: usize,
}

impl Piece {
    #[must_use]
    pub fn new() -> Self {
        Self {
            buffers: Buffers {
                original: "".into(),
                clients: vec![],
            },
            piece_table: PieceTable {
                table: LinkedList::from_iter([Range {
                    buf: 0,
                    start: 0,
                    len: 0,
                }]),
                cursors: vec![],
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
        let original = string.into_boxed_str();
        let mut list = LinkedList::new();
        list.push_back(Range {
            buf: 0,
            start: 0,
            len: original.len(),
        });
        Ok(Self {
            buffers: Buffers {
                original,
                clients: vec![],
            },
            piece_table: PieceTable {
                table: list,
                cursors: vec![],
            },
        })
    }

    pub fn add_client(&mut self) -> Arc<AppendOnlyStr> {
        let append_only = AppendOnlyStr::new();
        let arc = Arc::new(append_only);
        self.piece_table.cursors.push(Arc::clone(&arc));
        self.buffers.clients.push(Arc::clone(&arc));
        arc
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
        ret.extend(self.buffers.original.bytes());
        for client in &self.buffers.clients {
            // 0xfe is used here because its not representable by utf8, and makes stuff easier to
            // parse. This is useful because the alternative is the specify the strings length,
            // which would take up at least as many bytes
            ret.push_back(0xfe);
            ret.extend(client.slice(..).iter());
        }
        // Might be useless, but it's a single byte
        ret.push_back(0xff);

        let cursors = self.piece_table.cursors;

        ret.extend((self.piece_table.table.len() as u64).to_be_bytes());
        for piece in &self.piece_table.table {
            ret.extend((piece.buf as u64).to_be_bytes());
            ret.extend((piece.start as u64).to_be_bytes());
            ret.extend((piece.len as u64).to_be_bytes());
        }
        todo!("The cursors aren't sent yet");
        ret
    }
}

impl Deserialize for Piece {
    fn deserialize(data: &[u8]) -> Self {
        let mut iter = data.iter().cloned().peekable();

        let original_buffer = String::from_utf8(
            iter.take_while_ref(|x| !(*x == 254 || *x == 255))
                .collect::<Vec<_>>(),
        )
        .unwrap()
        .into_boxed_str();

        let mut client_buffers = Vec::new();
        loop {
            if iter.next() == Some(255) {
                break;
            }
            client_buffers.push(Arc::new(
                iter.by_ref()
                    .take_while(|x| !(*x == 254 || *x == 255))
                    .collect::<AppendOnlyStr>(),
            ))
        }
        let pieces: [u8; 8] = iter
            .by_ref()
            .take(mem::size_of::<u64>())
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();
        let piece_count = u64::from_be_bytes(pieces) as usize;

        let table = iter
            .by_ref()
            .take(piece_count)
            // This should be take while in order to actually consume the next value. This is
            // expected because it allows  for disambiguation between a value and and a control
            // value
            .chunks::<{ 3 * mem::size_of::<u64>() }>()
            .map(|x| {
                let slices = x
                    .into_iter()
                    .chunks::<{ mem::size_of::<u64>() }>()
                    .collect::<Vec<_>>();
                Range {
                    buf: u64::from_be_bytes(slices[0]) as usize,
                    start: u64::from_be_bytes(slices[1]) as usize,
                    len: u64::from_be_bytes(slices[2]) as usize,
                }
            })
            .collect();

        Self {
            buffers: Buffers {
                original: original_buffer,
                clients: client_buffers,
            },
            piece_table: PieceTable {
                table,
                cursors: todo!(),
            },
        }
    }
}

#[cfg(test)]
mod test {
    use std::io::BufReader;

    use crate::{Piece, Range};

    #[test]
    fn from_reader() {
        let mut bytes = &b"test"[..];
        let piece =
            Piece::original_from_reader(BufReader::with_capacity(bytes.len(), &mut bytes)).unwrap();
        assert!(piece.buffers.original == "test".to_string().into_boxed_str());
        assert_eq!(piece.buffers.clients.len(), 0);
        let mut iter = piece.piece_table.table.into_iter();
        assert_eq!(
            iter.next(),
            Some(Range {
                buf: 0,
                start: 0,
                len: 4,
            })
        );
        assert_eq!(iter.next(), None);
        assert_eq!(piece.piece_table.cursors.len(), 0);
    }

    #[test]
    fn serialize_original() {
        let mut bytes = &b"test"[..];
        let piece =
            Piece::original_from_reader(BufReader::with_capacity(bytes.len(), &mut bytes)).unwrap();
    }

    #[test]
    fn insert() {
        let mut piece = Piece::new();
        piece.add_client();
    }
}
