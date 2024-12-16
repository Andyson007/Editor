use std::{collections::HashMap, hash::Hash};

#[derive(Debug)]
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

    /// inserts a node in the trie.
    /// Returns the previous value if it was present
    /// returns whether the resulting node is a leaf node
    pub fn insert<I>(&mut self, key: I, value: V) -> (Option<V>, bool)
    where
        I: IntoIterator<Item = K>,
        K: Eq,
    {
        let mut iter = key.into_iter();
        let Some(next) = iter.next() else {
            return (None, false);
        };
        let mut curr = self.nodes.entry(next).or_default();
        for elem in iter {
            curr = curr.nodes.entry(elem).or_default();
        }
        let prev = curr.value.take();
        curr.value = Some(value);
        (prev, curr.nodes.is_empty())
    }

    pub fn get<I>(&self, key: I) -> Option<(&V, bool)>
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
        curr.value.as_ref().map(|x| (x, curr.nodes.is_empty()))
    }

    pub fn get_mut<I>(&mut self, key: I) -> Option<(&mut V, bool)>
    where
        K: Eq,
        I: IntoIterator<Item = K>,
    {
        let mut iter = key.into_iter();
        let next = iter.next()?;
        let mut curr = self.nodes.get_mut(&next)?;
        for elem in iter {
            curr = curr.nodes.get_mut(&elem)?;
        }
        let is_leaf = curr.nodes.is_empty();
        curr.value.as_mut().map(|x| (x, is_leaf))
    }

    /// Checks whether there is a child of this node.
    // NOTE: This function assumes that there are "null" nodes
    // null nodes being nodes which are leaves, but don't contain a value
    pub fn exists_child<I>(&self, key: I) -> bool
    where
        I: IntoIterator<Item = K>,
        K: Eq,
    {
        let mut iter = key.into_iter();
        let Some(next) = iter.next() else {
            return !self.nodes.is_empty();
        };
        let Some(mut curr) = self.nodes.get(&next) else {
            return false;
        };
        for elem in iter {
            let Some(binding) = curr.nodes.get(&elem) else {
                return false;
            };
            curr = binding;
        }
        !curr.nodes.is_empty()
    }

    /// Removes a key from the trie and returns it
    pub fn remove<I>(&mut self, key: I) -> Option<V>
    where
        I: IntoIterator<Item = K>,
        K: Eq,
    {
        let mut iter = key.into_iter();
        let next = iter.next()?;
        let ret = self.nodes.get_mut(&next)?.remove(iter);
        if self.nodes.get(&next)?.nodes.is_empty() && self.nodes.get(&next)?.value.is_none() {
            self.nodes.remove(&next);
        }
        ret
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

#[derive(Debug)]
pub struct TrieChild<K, V>
where
    K: Hash,
{
    value: Option<V>,
    nodes: HashMap<K, TrieChild<K, V>>,
}

impl<K, V> Default for TrieChild<K, V>
where
    K: Hash,
{
    fn default() -> Self {
        Self {
            value: None,
            nodes: HashMap::new(),
        }
    }
}

impl<I, K, V, const N: usize> From<[(I, V); N]> for Trie<K, V>
where
    I: IntoIterator<Item = K>,
    K: Hash + Eq,
{
    fn from(elements: [(I, V); N]) -> Self {
        let mut trie = Self::new();
        for (key, value) in elements {
            trie.insert(key, value);
        }
        trie
    }
}
impl<K, V> TrieChild<K, V>
where
    K: Hash,
{
    pub fn remove<I>(&mut self, key: I) -> Option<V>
    where
        I: IntoIterator<Item = K>,
        K: Eq,
    {
        let mut iter = key.into_iter();
        if let Some(x) = iter.next() {
            let ret = self.nodes.get_mut(&x)?.remove(iter);
            if self.nodes.get(&x)?.nodes.is_empty() && self.nodes.get(&x)?.value.is_none() {
                self.nodes.remove(&x);
            }
            ret
        } else {
            self.value.take()
        }
    }
}

#[cfg(test)]
mod test {
    use crate::Trie;

    #[test]
    fn simple_double_insert() {
        let mut trie = Trie::new();
        assert_eq!(trie.insert([1, 2, 3], ()), (None, true));
        assert_eq!(trie.insert([1, 2, 3], ()), (Some(()), true));
    }

    #[test]
    fn no_intermediate() {
        let mut trie = Trie::new();
        assert_eq!(trie.insert([1, 2, 3], ()), (None, true));
        assert_eq!(trie.get([1, 2, 3]), (Some((&(), true))));
        assert_eq!(trie.get([1, 2]), None);
    }

    #[test]
    fn leaf() {
        let mut trie = Trie::new();
        assert_eq!(trie.insert([1, 2, 3], ()), (None, true));
        assert_eq!(trie.get([1, 2, 3]), (Some((&(), true))));
        assert_eq!(trie.insert([1, 2, 3, 4], ()), (None, true));
        assert_eq!(trie.get([1, 2, 3]), (Some((&(), false))));
    }

    #[test]
    fn remove() {
        let mut trie = Trie::new();
        assert_eq!(trie.insert([1, 2, 3], ()), (None, true));
        assert_eq!(trie.insert([1, 2, 3, 4], ()), (None, true));
        assert_eq!(trie.remove([1, 2]), None);
        assert_eq!(trie.remove([1, 2, 3]), Some(()));
    }
}
