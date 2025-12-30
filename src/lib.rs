pub mod array;
mod skiplist;
pub mod sl_lock_free;

use rand::prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

pub trait Min<T> {
    fn min() -> T;
}

pub(crate) struct Node<K, V> {
    pub(crate) height: usize,
    pub(crate) key: K,
    pub(crate) value: Option<V>,
    pub(crate) forward: Vec<Option<Arc<RwLock<Node<K, V>>>>>,
}

impl<K, V> Node<K, V>
where
    K: Send + Sync + Eq + Ord,
    V: Send + Sync,
{
    pub fn new(key: K, value: Option<V>, height: usize) -> Self {
        Self {
            key,
            value,
            height,
            forward: vec![None; height],
        }
    }
}

pub struct SkipList<K, V> {
    max_level: usize,
    cur_level: AtomicUsize,
    nodes: Arc<RwLock<Node<K, V>>>,
}

impl<K, V> SkipList<K, V>
where
    K: Eq + Ord + Min<K> + Send + Sync,
    V: Send + Sync,
{
    pub fn new(max_level: usize) -> Self {
        assert!(max_level > 0);
        let new_node: Node<K, V> = Node::new(<K as Min<K>>::min(), None, max_level + 1);
        Self {
            max_level,
            cur_level: AtomicUsize::new(0),
            nodes: Arc::new(RwLock::new(new_node)),
        }
    }

    pub fn insert(&self, key: K, value: V) {
        let mut curr = self.nodes.clone();
        let mut update: Vec<Option<Arc<RwLock<Node<K, V>>>>> = vec![None; self.max_level + 1];

        for i in (0..self.max_level).rev() {
            loop {
                let ptr = Arc::clone(&curr);
                match ptr.read().unwrap().forward[i].clone() {
                    None => break,
                    Some(val) => {
                        if val.read().unwrap().key < key {
                            curr = Arc::clone(&val);
                        } else {
                            break;
                        }
                    }
                }
            }
            update[i] = Some(Arc::clone(&curr));
        }

        if let Some(next) = &curr.read().unwrap().forward[0] {
            if next.read().unwrap().key == key {
                next.write().unwrap().value = Some(value);
                return;
            }
        }

        let lvl = Self::random_level(self.max_level);
        if lvl > self.cur_level.load(Ordering::Acquire) {
            for i in self.cur_level.load(Ordering::Acquire) + 1..=lvl {
                update[i] = Some(Arc::clone(&self.nodes));
            }
            self.cur_level.store(lvl, Ordering::Release);
        }

        let new_node = Arc::new(RwLock::new(Node::new(key, Some(value), lvl + 1)));

        for i in 0..=lvl {
            new_node.write().unwrap().forward[i] =
                update[i].clone().unwrap().read().unwrap().forward[i].clone();
            update[i].clone().unwrap().write().unwrap().forward[i] = Some(Arc::clone(&new_node));
        }
    }

    pub fn get(&self, key: &K) -> Option<V>
    where
        V: Clone,
    {
        let mut curr = Arc::clone(&self.nodes);

        for i in (0..=self.cur_level.load(Ordering::Acquire)).rev() {
            while let Some(next) = curr.clone().read().unwrap().forward[i].clone() {
                let next_guard = next.read().unwrap();

                match next_guard.key.cmp(key) {
                    std::cmp::Ordering::Less => {
                        drop(next_guard);
                        curr = next;
                    }
                    std::cmp::Ordering::Equal => {
                        return next_guard.value.clone();
                    }
                    std::cmp::Ordering::Greater => break,
                }
            }
        }

        None
    }

    fn random_level(max_level: usize) -> usize {
        let mut rng = rand::rng();
        rng.random_range(0..max_level)
    }
}

// Traits for arbitrary types
impl Min<i32> for i32 {
    fn min() -> i32 {
        i32::MIN
    }
}

impl Min<u64> for u64 {
    fn min() -> u64 {
        u64::MIN
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skiplist_test() {
        let skip_list: SkipList<i32, i32> = SkipList::new(3);
        assert_eq!(skip_list.max_level, 3);
        skip_list.insert(1, 2);
        skip_list.insert(2, 3);
        skip_list.insert(3, 4);
        assert_eq!(skip_list.get(&1), Some(2));
        assert_eq!(skip_list.get(&2), Some(3));
        assert_eq!(skip_list.get(&3), Some(4));
    }
}
