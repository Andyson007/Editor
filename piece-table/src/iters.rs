use core::slice;
use std::{iter, ops::RangeInclusive, str};

use append_only_str::AppendOnlyStr;

use crate::Piece;

pub struct Chars<'a, T> {
    ranges: T,
    main: &'a str,
    clients: &'a Vec<AppendOnlyStr>,
    current_iter: Option<iter::Take<iter::Skip<str::Chars<'a>>>>,
}

impl<T> Iterator for Chars<'_, T>
where
    T: Iterator<Item = (usize, RangeInclusive<usize>)>,
{
    type Item = char;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_iter.is_none() {
            let (buf, current_range) = self.ranges.next()?;
            self.current_iter = Some(if buf == 0 {
                self.main
                    .chars()
                    .skip(*current_range.start())
                    .take(*current_range.end() - *current_range.start())
            } else {
                self.clients[buf - 1]
                    .chars()
                    .skip(*current_range.start())
                    .take(*current_range.end() - *current_range.start())
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

pub struct Lines<'a, T> {
    chars: Chars<'a, T>,
}

impl<T> Iterator for Lines<'_, T>
where
    T: Iterator<Item = (usize, RangeInclusive<usize>)>,
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
            chars: Chars {
                ranges: self.piece_table.table.iter(),
                main: &self.buffers.original,
                current_iter: None,
                clients: &self.buffers.clients,
            },
        }
    }
}
