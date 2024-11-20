//! Implements iterator types for Pieces
use append_only_str::{iters::Chars as AppendChars, StrSlice};

use crate::Piece;

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
        *current_iter = self.ranges.next().unwrap().owned_chars();
        self.next()
    }
}

pub struct Lines<T>
where
    T: Iterator<Item = StrSlice>,
{
    chars: Chars<T>,
}

impl<T> Iterator for Lines<T>
where
    T: Iterator<Item = StrSlice>,
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
    #[must_use]
    pub fn chars(
        &self,
    ) -> Chars<
        std::iter::Map<
            std::collections::linked_list::IntoIter<crate::table::InnerTable<StrSlice>>,
            impl FnMut(crate::table::InnerTable<StrSlice>) -> StrSlice,
        >,
    > {
        Chars {
            ranges: self
                .piece_table
                .table
                .read_full()
                .unwrap()
                .clone()
                .into_iter()
                .map(|x| x.read().unwrap().clone()),
            current_iter: None,
        }
    }

    pub fn lines(&self) -> impl Iterator<Item = String> {
        Lines {
            chars: self.chars(),
        }
    }
}

#[cfg(test)]
mod test {
    use std::{
        collections::LinkedList,
        iter,
        str::FromStr,
        sync::{Arc, RwLock},
    };

    use append_only_str::AppendOnlyStr;

    use crate::{table::Table, Buffers, Piece, PieceTable, Range};

    fn with_len(buf: usize, start: usize, len: usize) -> Arc<Range> {
        Arc::new(Range { buf, start, len })
    }

    #[test]
    fn test_chars_no_clients() {
        let text = "test\nmore tests\n";
        let original: AppendOnlyStr = text.into();
        let piece = Piece {
            piece_table: PieceTable {
                table: Table::from_iter(iter::once(original.str_slice(..))),
                cursors: vec![],
            },
            buffers: Buffers {
                original,
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
            piece_table: PieceTable {
                table: Table::from_iter(iter::once(original.str_slice(..))),
                cursors: vec![],
            },
            buffers: Buffers {
                original,
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
            piece_table: PieceTable {
                table: Table::from_iter(std::iter::once(original.str_slice(..))),
                cursors: vec![],
            },
            buffers: Buffers {
                original,
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
            piece_table: PieceTable {
                table: Table::from_iter(
                    [
                        original.str_slice(..),
                        Arc::clone(&client1).read().unwrap().str_slice(..),
                    ]
                    .into_iter(),
                ),
                cursors: vec![],
            },
            buffers: Buffers {
                original,
                clients: vec![client1],
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
            piece_table: PieceTable {
                table: Table::from_iter(
                    [
                        original.str_slice(0..1),
                        Arc::clone(&client1).read().unwrap().str_slice(0..1),
                        original.str_slice(1..3),
                        Arc::clone(&client1).read().unwrap().str_slice(1..3),
                    ]
                    .into_iter(),
                ),
                cursors: vec![],
            },
            buffers: Buffers {
                original,
                clients: vec![client1],
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
