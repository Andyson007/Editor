use std::{fmt::Debug, iter::Peekable};

pub struct TakeWhileRef<'a, I, P>
where
    I: Iterator,
{
    iter: &'a mut I,
    predicate: P,
}

impl<'a, I, P, T> Iterator for TakeWhileRef<'a, I, P>
where
    I: Iterator<Item = T> + Clone,
    P: FnMut(&I::Item) -> bool,
    T: Debug + 'a,
{
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.iter.clone().next().unwrap();
        if (self.predicate)(&next) {
            println!("{next:?}");
            self.iter.next()
        } else {
            None
        }
    }
}

pub trait IteratorExt: Iterator + Sized {
    fn take_while_ref<P>(&mut self, predicate: P) -> TakeWhileRef<Self, P>;
}

impl<T> IteratorExt for T
where
    T: Iterator,
{
    fn take_while_ref<P>(&mut self, predicate: P) -> TakeWhileRef<Self, P> {
        TakeWhileRef {
            iter: self,
            predicate,
        }
    }
}

#[cfg(test)]
mod test {
    use super::IteratorExt;

    #[test]
    fn take_while_ref() {
        let mut iter = 1..6;
        let sum_a = iter.take_while_ref(|x: &u8| *x < 3).sum::<u8>();
        let sum_b = iter.sum::<u8>();
        assert_eq!(sum_a, 1 + 2);
        assert_eq!(sum_b, 3 + 4 + 5);
    }
}
