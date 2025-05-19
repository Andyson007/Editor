//! A Piece table implementation with multiple clients
#![feature(linked_list_cursors)]
#![feature(async_iterator)]
use std::{
    io::{self, Read},
    iter,
    str::FromStr,
    sync::{Arc, RwLock},
};

pub mod iters;
pub mod table;

use append_only_str::{slices::StrSlice, AppendOnlyStr};
use btep::{Deserialize, Serialize};
use table::{InnerTable, LockError, Table};
use tokio::io::AsyncReadExt;
use utils::{
    bufread::BufReaderExt,
    other::{AutoIncrementing, CursorPos},
};

/// A wrapper around all the buffers
/// This includes the append buffers for each of the clients, and the original buffer
#[derive(Debug)]
pub struct Buffers {
    /// The original file content together with its id generator
    pub original: (AutoIncrementing, AppendOnlyStr),
    /// The appendbuffers for each of the clients
    #[allow(clippy::type_complexity)]
    pub clients: Vec<(Arc<RwLock<AutoIncrementing>>, Arc<RwLock<AppendOnlyStr>>)>,
}

/// A complete Piece table. It has support for handling multiple clients at the same time
#[derive(Debug)]
pub struct Piece {
    /// Holds the buffers that get modified when anyone inserts
    pub buffers: Buffers,
    /// stores the pieces to reconstruct the whole file
    pub piece_table: Table<TableElem>,
}

#[derive(Debug)]
/// An element of the piece table
pub struct TableElem {
    /// The buffer that the `text` is pointing to
    /// whether the this element is being edited
    pub buf: Option<(usize, bool)>,
    /// The id of this buffer
    pub id: usize,
    /// A slice to the text
    pub text: StrSlice,
}

impl Piece {
    #[must_use]
    /// Creates an empty piece table
    pub fn new() -> Self {
        let original: AppendOnlyStr = "".into();
        Self {
            piece_table: std::iter::once(TableElem {
                id: 0,
                buf: None,
                text: original
                    .str_slice(..)
                    .expect("A full slice is always valid"),
            })
            .collect(),
            buffers: Buffers {
                original: (AutoIncrementing::new(), original),
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
            piece_table: iter::once(TableElem {
                buf: None,
                id: 0,
                text: original
                    .str_slice(..)
                    .expect("A full slice is always valid"),
            })
            .collect(),
            buffers: Buffers {
                original: (AutoIncrementing::new(), original),
                clients: vec![],
            },
        })
    }

    /// Creates a new piece table using an &str as its base
    #[must_use]
    pub fn original_from_str(original: &str) -> Self {
        let original: AppendOnlyStr = original.into();

        Self {
            piece_table: iter::once(TableElem {
                buf: None,
                id: 0,
                text: original
                    .str_slice(..)
                    .expect("A full slice is always valid"),
            })
            .collect(),
            buffers: Buffers {
                original: (AutoIncrementing::new(), original),
                clients: vec![],
            },
        }
    }

    /// Creates an `InnerTable` within the piece table.
    /// This allows the list to be mutated at that point.
    /// # Panics
    /// Shouldn't panic
    pub fn insert_at(
        &mut self,
        pos: CursorPos,
        clientid: usize,
    ) -> Option<(Option<usize>, InnerTable<TableElem>)> {
        // FIXME:
        // Rather than this we should store the amount of lines in each `TableElem`
        let bytes_to_row: usize = self
            .lines()
            .take(pos.row)
            .map(|x| x.len() + '\n'.len_utf8())
            .sum();
        let char_nr = bytes_to_row
            + self
                .lines()
                .nth(pos.row)
                .unwrap_or_default()
                .chars()
                .take(pos.col)
                .map(char::len_utf8)
                .sum::<usize>();
        let binding = self
            .piece_table
            .write_full()
            .expect("The entire piece table is poisoned");

        let mut to_split = binding.write();
        let mut cursor = to_split.cursor_front_mut();
        let mut curr_pos = cursor
            .current()
            .expect("The Linked list is empty")
            .read()
            .text
            .len();
        let is_end = loop {
            if curr_pos > char_nr {
                break false;
            }
            cursor.move_next();
            if let Some(x) = cursor.current() {
                curr_pos += x.read().text.len();
            } else {
                break true;
            }
        };

        let offset = if is_end {
            // is_end implies that current refers to the ghost node. We know that we have a
            // non-zero amount of elements in the list
            let current = cursor.peek_prev().expect("prev isn't ghost");
            let buf = current.read().buf;
            let curr = if let Some((buf, _)) = current.read().buf {
                &*self.buffers.clients[buf]
                    .1
                    .read()
                    .expect("AppendOnlyStr got poisoned")
            } else {
                &self.buffers.original.1
            };

            cursor.insert_before(InnerTable::new(
                TableElem {
                    buf,
                    text: curr.str_slice_end(),
                    id: self.buffers.clients[clientid]
                        .0
                        .write()
                        .expect("Poison")
                        .get()
                        * self.buffers.clients.len()
                        + clientid,
                },
                self.piece_table.state(),
            ));
            None
        } else {
            let (buf_of_split, current) = {
                let current = cursor
                    .current()
                    .expect("Cursor should not be at the ghost element")
                    .read();
                (current.buf, current.text.clone())
            };
            // byte offset within the current buffer
            let offset = char_nr - (curr_pos - current.len());

            if offset != 0 {
                cursor.insert_before(InnerTable::new(
                    TableElem {
                        buf: buf_of_split.map(|x| (x.0, false)),
                        text: current
                            .subslice(..offset)
                            .expect("offset should be on a byte boundary"),
                        id: self.buffers.clients[clientid]
                            .0
                            .write()
                            .expect("Poison")
                            .get()
                            * self.buffers.clients.len()
                            + clientid,
                    },
                    self.piece_table.state(),
                ));
            }

            cursor.current().expect("Current is not on a utf-8 boundary").write().unwrap().text = current
                .subslice(offset..)
                .expect("offset is not on a byte boundary");
            Some(offset)
        };
        let curr = self.buffers.clients[clientid].1.read().unwrap();
        cursor.insert_before(InnerTable::new(
            TableElem {
                buf: Some((clientid, true)),
                text: curr.str_slice_end(),
                id: self.buffers.clients[clientid].0.write().expect("Poison").get()
                    * self.buffers.clients.len()
                    + clientid,
            },
            self.piece_table.state(),
        ));
        Some((offset, cursor.peek_prev().unwrap().clone()))
    }

    /// Locks down the full list for reading.
    /// This means that
    /// - No value within the list can be mutated
    /// - The order of elements cannot be changed
    /// # Errors
    /// - The state value is poisoned
    pub fn read_full(&self) -> Result<table::TableReader<TableElem>, LockError> {
        self.piece_table.read_full()
    }

    /// Locks down the full list for reading.
    /// This means that
    /// - This is the sole write lock on the full list
    /// - No reading lock can be made
    /// # Errors
    /// - The state value is poisoned
    pub fn write_full(&self) -> Result<table::TableWriter<TableElem>, LockError> {
        self.piece_table.write_full()
    }
}

impl Default for Piece {
    fn default() -> Self {
        Self::new()
    }
}

impl Serialize for &Piece {
    fn serialize(&self) -> Vec<u8> {
        let mut ret = Vec::new();
        ret.extend((self.buffers.original.0.peek() as u64).to_be_bytes());
        ret.extend(
            self.buffers
                .original
                .1
                .str_slice(..)
                .unwrap()
                .as_str()
                .bytes(),
        );
        for client in &self.buffers.clients {
            // 0xfe is used here because its not representable by utf8, and makes stuff easier to
            // parse. This is useful because the alternative is the specify the strings length,
            // which would take up at least as many bytes
            ret.push(0xfe);
            ret.extend((client.0.read().unwrap().peek() as u64).to_be_bytes());
            ret.extend(
                client
                    .1
                    .read()
                    .unwrap()
                    .str_slice(..)
                    .unwrap()
                    .as_str()
                    .bytes(),
            );
        }
        // Might be useless, but it's a single byte
        ret.push(0xff);
        ret.extend((self.piece_table.read_full().unwrap().read().len() as u64).to_be_bytes());

        for piece in self.piece_table.read_full().unwrap().read().iter() {
            let piece = piece.read();
            if let Some((bufnr, occupied)) = piece.buf {
                ret.extend([if occupied { 2 } else { 1 }]);
                ret.extend((bufnr as u64).to_be_bytes());
            } else {
                ret.push(0);
            }
            ret.extend((piece.id as u64).to_be_bytes());
            ret.extend((piece.text.start() as u64).to_be_bytes());
            ret.extend((piece.text.end() as u64).to_be_bytes());
        }
        ret
    }
}

impl Deserialize for Piece {
    async fn deserialize<T>(data: &mut T) -> io::Result<Self>
    where
        T: AsyncReadExt + Unpin + Send,
        Self: Sized,
    {
        let mut str_buf = String::new();
        let original_start = data.read_u64().await?;
        let mut separator = data.read_valid_str(&mut str_buf).await?;

        let original_buffer: AppendOnlyStr = AppendOnlyStr::from_str(&str_buf).unwrap();

        let mut client_buffers = Vec::new();
        loop {
            let mut str_buf = String::new();
            if separator == Some(255) {
                break;
            }
            let counter_start = data.read_u64().await? as usize;

            str_buf.clear();
            separator = data.read_valid_str(&mut str_buf).await?;

            client_buffers.push((
                Arc::new(RwLock::new(AutoIncrementing::new_with_start(counter_start))),
                Arc::new(RwLock::<AppendOnlyStr>::new(str_buf.into())),
            ));
        }

        let piece_count = data.read_u64().await? as usize;

        let mut builder = InnerTable::builder();
        for _ in 0..piece_count {
            let buf = match data.read_u8().await? {
                0 => None,
                1 => Some((data.read_u64().await? as usize, false)),
                2 => Some((data.read_u64().await? as usize, true)),
                _ => unreachable!(),
            };

            let id = data.read_u64().await? as usize;
            let start = data.read_u64().await? as usize;
            let end = data.read_u64().await? as usize;
            builder.push(TableElem {
                buf,
                text: if let Some((bufid, _)) = buf {
                    client_buffers[bufid]
                        .1
                        .read()
                        .unwrap()
                        .str_slice(start..end)
                        .unwrap()
                } else {
                    original_buffer.str_slice(start..end).unwrap()
                },
                id,
            });
        }

        Ok(Self {
            buffers: Buffers {
                original: (
                    AutoIncrementing::new_with_start(original_start as usize),
                    original_buffer,
                ),
                clients: client_buffers,
            },
            piece_table: Table::new(builder),
        })
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
        assert_eq!(&*next.text, "test");
        assert_eq!(next.buf, None);
        assert!(iter.next().is_none());
    }
}
