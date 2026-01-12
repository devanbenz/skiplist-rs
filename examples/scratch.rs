use crossbeam_epoch::{self as epoch, Atomic, Guard, Shared, unprotected};
use std::fmt::{Debug, Display};
use std::iter::Skip;
use std::ops::Deref;
use std::sync::Arc;
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};

pub trait Min<T> {
    fn min() -> T;
}

impl Min<i32> for i32 {
    fn min() -> i32 {
        i32::MIN
    }
}

struct Node<K, V, const N: usize> {
    key: K,
    value: Option<V>,
    level: usize,
    forward: [Atomic<Node<K, V, N>>; N],
}

impl<K, V, const N: usize> Node<K, V, N> {
    pub fn new(key: K, value: Option<V>, level: usize) -> Self {
        let forward = [const { Atomic::null() }; N];
        Self {
            key,
            value,
            level,
            forward,
        }
    }

    pub fn forward_ref(&mut self, index: usize) -> Option<&mut Atomic<Node<K, V, N>>> {
        if index >= N {
            panic!("index {index} is out of bounds of {N}")
        }
        self.forward.get_mut(index)
    }
}

struct Skiplist<K, V, const N: usize> {
    cur_level: AtomicUsize,
    max_level: usize,
    head: Atomic<Node<K, V, N>>,
}

impl<K, V, const N: usize> Display for Skiplist<K, V, N>
where
    K: Display,
    V: Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let guard = &epoch::pin();
        let head = self.head.load(Ordering::SeqCst, guard);
        let head_node = unsafe { head.deref() };
        for node in head_node.forward.iter() {
            let node = node.load(Ordering::SeqCst, guard);
            if !node.is_null() {
                let mut val = String::from("null");
                let node_key = unsafe { &node.deref().key };
                let node_value = unsafe { &node.deref().value };
                if let Some(v) = node_value {
                    val = format!("{}", v);
                }
                write!(f, "key={}; value={}\n", node_key, val)?;
            }
        }

        Ok(())
    }
}

impl<K, V, const N: usize> Skiplist<K, V, N>
where
    K: Min<K> + PartialOrd + Ord,
{
    pub fn new() -> Self {
        Self {
            cur_level: AtomicUsize::new(0),
            max_level: N,
            head: Atomic::new(Node::new(<K as Min<K>>::min(), None, N)),
        }
    }

    pub fn insert(&self, key: K, value: V) {
        let guard = &epoch::pin();
        let head_ptr = self.head.load(Ordering::Acquire, guard);
        let mut curr_ptr = head_ptr;

        let mut update: [Shared<'_, Node<K, V, N>>; N] = [Shared::null(); N];
        let current_height = self.cur_level.load(Ordering::SeqCst);
        for i in 0..self.max_level {
            loop {
                let curr_node = unsafe { curr_ptr.deref() };
                match curr_node.forward.get(i) {
                    Some(next_atomic) => {
                        let next_ptr = next_atomic.load(Ordering::SeqCst, guard);

                        if next_ptr.is_null() {
                            break;
                        }

                        let next_node = unsafe { next_ptr.deref() };
                        if next_node.key < key {
                            curr_ptr = next_ptr;
                        } else {
                            break;
                        }
                    }
                    None => break,
                }
            }
            update[i] = curr_ptr;
        }

        let curr_node = unsafe { curr_ptr.deref() };
        if let Some(next_ptr) = curr_node.forward.get(0) {
            let mut next_node = next_ptr.load(Ordering::SeqCst, guard);
            if !next_node.is_null() && unsafe { next_node.deref() }.key == key {
                unsafe { next_node.deref_mut() }.value = Some(value);
                return;
            }
        }

        let lvl = Self::random_level(self.max_level);
        let cur_level = self.cur_level.load(Ordering::Acquire);
        if lvl > cur_level {
            for i in cur_level + 1..=lvl {
                update[i] = head_ptr;
            }
            self.cur_level.store(lvl, Ordering::Release);
        }

        let new_node_ptr = Atomic::new(Node::<K, V, N>::new(key, Some(value), lvl + 1));
        let new_node = unsafe { new_node_ptr.load(Ordering::SeqCst, guard).deref() };
        for i in (0..=lvl).rev() {
            let prev_node = unsafe { update[i].deref() };
            let next = prev_node.forward[i].load(Ordering::Acquire, guard);

            new_node.forward[i].store(next, Ordering::Release);

            prev_node.forward[i]
                .compare_exchange(
                    next,
                    new_node_ptr.load(Ordering::Acquire, guard),
                    Ordering::AcqRel,
                    Ordering::Acquire,
                    guard,
                )
                .expect("Failed to compare node");
        }
    }

    pub fn get<'a>(&self, key: &K, guard: &'a Guard) -> Option<&'a V>
    where
        K: 'a,
    {
        let curr = self.head.load(Ordering::SeqCst, guard);
        let mut curr_node = unsafe { curr.deref() };
        let curr_level = self.cur_level.load(Ordering::SeqCst);
        for i in (0..=curr_level).rev() {
            if i < curr_level {
                loop {
                    if let Some(fwd) = curr_node.forward.get(i) {
                        let fwd_raw = fwd.load(Ordering::SeqCst, guard);
                        if fwd_raw.is_null() {
                            break;
                        }
                        let fwd_node = unsafe { fwd_raw.deref() };

                        match fwd_node.key.cmp(key) {
                            std::cmp::Ordering::Less => {
                                curr_node = fwd_node;
                            }
                            std::cmp::Ordering::Equal => {
                                if let Some(v) = &fwd_node.value {
                                    return Some(v);
                                }
                            }
                            std::cmp::Ordering::Greater => break,
                        }
                    }
                }
            }
        }
        None
    }

    fn random_level(max_level: usize) -> usize {
        use rand::Rng;
        let mut rng = rand::rng();
        let mut level = 0;
        while level < max_level - 1 && rng.random_bool(0.5) {
            level += 1;
        }
        level
    }
}

fn main() {
    let skiplist: Skiplist<i32, i32, 3> = Skiplist::new();
    skiplist.insert(1, 2);
    println!("before: {}", skiplist);
    skiplist.insert(1, 4);
    println!("after: {}", skiplist);
    skiplist.insert(2, 5);
    skiplist.insert(3, 7);
    skiplist.insert(5, 8);
    skiplist.insert(9, 12);
    skiplist.insert(0, 19);
    println!("final: {}", skiplist);
    let guard = &epoch::pin();
    let v = skiplist.get(&9, guard);
    assert!(v.is_some());
    assert_eq!(v.unwrap(), &12);
    println!("Found data: key=9; value={}", v.unwrap());
}
