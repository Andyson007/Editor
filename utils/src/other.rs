//! Creates handly utility structs

use std::ops::Add;
/// A simple wrapper around a usize which increments after each access
#[derive(Debug, Default)]
pub struct AutoIncrementing {
    counter: usize,
}

impl AutoIncrementing {
    /// Creates a new `AutoIncrementing` value starting at 0
    #[must_use]
    pub const fn new() -> Self {
        Self { counter: 0 }
    }

    /// Creates a new `AutoIncrementing` value starting at an arbitrary point
    #[must_use]
    pub const fn new_with_start(start: usize) -> Self {
        Self { counter: start }
    }

    /// Gets the underlying value and increments self
    #[must_use]
    pub fn get(&mut self) -> usize {
        let ret = self.counter;
        self.counter += 1;
        ret
    }

    /// Gets the underlying value without incrementing
    #[must_use]
    pub const fn peek(&self) -> usize {
        self.counter
    }
}

/// `CursorPos` is effectively an (x, y) tuple.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CursorPos {
    /// The row the cursor is on. This is effectively the line number
    pub row: usize,
    /// What column the cursor is on. Distance from the start of the line
    pub col: usize,
}

impl From<(usize, usize)> for CursorPos {
    fn from((col, row): (usize, usize)) -> Self {
        Self { row, col }
    }
}

impl Add<(isize, isize)> for CursorPos {
    type Output = Self;

    fn add(self, (row, col): (isize, isize)) -> Self::Output {
        Self {
            row: usize::try_from(self.row as isize + row).unwrap(),
            col: usize::try_from(self.col as isize + col).unwrap(),
        }
    }
}

#[cfg(test)]
mod test {
    use super::AutoIncrementing;

    #[test]
    fn autoincrement() {
        let mut incrementing = AutoIncrementing::new();
        assert_eq!(incrementing.get(), 0);
        assert_eq!(incrementing.get(), 1);
        assert_eq!(incrementing.get(), 2);
        assert_eq!(incrementing.get(), 3);
    }
}
