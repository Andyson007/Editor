//! Implements iterator types for Pieces
use append_only_str::{iters::Chars as AppendChars, slices::StrSlice};

use crate::{table::InnerTable, Piece, TableElem};

/// An iterator over the chars of a piece.
/// This locks the `Piece` for writing
pub struct Chars<T>
where
    T: Iterator<Item = StrSlice>,
{
    ranges: T,
    current_iter: Option<AppendChars>,
}

impl<T> Iterator for Chars<T>
where
    T: Iterator<Item = StrSlice>,
{
    type Item = char;

    fn next(&mut self) -> Option<Self::Item> {
        let Some(ref mut current_iter) = self.current_iter else {
            self.current_iter = Some(self.ranges.next()?.owned_chars());
            return self.next();
        };
        if let Some(next) = current_iter.next() {
            return Some(next);
        }
        *current_iter = self.ranges.next()?.owned_chars();
        self.next()
    }
}

/// Iterates over the piece table
/// This locks the `Piece` for writing
pub struct Lines<T>
where
    T: Iterator<Item = char>,
{
    chars: T,
}

impl<T> Iterator for Lines<T>
where
    T: Iterator<Item = char>,
{
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        let mut ret = String::new();
        for x in self.chars.by_ref() {
            if x == '\n' {
                return Some(ret);
            }
            ret.push(x);
        }
        if ret.is_empty() {
            None
        } else {
            Some(ret)
        }
    }
}

impl Piece {
    /// Iterates over the piece table one char at the time.
    /// # Panics
    /// panics if a lock can't be made on the full piece table
    pub fn chars(&self) -> impl Iterator<Item = char> {
        Chars {
            ranges: self
                .piece_table
                .read_full()
                .unwrap()
                .read()
                .clone()
                .into_iter()
                .map(|x| x.read().text.clone()),
            current_iter: None,
        }
    }

    /// Creates an iterator over the lines of the file
    /// # Panics
    /// panics if a lock can't be made on the full piece table
    pub fn lines(&self) -> impl Iterator<Item = String> {
        Lines {
            chars: self.chars(),
        }
    }

    /// Creates an iterator over the internal buffers of the piece table.
    /// # Panics
    /// the piece tables state got poisoned
    pub fn bufs(&self) -> impl Iterator<Item = InnerTable<TableElem>> {
        self.piece_table
            .read_full()
            .unwrap()
            .read()
            .clone()
            .into_iter()
            .map(|x| x)
    }
}

#[cfg(test)]
mod test {
    use std::{
        iter,
        sync::{Arc, RwLock},
    };

    use append_only_str::AppendOnlyStr;
    use utils::other::AutoIncrementing;

    use crate::{table::Table, Buffers, Piece, TableElem};

    #[test]
    fn test_chars_no_clients() {
        let text = "test\nmore tests\n";
        let original: AppendOnlyStr = text.into();
        let piece = Piece {
            piece_table: iter::once(TableElem {
                buf: None,
                text: original.str_slice(..),
                id: 0,
            })
            .collect(),
            buffers: Buffers {
                original: (AutoIncrementing::new(), original),
                clients: vec![],
            },
        };
        let mut chars = piece.chars();
        assert_eq!(chars.next(), Some('t'));
        assert_eq!(chars.next(), Some('e'));
    }

    #[test]
    fn lines_no_clients() {
        let text = "test\nmore tests\n";
        let original: AppendOnlyStr = text.into();
        let piece = Piece {
            piece_table: iter::once(TableElem {
                buf: None,
                id: 0,
                text: original.str_slice(..),
            })
            .collect(),
            buffers: Buffers {
                original: (AutoIncrementing::new(), original),
                clients: vec![],
            },
        };

        let mut lines = piece.lines();
        assert_eq!(lines.next(), Some("test".to_string()));
        assert_eq!(lines.next(), Some("more tests".to_string()));
        assert_eq!(lines.next(), None);
    }

    #[test]
    fn trailing_no_clients() {
        let text = "test\nmore tests\na";
        let original: AppendOnlyStr = text.into();
        let piece = Piece {
            piece_table: Table::from_iter(std::iter::once(TableElem {
                buf: None,
                id: 0,
                text: original.str_slice(..),
            })),
            buffers: Buffers {
                original: (AutoIncrementing::new(), original),
                clients: vec![],
            },
        };

        let mut lines = piece.lines();
        assert_eq!(lines.next(), Some("test".to_string()));
        assert_eq!(lines.next(), Some("more tests".to_string()));
        assert_eq!(lines.next(), Some("a".to_string()));
        assert_eq!(lines.next(), None);
    }

    #[test]
    fn chars_one_client_trailing() {
        let original: AppendOnlyStr = "abc".into();
        let client1: Arc<RwLock<AppendOnlyStr>> = Arc::new(RwLock::new("def".into()));
        let piece = Piece {
            piece_table: [
                TableElem {
                    buf: None,
                    text: original.str_slice(..),
                    id: 0,
                },
                TableElem {
                    buf: Some((0, false)),
                    text: Arc::clone(&client1).read().unwrap().str_slice(..),
                    id: 1,
                },
            ]
            .into_iter()
            .collect(),

            buffers: Buffers {
                original: (AutoIncrementing::new(), original),
                clients: vec![(Arc::new(RwLock::new(AutoIncrementing::new())), client1)],
            },
        };

        let mut chars = piece.chars();
        assert_eq!(chars.next(), Some('a'));
        assert_eq!(chars.next(), Some('b'));
        assert_eq!(chars.next(), Some('c'));
        assert_eq!(chars.next(), Some('d'));
        assert_eq!(chars.next(), Some('e'));
        assert_eq!(chars.next(), Some('f'));
        assert_eq!(chars.next(), None);
    }

    #[test]
    fn chars_one_client_interleaved() {
        let original: AppendOnlyStr = "acd".into();
        let client1: Arc<RwLock<AppendOnlyStr>> = Arc::new(RwLock::new("bef".into()));
        let piece = Piece {
            piece_table: Table::from_iter([
                TableElem {
                    buf: None,
                    id: 0,
                    text: original.str_slice(0..1),
                },
                TableElem {
                    buf: Some((0, false)),
                    text: Arc::clone(&client1).read().unwrap().str_slice(0..1),
                    id: 1,
                },
                TableElem {
                    buf: None,
                    id: 2,
                    text: original.str_slice(1..3),
                },
                TableElem {
                    buf: Some((0, false)),
                    text: Arc::clone(&client1).read().unwrap().str_slice(1..3),
                    id: 3,
                },
            ]),
            buffers: Buffers {
                original: (AutoIncrementing::new(), original),
                clients: vec![(Arc::new(RwLock::new(AutoIncrementing::new())), client1)],
            },
        };

        let mut chars = piece.chars();
        assert_eq!(chars.next(), Some('a'));
        assert_eq!(chars.next(), Some('b'));
        assert_eq!(chars.next(), Some('c'));
        assert_eq!(chars.next(), Some('d'));
        assert_eq!(chars.next(), Some('e'));
        assert_eq!(chars.next(), Some('f'));
        assert_eq!(chars.next(), None);
    }
}
