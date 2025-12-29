use crate::Min;
use crate::array::LockFreeArray;
use rand::Rng;
use std::fmt::Debug;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug)]
pub(crate) struct NodeNoLock<K, V, const N: usize>
where
    K: Send + Sync,
    V: Send + Sync,
{
    pub(crate) key: K,
    pub(crate) value: Option<V>,
    pub(crate) height: usize,
    pub(crate) forward: Arc<Box<LockFreeArray<Option<Arc<Box<NodeNoLock<K, V, N>>>>, N>>>,
}

impl<K, V, const N: usize> NodeNoLock<K, V, N>
where
    K: Send + Sync + Eq + Ord,
    V: Send + Sync,
{
    pub fn new(key: K, value: Option<V>, height: usize) -> Self {
        let lfa: LockFreeArray<Option<Arc<Box<NodeNoLock<K, V, N>>>>, N> = LockFreeArray::new();
        Self {
            key,
            value,
            height,
            forward: Arc::new(Box::new(lfa)),
        }
    }
}

pub struct SkipListNoLock<K, V, const N: usize>
where
    K: Send + Sync + Eq + Ord,
    V: Send + Sync,
{
    max_level: usize,
    cur_level: AtomicUsize,
    nodes: Arc<Box<NodeNoLock<K, V, N>>>,
}

impl<K, V, const N: usize> SkipListNoLock<K, V, N>
where
    K: Debug + Eq + Ord + Min<K> + Send + Sync,
    V: Debug + Send + Sync,
{
    pub fn new() -> Self {
        assert!(N > 0);
        let new_node = NodeNoLock::<K, V, N>::new(<K as Min<K>>::min(), None, N);
        Self {
            max_level: N,
            cur_level: AtomicUsize::new(0),
            nodes: Arc::new(Box::new(new_node)),
        }
    }

    pub fn insert(&self, key: K, value: V) {
        let mut curr = self.nodes.clone();
        let update: LockFreeArray<Option<Arc<Box<NodeNoLock<K, V, N>>>>, N> = LockFreeArray::new();
        update.fill(Some(Arc::clone(&self.nodes)));

        for i in (0..self.max_level).rev() {
            if i < curr.height {
                loop {
                    let forward_ptr = curr.forward.ptr_at(i);
                    if let Some(Some(node)) = unsafe { forward_ptr.as_ref() } {
                        if node.key < key {
                            curr = Arc::clone(node);
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }
                update
                    .try_insert_at(i, Option::from(Arc::clone(&curr)))
                    .expect("Failed to insert node");
            }
        }

        let next_ref = unsafe { curr.forward.ptr_at(0).as_ref() };
        if next_ref.is_some() {
            if next_ref.unwrap().is_some() && next_ref.clone().unwrap().clone().unwrap().key == key
            {
                let new_node = Arc::new(Box::new(NodeNoLock::<K, V, N>::new(
                    key,
                    Some(value),
                    next_ref.clone().unwrap().clone().unwrap().height,
                )));

                curr.forward
                    .try_insert_at(0, Option::from(new_node))
                    .expect("Failed to insert node");
                return;
            }
        }

        let lvl = Self::random_level(self.max_level);
        if lvl > self.cur_level.load(Ordering::Acquire) {
            for i in self.cur_level.load(Ordering::Acquire) + 1..=lvl {
                update
                    .try_insert_at(i, Option::from(Arc::clone(&self.nodes)))
                    .expect("Failed to insert node");
            }
            self.cur_level.store(lvl, Ordering::Release);
        }

        let new_node = Arc::new(Box::new(NodeNoLock::<K, V, N>::new(
            key,
            Some(value),
            lvl + 1,
        )));

        for i in 0..=lvl {
            let update_node = unsafe { update.ptr_at(i).as_ref() };

            if let Some(update_node_option) = update_node {
                if let Some(update_node_arc) = update_node_option {
                    let next_ptr = update_node_arc.forward.ptr_at(i);
                    let next = if next_ptr.is_null() {
                        None
                    } else {
                        unsafe { (*next_ptr).clone() }
                    };

                    new_node
                        .forward
                        .try_insert_at(i, next)
                        .expect("Failed to insert node");

                    update_node_arc
                        .forward
                        .try_insert_at(i, Some(new_node.clone()))
                        .expect("Failed to insert node");
                }
            }
        }
    }

    pub fn get(&self, key: &K) -> Option<V>
    where
        V: Clone,
    {
        let mut curr = Arc::clone(&self.nodes);

        for i in (0..=self.cur_level.load(Ordering::Acquire)).rev() {
            if i < curr.height {
                while let Some(Some(next)) = unsafe { curr.clone().forward.ptr_at(i).as_ref() } {
                    let next_clone = Arc::clone(next);
                    match next_clone.key.cmp(key) {
                        std::cmp::Ordering::Less => {
                            curr = next_clone;
                        }
                        std::cmp::Ordering::Equal => {
                            return next_clone.value.clone();
                        }
                        std::cmp::Ordering::Greater => break,
                    }
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
