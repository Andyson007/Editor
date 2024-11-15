use std::{
    cell::Cell,
    collections::LinkedList,
    io::{self, Read},
    ops::Range,
    sync::RwLock,
};

use append_only_bytes::AppendOnlyBytes;
use btep::Serialize;

#[derive(Debug)]
struct Buffers {
    original: Box<str>,
    clients: Vec<AppendOnlyBytes>,
}

#[derive(Debug)]
pub struct Piece {
    buffers: Buffers,
    piece_table: PieceTable,
}

#[derive(Debug)]
struct PieceTable {
    table: LinkedList<Range<usize>>,
    cursors: Vec<RwLock<Box<str>>>,
}

impl Piece {
    pub fn original_from_reader<T: Read>(mut read: T) -> io::Result<Self> {
        let mut string = String::new();
        read.read_to_string(&mut string)?;
        let original = string.into_boxed_str();
        let mut list = LinkedList::new();
        list.push_back(0..original.len());
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

impl Serialize for Piece {
    fn serialize(&self) -> impl IntoIterator<Item = u8> {
        todo!();
        std::iter::empty()
    }
}

impl Serialize for &Piece {
    fn serialize(&self) -> impl IntoIterator<Item = u8> {
        todo!();
        std::iter::empty()
    }
}
