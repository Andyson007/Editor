use std::{
    fmt::Debug,
    iter::{self, Peekable},
    mem::MaybeUninit,
};

pub struct TakeWhileRef<'a, I, P>
where
    I: Iterator,
{
    iter: &'a mut Peekable<I>,
    predicate: P,
}

impl<I, P, T> Iterator for TakeWhileRef<'_, I, P>
where
    I: Iterator<Item = T> + Clone,
    P: FnMut(&I::Item) -> bool,
{
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.iter.peek()?;
        if (self.predicate)(next) {
            self.iter.next()
        } else {
            None
        }
    }
}

fn take_while_ref<I, P>(iter: &mut Peekable<I>, predicate: P) -> TakeWhileRef<I, P>
where
    I: Iterator,
{
    TakeWhileRef { iter, predicate }
}

pub struct Chunks<I, const N: usize>
where
    I: Iterator,
    I::Item: Copy,
{
    iter: I,
}

impl<T, const N: usize> Iterator for Chunks<T, N>
where
    T: Iterator,
    T::Item: Copy,
{
    type Item = [T::Item; N];

    fn next(&mut self) -> Option<Self::Item> {
        let mut ret = [MaybeUninit::<T::Item>::uninit(); N];
        let mut ret_iter = ret.iter_mut();
        for i in 0..N {
            let (Some(ret), val) = (ret_iter.next(), self.iter.next()?) else {
                unreachable!()
            };
            {
                ret.write(val);
            }
        }
        //// # Safety
        //// We have iterated over the entirity of ret meaning that every item has been written to.
        //// we can therefore conclude that veery location has been visited
        Some(unsafe { MaybeUninit::array_assume_init(ret) })
    }
}

pub trait InnerIteratorExt<T>: Iterator + Sized
where
    T: Iterator<Item = Self::Item>,
{
    fn take_while_ref<P>(&mut self, predicate: P) -> TakeWhileRef<T, P>
    where
        P: FnMut(&Self::Item) -> bool;
}

impl<I> InnerIteratorExt<I> for Peekable<I>
where
    I: Iterator,
{
    fn take_while_ref<P>(&mut self, predicate: P) -> TakeWhileRef<I, P>
    where
        P: FnMut(&Self::Item) -> bool,
    {
        take_while_ref(self, predicate)
    }
}

pub trait IteratorExt: Iterator + Sized {
    fn chunks<const N: usize>(self) -> Chunks<Self, N>
    where
        Self::Item: Copy;
}

impl<T> IteratorExt for T
where
    T: Iterator,
{
    fn chunks<const N: usize>(self) -> Chunks<Self, N>
    where
        Self::Item: Copy,
    {
        Chunks { iter: self }
    }
}

#[cfg(test)]
mod test {
    use crate::iters::IteratorExt;

    use super::InnerIteratorExt;

    #[test]
    fn take_while_ref_test() {
        let mut iter = (1..6).peekable();

        let sum_a = iter.take_while_ref(|x: &u8| *x < 3).sum::<u8>();
        let sum_b = iter.sum::<u8>();
        assert_eq!(sum_a, 1 + 2);
        assert_eq!(sum_b, 3 + 4 + 5);
    }

    #[test]
    fn exact_size_chunk() {
        assert_ne!((0..24).chunks::<24>().next(), None);
    }
}
