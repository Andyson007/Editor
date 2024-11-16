use std::{iter, ops::RangeInclusive, str};

use append_only_str::AppendOnlyStr;

use crate::Piece;

pub struct Chars<'a, T>
where
    T: Iterator<Item = &'a (usize, RangeInclusive<usize>)>,
{
    ranges: T,
    main: &'a str,
    clients: &'a Vec<AppendOnlyStr>,
    current_iter: Option<iter::Take<iter::Skip<str::Chars<'a>>>>,
}

impl<'a, T> Iterator for Chars<'a, T>
where
    T: Iterator<Item = &'a (usize, RangeInclusive<usize>)>,
{
    type Item = char;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_iter.is_none() {
            let (buf, current_range) = self.ranges.next()?;
            self.current_iter = Some(if *buf == 0 {
                self.main
                    .chars()
                    .skip(*current_range.start())
                    .take(*current_range.end() - *current_range.start() + 1)
            } else {
                self.clients[buf - 1]
                    .chars()
                    .skip(*current_range.start())
                    .take(*current_range.end() - *current_range.start() + 1)
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
    T: Iterator<Item = &'a (usize, RangeInclusive<usize>)>,
{
    chars: Chars<'a, T>,
}

impl<'a, T> Iterator for Lines<'a, T>
where
    T: Iterator<Item = &'a (usize, RangeInclusive<usize>)>,
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
    pub fn chars(
        &self,
    ) -> Chars<'_, std::collections::linked_list::Iter<'_, (usize, RangeInclusive<usize>)>> {
        Chars {
            ranges: self.piece_table.table.iter(),
            main: &self.buffers.original,
            current_iter: None,
            clients: &self.buffers.clients,
        }
    }

    pub fn lines(
        &self,
    ) -> Lines<'_, std::collections::linked_list::Iter<'_, (usize, RangeInclusive<usize>)>> {
        Lines {
            chars: self.chars(),
        }
    }
}

#[cfg(test)]
mod test {
    use std::collections::LinkedList;

    use crate::{Buffers, Piece, PieceTable};

    #[test]
    fn test_chars_no_clients() {
        let text = "test\nmore tests\n";
        let piece = Piece {
            buffers: Buffers {
                original: text.to_string().into_boxed_str(),
                clients: vec![],
            },
            piece_table: PieceTable {
                table: LinkedList::from_iter(std::iter::once((0, 0..=text.len() - 1))),
                cursors: vec![],
            },
        };
        let mut chars = piece.chars();
        assert_eq!(chars.next(), Some('t'));
        assert_eq!(chars.next(), Some('e'));
    }

    #[test]
    fn test_lines_no_clients() {
        let text = "test\nmore tests\n";
        let piece = Piece {
            buffers: Buffers {
                original: text.to_string().into_boxed_str(),
                clients: vec![],
            },
            piece_table: PieceTable {
                table: LinkedList::from_iter(std::iter::once((0, 0..=text.len() - 1))),
                cursors: vec![],
            },
        };

        let mut lines = piece.lines();
        assert_eq!(lines.next(), Some("test".to_string()));
        assert_eq!(lines.next(), Some("more tests".to_string()));
        assert_eq!(lines.next(), None);
    }

    #[test]
    fn test_trailing_no_clients() {
        let text = "test\nmore tests\na";
        let piece = Piece {
            buffers: Buffers {
                original: text.to_string().into_boxed_str(),
                clients: vec![],
            },
            piece_table: PieceTable {
                table: LinkedList::from_iter(std::iter::once((0, 0..=text.len() - 1))),
                cursors: vec![],
            },
        };

        let mut lines = piece.lines();
        assert_eq!(lines.next(), Some("test".to_string()));
        assert_eq!(lines.next(), Some("more tests".to_string()));
        assert_eq!(lines.next(), Some("a".to_string()));
        assert_eq!(lines.next(), None);
    }
}
