use std::{iter, str, sync::Arc};

use append_only_str::AppendOnlyStr;

use crate::{Piece, Range};

pub struct Chars<'a, T>
where
    T: Iterator<Item = Range>,
{
    ranges: T,
    main: &'a str,
    clients: &'a Vec<Arc<AppendOnlyStr>>,
    current_iter: Option<iter::Take<iter::Skip<str::Chars<'a>>>>,
}

impl<T> Iterator for Chars<'_, T>
where
    T: Iterator<Item = Range>,
{
    type Item = char;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_iter.is_none() {
            let Range { buf, start, len } = self.ranges.next()?;
            self.current_iter = Some(if buf == 0 {
                self.main.chars().skip(start).take(len)
            } else {
                self.clients[buf - 1].chars().skip(start).take(len)
            });
        }
        if let Some(x) = self.current_iter.as_mut().unwrap().next() {
            return Some(x);
        }

        self.current_iter = None;
        self.next()
    }
}

pub struct Lines<'a, T>
where
    T: Iterator<Item = Range>,
{
    chars: Chars<'a, T>,
}

impl<T> Iterator for Lines<'_, T>
where
    T: Iterator<Item = Range>,
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
    pub fn chars(&self) -> Chars<'_, std::vec::IntoIter<Range>> {
        Chars {
            ranges: self
                .piece_table
                .table
                .iter()
                .map(|x| Range::clone(x))
                .collect::<Vec<_>>()
                .into_iter(),
            main: &self.buffers.original,
            current_iter: None,
            clients: &self.buffers.clients,
        }
    }

    #[must_use]
    pub fn lines(&self) -> Lines<'_, std::vec::IntoIter<Range>> {
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
