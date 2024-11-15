use std::{iter::Peekable, ops::RangeInclusive};

use crate::{Buffers, Piece};

#[derive(Debug)]
pub struct Chars<T>
where
    T: Iterator<Item = (usize, RangeInclusive<usize>)>,
{
    buffers: Buffers,
    ranges: Peekable<T>,
}

impl<T> Iterator for Chars<T>
where
    T: Iterator<Item = (usize, RangeInclusive<usize>)>,
{
    type Item = char;

    fn next(&mut self) -> Option<Self::Item> {
        let (buf, curr) = self.ranges.peek_mut()?;
        if curr.is_empty() {
            self.ranges.next();
            self.next()
        } else if *buf == 0 {
            self.buffers.original.chars().nth(*curr.start())
        } else {
            self.buffers.clients.get(*buf)?.chars().nth(*curr.start())
        }
    }
}

impl Piece {}
