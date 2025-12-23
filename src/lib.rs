use rand::prelude::*;

trait Min<T> {
    fn min() -> T;
}

pub(crate) struct Node<K, V> {
    pub(crate) key: K,
    pub(crate) value: Option<V>,
    pub(crate) forward: Vec<Option<Node<K, V>>>,
}

impl<K, V> Clone for Node<K, V>
where
    K: Eq + Ord + Clone,
    V: Clone,
{
    fn clone(&self) -> Self {
        Self {
            key: self.key.clone(),
            value: self.value.clone(),
            forward: self.forward.iter().map(|node| node.clone()).collect(),
        }
    }
}

impl<K, V> Node<K, V>
where
    K: Eq + Ord + Clone,
    V: Clone,
{
    pub fn new(key: K, value: Option<V>, level: usize) -> Self {
        let mut forward = Vec::with_capacity(level);
        for _ in 0..level {
            let mut f = Vec::new();
            f.fill(None);
            forward.push(Some(Node {
                key: key.clone(),
                value: value.clone(),
                forward: f,
            }));
        }
        Self {
            key,
            value,
            forward,
        }
    }
}

pub struct SkipList<K, V> {
    max_level: usize,
    cur_level: usize,
    nodes: Node<K, V>,
}

impl<K, V> SkipList<K, V>
where
    K: Eq + Ord + Clone + Min<K>,
    V: Clone,
{
    pub fn new(max_level: usize) -> Self {
        assert!(max_level > 0);
        let new_node: Node<K, V> = Node::new(<K as Min<K>>::min(), None, max_level + 1);
        Self {
            max_level,
            cur_level: 0,
            nodes: new_node,
        }
    }

    pub fn insert(&mut self, key: K, value: V) {}

    pub fn get(&self, key: &K) -> Option<&V> {
        for val in self.nodes.forward {
            if val.is_some() && val.as_ref().unwrap().key == *key {
                return val.as_ref().unwrap().value.as_ref();
            }

            if val.is_some() && val.as_ref().unwrap().key < *key {}

            if val.is_none() {
                return None;
            }
        }

        None
    }

    fn random_level(&self) -> usize {
        let mut rng = rand::rng();
        rng.random_range(0..self.max_level)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    impl Min<i32> for i32 {
        fn min() -> i32 {
            i32::MIN
        }
    }

    #[test]
    fn node_test() {
        let node = Node::new(1, Some(1), 3);
        assert_eq!(node.forward.len(), 3);
    }

    #[test]
    fn skiplist_test() {
        let skip_list: SkipList<i32, i32> = SkipList::new(3);
        assert_eq!(skip_list.max_level, 3);
        assert_eq!(skip_list.cur_level, 0);
    }
}
