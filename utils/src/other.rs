//! Creates handly utility structs
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