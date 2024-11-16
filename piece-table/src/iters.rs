use std::{iter, ops::RangeInclusive, str};

use append_only_str::AppendOnlyStr;

use crate::{Piece, Range};

pub struct Chars<'a, T>
where
    T: Iterator<Item = &'a Range>,
{
    ranges: T,
    main: &'a str,
    clients: &'a Vec<AppendOnlyStr>,
    current_iter: Option<iter::Take<iter::Skip<str::Chars<'a>>>>,
}

impl<'a, T> Iterator for Chars<'a, T>
where
    T: Iterator<Item = &'a Range>,
{
    type Item = char;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_iter.is_none() {
            let Range { buf, start, len } = self.ranges.next()?;
            self.current_iter = Some(if *buf == 0 {
                self.main.chars().skip(*start).take(*len)
            } else {
                self.clients[buf - 1].chars().skip(*start).take(*len)
            })
        }
        if let Some(x) = self.current_iter.as_mut().unwrap().next() {
            return Some(x);
        } else {
            self.current_iter = None;
        }
        self.next()
    }
}

pub struct Lines<'a, T>
where
    T: Iterator<Item = &'a Range>,
{
    chars: Chars<'a, T>,
}

impl<'a, T> Iterator for Lines<'a, T>
where
    T: Iterator<Item = &'a Range>,
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
    pub fn chars(&self) -> Chars<'_, std::collections::linked_list::Iter<'_, Range>> {
        Chars {
            ranges: self.piece_table.table.iter(),
            main: &self.buffers.original,
            current_iter: None,
            clients: &self.buffers.clients,
        }
    }

    pub fn lines(&self) -> Lines<'_, std::collections::linked_list::Iter<'_, Range>> {
        Lines {
            chars: self.chars(),
        }
    }
}

#[cfg(test)]
mod test {
    use std::{collections::LinkedList, str::FromStr};

    use append_only_str::AppendOnlyStr;

    use crate::{iters::test, Buffers, Piece, PieceTable, Range};

    #[test]
    fn test_chars_no_clients() {
        let text = "test\nmore tests\n";
        let piece = Piece {
            buffers: Buffers {
                original: text.to_string().into_boxed_str(),
                clients: vec![],
            },
            piece_table: PieceTable {
                table: LinkedList::from_iter(std::iter::once(Range {
                    buf: 0,
                    start: 0,
                    len: text.len(),
                })),
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
                table: LinkedList::from_iter(std::iter::once(Range {
                    buf: 0,
                    start: 0,
                    len: text.len(),
                })),
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
                table: LinkedList::from_iter(std::iter::once(Range {
                    buf: 0,
                    start: 0,
                    len: text.len(),
                })),
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
                clients: vec![AppendOnlyStr::from_str(client1).unwrap()],
            },
            piece_table: PieceTable {
                table: LinkedList::from_iter([
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
                ]),
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
                clients: vec![AppendOnlyStr::from_str(client1).unwrap()],
            },
            piece_table: PieceTable {
                table: LinkedList::from_iter([
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
                ]),
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
