use std::{collections::HashMap, hash::Hash};

pub struct Trie<K, V>
where
    K: Hash,
{
    nodes: HashMap<K, TrieChild<K, V>>,
}

impl<K, V> Trie<K, V>
where
    K: Hash,
{
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert<I>(&mut self, key: I, value: V) -> Option<V>
    where
        I: IntoIterator<Item = K>,
        K: Eq,
    {
        let mut iter = key.into_iter();
        let next = iter.next()?;
        let mut curr = self.nodes.entry(next).or_default();
        for elem in iter {
            curr = curr.nodes.entry(elem).or_default();
        }
        let prev = curr.value.take();
        curr.value = Some(value);
        prev
    }

    pub fn get<I>(&mut self, key: I) -> Option<&V>
    where
        K: Eq,
        I: IntoIterator<Item = K>,
    {
        let mut iter = key.into_iter();
        let next = iter.next()?;
        let mut curr = self.nodes.get(&next)?;
        for elem in iter {
            curr = curr.nodes.get(&elem)?;
        }
        curr.value.as_ref()
    }
}

impl<K, V> Default for Trie<K, V>
where
    K: Hash,
{
    fn default() -> Self {
        Self {
            nodes: HashMap::new(),
        }
    }
}

pub struct TrieChild<K, V>
where
    K: Hash,
{
    value: Option<V>,
    nodes: HashMap<K, TrieChild<K, V>>,
}

impl<K, V> Default for TrieChild<K, V>
where
    K: Hash + Eq,
{
    fn default() -> Self {
        Self {
            value: None,
            nodes: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod test {
    use crate::Trie;

    #[test]
    fn simple_double_insert() {
        let mut trie = Trie::new();
        assert_eq!(trie.insert([1, 2, 3], ()), None);
        assert_eq!(trie.insert([1, 2, 3], ()), Some(()));
    }
}
