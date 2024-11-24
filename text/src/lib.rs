//! A crate above the piece table for handling actual text with more helper functions

use std::{
    collections::VecDeque,
    io,
    io::Read,
    mem,
    sync::{Arc, RwLock},
};

use append_only_str::AppendOnlyStr;
use btep::{Deserialize, Serialize};
use client::Client;
use piece_table::Piece;
use utils::iters::IteratorExt;
pub mod client;

#[derive(Debug)]
pub struct Text {
    table: Arc<RwLock<Piece>>,
    clients: Vec<Client>,
}

impl Serialize for &Text {
    fn serialize(&self) -> std::collections::VecDeque<u8> {
        let mut ret = VecDeque::new();
        let to_extend = (&*self.table.read().unwrap()).serialize();
        ret.extend((to_extend.len() as u64).to_be_bytes());
        println!("{}", ret.len());
        ret.extend(to_extend);
        ret.extend(self.clients.iter().flat_map(|x| {
            let mut ret = VecDeque::new();
            if let Some(x) = &x.slice {
                ret.push_back(1);
                ret.extend((x.read().1.start() as u64).to_be_bytes());
                ret.extend((x.read().1.end() as u64).to_be_bytes());
            } else {
                ret.push_back(0);
            }
            ret
        }));
        ret
    }
}

impl Deserialize for Text {
    fn deserialize(data: &[u8]) -> Self {
        let mut iter = data.iter();
        let len = u64::from_be_bytes(
            iter.by_ref()
                .cloned()
                .chunks::<{ mem::size_of::<u64>() }>()
                .next()
                .unwrap(),
        ) as usize;
        let piece: Piece = Deserialize::deserialize(
            iter.by_ref()
                .cloned()
                .take(len)
                .collect::<Vec<_>>()
                .as_slice(),
        );

        let arced = Arc::new(RwLock::new(piece));
        let mut counter = 0;

        let mut clients = Vec::new();

        while let Some(x) = iter.next() {
            if *x == 0 {
                let start = u64::from_be_bytes(
                    iter.by_ref()
                        .cloned()
                        .chunks::<{ mem::size_of::<u64>() }>()
                        .next()
                        .unwrap(),
                ) as usize;
                let end = u64::from_be_bytes(
                    iter.by_ref()
                        .cloned()
                        .chunks::<{ mem::size_of::<u64>() }>()
                        .next()
                        .unwrap(),
                ) as usize;

                clients.push(Client {
                    piece: Arc::clone(&arced),
                    buffer: Arc::clone(&arced.read().unwrap().buffers.clients[counter]),
                    slice: arced
                        .read()
                        .unwrap()
                        .piece_table
                        .read_full()
                        .unwrap()
                        .read()
                        .iter()
                        .find(|x| {
                            let (buf, inner) = x.read();
                            if buf != Some(counter) {
                                return false;
                            };
                            inner.start() == start && inner.end() == end
                        })
                        .cloned(),
                    bufnr: counter,
                });
            } else {
                clients.push(Client {
                    piece: Arc::clone(&arced),
                    buffer: Arc::clone(&arced.read().unwrap().buffers.clients[counter]),
                    slice: None,
                    bufnr: counter,
                });
            }
            counter += 1;
        }
        Self {
            table: arced,
            clients,
        }
    }
}

impl Text {
    pub fn original_from_reader<T: Read>(read: T) -> io::Result<Self> {
        let piece = Piece::original_from_reader(read)?;
        Ok(Self::with_piece(piece))
    }
    pub fn with_piece(piece: Piece) -> Self {
        Self {
            table: Arc::new(RwLock::new(piece)),
            clients: Vec::new(),
        }
    }

    pub fn new() -> Self {
        Self {
            table: Arc::new(RwLock::new(Piece::new())),
            clients: Vec::new(),
        }
    }

    /// Creates a `Client` with an attached buffer
    pub fn add_client(&mut self) -> usize {
        let buf = Arc::new(RwLock::new(AppendOnlyStr::new()));
        self.table
            .write()
            .unwrap()
            .buffers
            .clients
            .push(Arc::clone(&buf));
        self.clients.push(Client::new(
            Arc::clone(&self.table),
            buf,
            self.clients.len(),
        ));
        self.clients.len() - 1
    }

    pub fn lines(&self) -> impl Iterator<Item = String> {
        self.table.read().unwrap().lines()
    }

    pub fn chars(&self) -> impl Iterator<Item = char> {
        self.table.read().unwrap().chars()
    }

    pub fn client(&mut self, idx: usize) -> &mut Client {
        &mut self.clients[idx]
    }
}

impl Default for Text {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod test {
    use crate::Text;
    use piece_table::Piece;

    #[test]
    fn insert() {
        let mut text = Text::new();
        text.add_client();

        text.clients[0].enter_insert(0);
        text.clients[0].push_str("andy");

        let mut iter = text.lines();
        assert_eq!(iter.next(), Some("andy".into()));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn two_clients_non_overlapping() {
        let mut text = Text::new();
        let client = text.add_client();
        let client2 = text.add_client();

        text.client(client).enter_insert(0);
        text.client(client).push_str("andy");

        text.client(client2).enter_insert(2);
        text.client(client2).push_str("andy");

        let mut iter = text.lines();
        assert_eq!(iter.next(), Some("anandydy".into()));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn multiple_clients_lines() {
        let mut text = Text::new();
        text.add_client();
        text.add_client();

        text.client(0).enter_insert(0);
        text.client(0).push_str("andy");

        text.add_client();

        text.client(1).enter_insert(2);
        text.client(2).enter_insert(4);
        text.client(1).push_str("andy");

        text.client(2).push_str("\n\na");
        let mut iter = text.lines();
        assert_eq!(iter.next(), Some("anandydy".into()));
        assert_eq!(iter.next(), Some("".into()));
        assert_eq!(iter.next(), Some("a".into()));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn multiple_inserts_single_client() {
        let mut text = Text::new();
        text.add_client();

        text.client(0).enter_insert(0);
        text.client(0).push_str("Hello");

        text.client(0).enter_insert(5);
        text.client(0).push_str("world!");

        text.client(0).enter_insert(5);
        text.client(0).push_str(" ");

        let mut iter = text.lines();
        assert_eq!(iter.next(), Some("Hello world!".to_string()));
        assert_eq!(iter.next(), None);
    }
}
