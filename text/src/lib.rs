//! A crate above the piece table for handling actual text with more helper functions
#![feature(linked_list_cursors)]

use std::{
    collections::VecDeque,
    io::{self, Read},
    sync::{Arc, RwLock},
};

use append_only_str::AppendOnlyStr;
use btep::{Deserialize, Serialize};
use client::{Client, Insertdata};
use piece_table::{table::InnerTable, Piece, TableElem};
use tokio::io::AsyncReadExt;
use utils::other::AutoIncrementing;
pub mod client;

/// A wrapper around a piece table.
/// It creates wrapper methods and adds support for multiple clients to interface more easily with
/// the piece table
#[derive(Debug)]
pub struct Text {
    pub(crate) table: Arc<RwLock<Piece>>,
    clients: Vec<Client>,
}

impl Serialize for &Text {
    fn serialize(&self) -> std::collections::VecDeque<u8> {
        let mut ret = VecDeque::new();
        let to_extend = (&*self.table.read().unwrap()).serialize();
        ret.extend((to_extend.len() as u64).to_be_bytes());
        ret.extend(to_extend);

        ret.extend((self.clients.len() as u64).to_be_bytes());

        ret.extend(self.clients.iter().flat_map(|x| {
            let mut ret = Vec::new();
            if let Some(Insertdata { slice, .. }) = &x.data {
                ret.push(1);
                ret.extend((slice.read().text.start() as u64).to_be_bytes());
                ret.extend((slice.read().text.end() as u64).to_be_bytes());
            } else {
                ret.push(0);
            }
            ret
        }));
        ret
    }
}

impl Deserialize for Text {
    async fn deserialize<T>(data: &mut T) -> io::Result<Self>
    where
        T: AsyncReadExt + Unpin + Send,
    {
        let _len = data.read_u64().await? as usize;
        // NOTE: We should probably limit the length of data here
        let piece = Piece::deserialize(data).await?;

        let arced = Arc::new(RwLock::new(piece));

        let client_count = data.read_u64().await? as usize;

        let mut clients = Vec::with_capacity(client_count as usize);
        for counter in 0..client_count {
            if data.read_u8().await? == 1 {
                let start = data.read_u64().await? as usize;
                let end = data.read_u64().await? as usize;

                clients.push(Client {
                    piece: Arc::clone(&arced),
                    buffer: Arc::clone(&arced.read().unwrap().buffers.clients[counter].1),
                    id_counter: Arc::clone(&arced.read().unwrap().buffers.clients[counter].0),
                    data: arced
                        .read()
                        .unwrap()
                        .piece_table
                        .read_full()
                        .unwrap()
                        .read()
                        .iter()
                        .find(|x| {
                            let inner = x.read();
                            if inner.buf.map(|(x, _)| x) != Some(counter) {
                                return false;
                            };
                            inner.text.start() == start && inner.text.end() == end
                        })
                        .cloned()
                        .map(|slice| Insertdata {
                            slice,
                            has_deleted: false,
                        }),
                    bufnr: counter,
                });
            } else {
                clients.push(Client {
                    piece: Arc::clone(&arced),
                    buffer: Arc::clone(&arced.read().unwrap().buffers.clients[counter].1),
                    id_counter: Arc::clone(&arced.read().unwrap().buffers.clients[counter].0),
                    data: None,
                    bufnr: counter,
                });
            }
        }
        Ok(Self {
            table: arced,
            clients,
        })
    }
}

impl Text {
    /// Creates a new piece table with the orginal buffer filled in from the reader.
    /// # Errors
    /// - The reader failed to read
    pub fn original_from_reader<T: Read>(read: T) -> io::Result<Self> {
        let piece = Piece::original_from_reader(read)?;
        Ok(Self::with_piece(piece))
    }

    /// Creates a new piece table with the orginal buffer filled in from the reader.
    /// # Errors
    /// - The reader failed to read
    #[must_use]
    pub fn original_from_str(original: &str) -> Self {
        let piece = Piece::original_from_str(original);
        Self::with_piece(piece)
    }

    /// Wraps an existsing piece inside a `Text`
    #[must_use]
    pub fn with_piece(piece: Piece) -> Self {
        Self {
            table: Arc::new(RwLock::new(piece)),
            clients: Vec::new(),
        }
    }

    /// Creates a new `Text` with an empty original buffer
    #[must_use]
    pub fn new() -> Self {
        Self {
            table: Arc::new(RwLock::new(Piece::new())),
            clients: Vec::new(),
        }
    }

    /// Creates a `Client` with an attached buffer
    /// # Panics
    /// probably only when failing to lock the buffers
    pub fn add_client(&mut self) -> usize {
        let buf = Arc::new(RwLock::new(AppendOnlyStr::new()));
        let counter = Arc::new(RwLock::new(AutoIncrementing::new()));
        self.table
            .write()
            .unwrap()
            .buffers
            .clients
            .push((Arc::clone(&counter), Arc::clone(&buf)));
        self.clients.push(Client::new(
            Arc::clone(&self.table),
            buf,
            self.clients.len(),
            counter,
        ));
        self.clients.len() - 1
    }

    /// Creates an iterator over the lines in the buffer
    /// # Panics
    /// A failed lock on reading the entire list
    pub fn lines(&self) -> impl Iterator<Item = String> {
        self.table.read().unwrap().lines()
    }

    /// Creates an iterator characters in the list
    /// # Panics
    /// A failed lock on reading the entire list
    pub fn chars(&self) -> impl Iterator<Item = char> {
        self.table.read().unwrap().chars()
    }

    /// Creates an iterator over the buffers of the table
    /// # Panics
    /// - Stuff got poisoned
    pub fn bufs(&self) -> impl Iterator<Item = InnerTable<TableElem>> {
        self.table.read().unwrap().bufs()
    }

    /// returns a mutable reference to a given client
    pub fn client(&mut self, idx: usize) -> &mut Client {
        &mut self.clients[idx]
    }

    pub fn clients(&self) -> &[Client] {
        &self.clients
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

    #[test]
    fn insert() {
        let mut text = Text::new();
        text.add_client();

        text.clients[0].enter_insert((0, 0).into());
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

        text.client(client).enter_insert((0, 0).into());
        text.client(client).push_str("andy");

        text.client(client2).enter_insert((0, 2).into());
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

        text.client(0).enter_insert((0, 0).into());
        text.client(0).push_str("andy");

        text.add_client();

        text.client(1).enter_insert((0, 2).into());
        text.client(2).enter_insert((0, 4).into());
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

        text.client(0).enter_insert((0, 0).into());
        text.client(0).push_str("Hello");

        text.client(0).enter_insert((0, 5).into());
        text.client(0).push_str("world!");

        text.client(0).enter_insert((0, 5).into());
        text.client(0).push_str(" ");

        let mut iter = text.lines();
        assert_eq!(iter.next(), Some("Hello world!".to_string()));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn backspace() {
        let mut text = Text::new();
        text.add_client();

        text.client(0).enter_insert((0, 0).into());
        text.client(0).push_str("Hello");

        text.client(0).enter_insert((0, 5).into());
        text.client(0).push_str("world!");

        text.client(0).enter_insert((0, 5).into());
        text.client(0).push_str(" ");

        text.client(0).enter_insert((0, 1).into());
        text.client(0).enter_insert((0, 2).into());

        text.client(0).backspace();

        println!(
            "{:?}",
            text.table
                .read()
                .unwrap()
                .read_full()
                .unwrap()
                .read()
                .iter()
                .map(|x| x.read().text.as_str().to_string())
                .collect::<Vec<_>>()
        );

        let mut iter = text.lines();
        assert_eq!(iter.next(), Some("Hllo world!".to_string()));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn backspace_typing() {
        let mut text = Text::new();
        text.add_client();
        text.clients[0].enter_insert((0, 0).into());
        text.clients[0].enter_insert((0, 0).into());
        text.clients[0].push_char('t');
        text.clients[0].push_char('e');
        text.clients[0].push_char('k');
        text.clients[0].push_char('s');
        text.clients[0].push_char('t');
        text.clients[0].backspace();
        text.clients[0].backspace();
        text.clients[0].backspace();
        text.clients[0].push_char('x');
        text.clients[0].push_char('t');

        println!(
            "{:?}",
            text.table
                .read()
                .unwrap()
                .read_full()
                .unwrap()
                .read()
                .iter()
                .collect::<Vec<_>>()
        );

        let mut iter = text.lines();
        assert_eq!(iter.next(), Some("text".to_string()));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn backspace_swap() {
        let mut text = Text::new();
        text.add_client();
        text.add_client();
        text.clients[0].enter_insert((0, 0).into());
        text.clients[0].push_char('t');
        text.clients[1].enter_insert((0, 1).into());
        text.clients[0].push_char('e');
        text.clients[0].backspace();
        text.clients[0].backspace();
        text.clients[0].push_char('t');
        text.clients[1].push_char('e');

        let mut iter = text.lines();
        assert_eq!(iter.next(), Some("te".to_string()));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn blocked_backspace() {
        let mut text = Text::new();
        text.add_client();
        text.add_client();
        text.clients[0].enter_insert((0, 0).into());
        // println!("{} {:#?}", line!(), text.client(0));
        text.clients[0].push_char('t');
        // println!("---------------------------------");
        // println!("{} {:#?}", line!(), text.client(0));
        text.clients[0].push_char('e');

        println!(
            "{} {:?}",
            line!(),
            text.client(0).data.as_ref().map(|x| x.slice.read().buf)
        );
        println!();
        text.clients[1].enter_insert((0, 1).into());
        println!("{} {:?}", line!(), text.bufs().collect::<Vec<_>>());
        println!(
            "{} {:?}",
            line!(),
            text.client(0).data.as_ref().map(|x| x.slice.read().buf)
        );
        text.clients[1].push_char('x');
        // println!("{} {:?}", line!(), text.bufs().collect::<Vec<_>>());
        text.clients[0].backspace();
        // println!("{} {:?}", line!(), text.bufs().collect::<Vec<_>>());
        text.clients[0].backspace();
        // println!("{} {:?}", line!(), text.bufs().collect::<Vec<_>>());
        text.clients[0].push_char('t');
        // println!("{} {:?}", line!(), text.bufs().collect::<Vec<_>>());
        text.clients[0].push_char('e');
        // println!("{} {:?}", line!(), text.bufs().collect::<Vec<_>>());

        let mut iter = text.lines();
        assert_eq!(iter.next(), Some("txte".to_string()));
        assert_eq!(iter.next(), None);
    }
}
