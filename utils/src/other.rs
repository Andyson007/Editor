//! Creates handly utility structs

use std::ops::{Add, AddAssign, SubAssign};
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
    fn from((row, col): (usize, usize)) -> Self {
        Self { row, col }
    }
}

impl Add<(isize, isize)> for CursorPos {
    type Output = Self;

    fn add(self, (row, col): (isize, isize)) -> Self::Output {
        Self {
            row: usize::try_from(
                isize::try_from(self.row).expect("self.row (usize) could not be cast to isize")
                    + row,
            )
            .expect("Cursorpos resulted in negative value"),
            col: usize::try_from(
                isize::try_from(self.col).expect("self.col (usize) could not be cast to isize")
                    + col,
            )
            .expect("Cursorpos resulted in negative value"),
        }
    }
}

impl AddAssign for CursorPos {
    fn add_assign(&mut self, rhs: Self) {
        self.row += rhs.row;
        self.col += rhs.col;
    }
}

impl SubAssign for CursorPos {
    fn sub_assign(&mut self, rhs: Self) {
        self.row -= rhs.row;
        self.col -= rhs.col;
    }
}

impl From<CursorPos> for (usize, usize) {
    fn from(pos: CursorPos) -> Self {
        (pos.row, pos.col)
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
