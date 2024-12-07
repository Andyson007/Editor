//! A Piece table implementation with multiple clients
#![feature(linked_list_cursors)]
#![feature(async_iterator)]
use std::{
    collections::VecDeque,
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
    /// The original file content
    pub original: AppendOnlyStr,
    /// The appendbuffers for each of the clients
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
    pub bufnr: Option<usize>,
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
                bufnr: None,
                text: original.str_slice(..),
            })
            .collect(),
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
            piece_table: iter::once(TableElem {
                bufnr: None,
                id: 0,
                text: original.str_slice(..),
            })
            .collect(),
            buffers: Buffers {
                original,
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
                bufnr: None,
                id: 0,
                text: original.str_slice(..),
            })
            .collect(),
            buffers: Buffers {
                original,
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
        let bytes_to_row: usize = self.lines().take(pos.row).map(|x| x.len() + 1).sum();
        let char_nr = bytes_to_row + pos.col;
        let binding = self.piece_table.write_full().unwrap();
        let mut to_split = binding.write();
        let mut cursor = to_split.cursor_front_mut();
        let mut curr_pos = cursor.current().unwrap().read().text.len();
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
            // NOTE: We rely on the cursors position later
            cursor.move_prev();
            let curr = self.buffers.clients[clientid].1.read().unwrap();
            cursor.insert_after(InnerTable::new(
                TableElem {
                    bufnr: Some(clientid),
                    text: curr.str_slice(curr.len()..),
                    id: self.buffers.clients[clientid].0.write().unwrap().get()
                        * self.buffers.clients.len()
                        + clientid,
                },
                self.piece_table.state(),
            ));
            None
        } else {
            let (buf_of_split, current) = {
                let current = cursor.current().unwrap().read();
                (current.bufnr, current.text.clone())
            };
            let offset = char_nr - (curr_pos - current.len());

            if offset != 0 {
                cursor.insert_before(InnerTable::new(
                    TableElem {
                        bufnr: buf_of_split,
                        text: current.subslice(..offset)?,
                        id: self.buffers.clients[clientid].0.write().unwrap().get()
                            * self.buffers.clients.len()
                            + clientid,
                    },
                    self.piece_table.state(),
                ));
            }

            cursor.insert_after(InnerTable::new(
                TableElem {
                    bufnr: buf_of_split,
                    text: current.subslice(offset..)?,
                    id: self.buffers.clients[clientid].0.write().unwrap().get()
                        * self.buffers.clients.len()
                        + clientid,
                },
                self.piece_table.state(),
            ));
            let curr = self.buffers.clients[clientid].1.read().unwrap();
            cursor.current().unwrap().write().unwrap().text = curr.str_slice(curr.len()..);
            cursor.current().unwrap().write().unwrap().bufnr = Some(clientid);
            Some(offset)
        };
        Some((offset, cursor.current().unwrap().clone()))
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
    fn serialize(&self) -> std::collections::VecDeque<u8> {
        let mut ret = VecDeque::new();
        ret.extend(self.buffers.original.str_slice(..).as_str().bytes());
        for client in &self.buffers.clients {
            // 0xfe is used here because its not representable by utf8, and makes stuff easier to
            // parse. This is useful because the alternative is the specify the strings length,
            // which would take up at least as many bytes
            ret.push_back(0xfe);
            ret.extend((client.0.read().unwrap().peek() as u64).to_be_bytes());
            ret.extend(client.1.read().unwrap().str_slice(..).as_str().bytes());
        }
        // Might be useless, but it's a single byte
        ret.push_back(0xff);
        ret.extend((self.piece_table.read_full().unwrap().read().len() as u64).to_be_bytes());

        for piece in self.piece_table.read_full().unwrap().read().iter() {
            let piece = piece.read();
            // NOTE: This probably shouldn't use u64::MAX, but idk about a better way
            ret.extend((piece.bufnr.map_or(u64::MAX, |x| x as u64)).to_be_bytes());
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
        // `take_while_ref` requires a peekable wrapper
        // #[allow(clippy::unused_peekable)]
        // let mut iter = data.bytes().peekable();
        let mut str_buf = String::new();
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
            let bufnr = data.read_u64().await?;
            let id = data.read_u64().await? as usize;
            let start = data.read_u64().await? as usize;
            let end = data.read_u64().await? as usize;
            builder.push(match bufnr {
                u64::MAX => TableElem {
                    bufnr: None,
                    text: original_buffer.str_slice(start..end),
                    id,
                },
                bufnr => {
                    let buf = usize::try_from(bufnr).unwrap();
                    TableElem {
                        bufnr: Some(buf),
                        text: client_buffers[buf].1.read().unwrap().str_slice(start..end),
                        id,
                    }
                }
            });
        }

        Ok(Self {
            buffers: Buffers {
                original: original_buffer,
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
        assert_eq!(next.bufnr, None);
        assert!(iter.next().is_none());
    }
}
