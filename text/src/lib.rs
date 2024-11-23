//! A crate above the piece table for handling actual text with more helper functions

use std::{
    ops::Index,
    sync::{Arc, RwLock},
};

use append_only_str::AppendOnlyStr;
use client::Client;
use piece_table::Piece;
pub mod client;

pub struct Text {
    table: Arc<RwLock<Piece>>,
    clients: Vec<Client>,
}

impl Text {
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
            self.table.read().unwrap().buffers.clients.len() - 1,
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
