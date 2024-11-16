use std::{
    collections::LinkedList,
    io::{self, Read},
    sync::Arc,
};

pub mod iters;

use append_only_str::AppendOnlyStr;
use btep::{Deserialize, Serialize};

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
    // TODO: This doesn't send the piece table yet
    fn serialize(&self) -> impl IntoIterator<Item = u8> {
        todo!();
        let mut ret = std::iter::empty();
        // .map(|x| {
        //     x.into_iter()
        //         .flat_map(|x| [b'\\', *x].into_iter().skip(if *x == b']' { 0 } else { 1 }))
        // })
        ret
    }
}

impl Deserialize for Piece {
    fn deserialize(data: impl IntoIterator<Item = u8>) -> Self {
        let original = String::from_utf8(data.into_iter().collect())
            .unwrap()
            .into_boxed_str();
        Self {
            piece_table: PieceTable {
                table: LinkedList::from_iter(std::iter::once(Range {
                    buf: 0,
                    start: 0,
                    len: original.len(),
                })),
                // TODO: This should also be populated
                cursors: vec![],
            },
            buffers: Buffers {
                original,
                // TODO: This should be populated
                clients: vec![],
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
