#[derive(Debug, Default)]
pub struct AutoIncrementing {
    counter: usize,
}

impl AutoIncrementing {
    pub fn new() -> Self {
        Self { counter: 0 }
    }

    pub fn new_with_start(start: usize) -> Self {
        Self { counter: start }
    }

    pub fn get(&mut self) -> usize {
        let ret = self.counter;
        self.counter += 1;
        ret
    }

    pub fn peek(&self) -> usize {
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
