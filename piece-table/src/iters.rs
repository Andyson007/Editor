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
    use std::{collections::LinkedList, str::FromStr, sync::Arc};

    use append_only_str::AppendOnlyStr;

    use crate::{Buffers, Piece, PieceTable, Range};

    fn with_len(buf: usize, start: usize, len: usize) -> Arc<Range> {
        Arc::new(Range { buf, start, len })
    }

    #[test]
    fn test_chars_no_clients() {
        let text = "test\nmore tests\n";
        let piece = Piece {
            buffers: Buffers {
                original: text.to_string().into_boxed_str(),
                clients: vec![],
            },
            piece_table: PieceTable {
                table: LinkedList::from_iter(std::iter::once(with_len(0, 0, text.len()))),
                cursors: vec![],
            },
        };
        let mut chars = piece.chars();
        assert_eq!(chars.next(), Some('t'));
        assert_eq!(chars.next(), Some('e'));
    }

    #[test]
    fn lines_no_clients() {
        let text = "test\nmore tests\n";
        let piece = Piece {
            buffers: Buffers {
                original: text.to_string().into_boxed_str(),
                clients: vec![],
            },
            piece_table: PieceTable {
                table: LinkedList::from_iter(std::iter::once(with_len(0, 0, text.len()))),
                cursors: vec![],
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
        let piece = Piece {
            buffers: Buffers {
                original: text.to_string().into_boxed_str(),
                clients: vec![],
            },
            piece_table: PieceTable {
                table: LinkedList::from_iter(std::iter::once(with_len(0, 0, text.len()))),
                cursors: vec![],
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
        let original = "abc";
        let client1 = "def";
        let piece = Piece {
            buffers: Buffers {
                original: original.to_string().into_boxed_str(),
                clients: vec![Arc::new(AppendOnlyStr::from_str(client1).unwrap())],
            },
            piece_table: PieceTable {
                table: LinkedList::from_iter(
                    [
                        Range {
                            buf: 0,
                            start: 0,
                            len: original.len(),
                        },
                        Range {
                            buf: 1,
                            start: 0,
                            len: client1.len(),
                        },
                    ]
                    .into_iter()
                    .map(Arc::new),
                ),
                cursors: vec![],
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
        let original = "acd";
        let client1 = "bef";
        let piece = Piece {
            buffers: Buffers {
                original: original.to_string().into_boxed_str(),
                clients: vec![Arc::new(AppendOnlyStr::from_str(client1).unwrap())],
            },
            piece_table: PieceTable {
                table: LinkedList::from_iter(
                    [
                        Range {
                            buf: 0,
                            start: 0,
                            len: 1,
                        },
                        Range {
                            buf: 1,
                            start: 0,
                            len: 1,
                        },
                        Range {
                            buf: 0,
                            start: 1,
                            len: 2,
                        },
                        Range {
                            buf: 1,
                            start: 1,
                            len: 2,
                        },
                    ]
                    .into_iter()
                    .map(Arc::new),
                ),
                cursors: vec![],
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
