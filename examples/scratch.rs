use crossbeam_epoch::{self as epoch, Atomic, Shared};
use std::iter::Skip;
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
    forward: [Atomic<Node<K, V, N>>; N],
}

impl<K, V, const N: usize> Node<K, V, N> {
    pub fn new(key: K, value: Option<V>) -> Self {
        let forward = [const { Atomic::null() }; N];
        Self {
            key,
            value,
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

impl<K, V, const N: usize> Skiplist<K, V, N>
where
    K: Min<K> + std::cmp::PartialOrd,
{
    pub fn new() -> Self {
        Self {
            cur_level: AtomicUsize::new(0),
            max_level: N,
            head: Atomic::new(Node::new(K::min(), None)),
        }
    }

    pub fn insert(&self, key: K, value: V) {
        let guard = &epoch::pin();
        let head_ptr = self.head.load(Ordering::Acquire, guard);
        let mut curr_ptr = head_ptr;

        let mut update: [Shared<'_, Node<K, V, N>>; N] = [Shared::null(); N];
        let current_height = self.cur_level.load(Ordering::SeqCst);
        for i in (0..=current_height).rev() {
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
    }
}

fn main() {
    let skiplist: Skiplist<i32, i32, 3> = Skiplist::new();
    skiplist.insert(10, 10);

    println!("done!");
}
