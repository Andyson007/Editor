use std::ops::RangeInclusive;
use std::{
    collections::LinkedList,
    io::{self, Read},
    sync::RwLock,
};

pub mod iters;

use append_only_str::AppendOnlyStr;
use btep::{Deserialize, Serialize};

#[derive(Debug)]
struct Buffers {
    original: Box<str>,
    clients: Vec<AppendOnlyStr>,
}

#[derive(Debug)]
pub struct Piece {
    buffers: Buffers,
    piece_table: PieceTable,
}

#[derive(Debug)]
struct PieceTable {
    table: LinkedList<Range>,
    cursors: Vec<RwLock<Box<str>>>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Range {
    buf: usize,
    start: usize,
    len: usize,
}

impl Piece {
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
}

impl Serialize for &Piece {
    fn serialize(&self) -> impl IntoIterator<Item = u8> {
        std::iter::empty()
    }
}

impl Deserialize for Piece {
    fn deserialize(data: impl IntoIterator<Item = u8>) -> Self {
        todo!()
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
}
