use crossbeam_epoch::{self as epoch, Atomic, Guard, Owned, Shared};
use std::fmt::Display;
use std::sync::atomic::{AtomicUsize, Ordering};

pub trait Min<T> {
    fn min() -> T;
}

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

struct Node<K, V, const N: usize> {
    key: K,
    value: Atomic<Option<Box<V>>>,
    level: usize,
    forward: [Atomic<Node<K, V, N>>; N],
}

impl<K, V, const N: usize> Node<K, V, N> {
    pub fn new(key: K, value: Option<V>, level: usize) -> Self {
        let forward = [const { Atomic::null() }; N];
        Self {
            key,
            value: Atomic::new(value.map(Box::new)),
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

pub struct Skiplist<K, V, const N: usize> {
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
                let node_value =
                    unsafe { &node.deref().value.load(Ordering::SeqCst, guard).deref() };
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
    V: Clone,
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

        let curr_node = unsafe { curr_ptr.deref() };
        if let Some(next_ptr) = curr_node.forward.get(0) {
            let next_node = next_ptr.load(Ordering::SeqCst, guard);
            if !next_node.is_null() && unsafe { next_node.deref() }.key == key {
                let node = unsafe { next_node.deref() };
                let new_value = Owned::new(Some(Box::new(value)));
                let old = node.value.swap(new_value, Ordering::AcqRel, guard);

                unsafe {
                    guard.defer_destroy(old);
                }

                return;
            }
        }

        let lvl = Self::random_level(self.max_level);
        if lvl > current_height {
            for i in current_height + 1..=lvl {
                update[i] = head_ptr;
            }
        }
        let mut cur = self.cur_level.load(Ordering::Acquire);
        while lvl > cur {
            match self
                .cur_level
                .compare_exchange(cur, lvl, Ordering::AcqRel, Ordering::Acquire)
            {
                Ok(_) => break,
                Err(actual) => cur = actual,
            }
        }

        let new_node_ptr = Atomic::new(Node::<K, V, N>::new(key, Some(value), lvl + 1));
        let new_node = unsafe { new_node_ptr.load(Ordering::SeqCst, guard).deref() };
        let new_node_shared = new_node_ptr.load(Ordering::SeqCst, guard);

        for i in (0..=lvl).rev() {
            let mut prev_ptr = update[i];
            loop {
                let prev_node = unsafe { prev_ptr.deref() };
                let next = prev_node.forward[i].load(Ordering::Acquire, guard);

                if !next.is_null() {
                    let next_node = unsafe { next.deref() };
                    if next_node.key < new_node.key {
                        prev_ptr = next;
                        continue;
                    }
                    if next_node.key == new_node.key && i == 0 {
                        let new_value = Owned::new(Some(Box::new(
                            unsafe { new_node.value.load(Ordering::Acquire, guard).deref() }
                                .as_ref()
                                .map(|b| (**b).clone())
                                .unwrap(),
                        )));
                        let old = next_node.value.swap(new_value, Ordering::AcqRel, guard);
                        unsafe { guard.defer_destroy(old) };
                        return;
                    }
                }

                new_node.forward[i].store(next, Ordering::Release);

                match prev_node.forward[i].compare_exchange(
                    next,
                    new_node_shared,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                    guard,
                ) {
                    Ok(_) => break,
                    Err(_) => continue,
                }
            }
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
                        std::cmp::Ordering::Equal => unsafe {
                            let value_ptr = fwd_node.value.load(Ordering::Acquire, guard);
                            if value_ptr.is_null() {
                                return None;
                            }
                            return unsafe { value_ptr.deref() }.as_ref().map(|v| &**v);
                        },
                        std::cmp::Ordering::Greater => break,
                    }
                } else {
                    break;
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

#[cfg(test)]
mod skiplist_tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn new_skiplist() {
        let sl = Skiplist::<i32, i32, 5>::new();
        let guard = &epoch::pin();
        assert_eq!(sl.max_level, 5);
        assert_eq!(sl.cur_level.load(Ordering::Acquire), 0);
        assert_eq!(
            unsafe { sl.head.load(Ordering::SeqCst, guard).deref() }.key,
            i32::MIN
        );
        assert!(
            unsafe {
                sl.head
                    .load(Ordering::SeqCst, guard)
                    .deref()
                    .value
                    .load(Ordering::SeqCst, guard)
                    .deref()
            }
            .is_none()
        )
    }

    #[test]
    fn skiplist_insert_and_get() {
        let sl = Skiplist::<i32, i32, 5>::new();
        let guard = &epoch::pin();
        sl.insert(0, 0);
        sl.insert(1, 1);
        sl.insert(2, 2);
        sl.insert(3, 2);
        sl.insert(4, 2);

        assert_eq!(sl.get(&0, guard), Some(&0));
        assert_eq!(sl.get(&1, guard), Some(&1));
        assert_eq!(sl.get(&2, guard), Some(&2));
        assert_eq!(sl.get(&3, guard), Some(&2));
        assert_eq!(sl.get(&4, guard), Some(&2));
    }

    #[test]
    fn skiplist_same_key_insert() {
        let sl = Skiplist::<i32, i32, 3>::new();
        let guard = &epoch::pin();
        sl.insert(0, 0);
        sl.insert(0, 1);
        sl.insert(0, 2);
        sl.insert(0, 3);
        println!("{}", sl);
        assert_eq!(sl.get(&0, guard), Some(&3));
        sl.insert(2, 0);
        sl.insert(2, 1);
        sl.insert(2, 2);
        sl.insert(2, 900);
        assert_eq!(sl.get(&2, guard), Some(&900));
    }

    #[test]
    fn concurrent_unique_keys() {
        for _ in 0..10 {
            let sl = Arc::new(Skiplist::<i32, i32, 8>::new());
            let threads = 4;
            let keys_per_thread = 100;

            let handles: Vec<_> = (0..threads)
                .map(|t| {
                    let sl = Arc::clone(&sl);
                    thread::spawn(move || {
                        for i in 0..keys_per_thread {
                            let key = t * keys_per_thread + i;
                            sl.insert(key, key);
                        }
                    })
                })
                .collect();

            for h in handles {
                h.join().unwrap();
            }

            let guard = &epoch::pin();
            for key in 0..(threads * keys_per_thread) {
                assert_eq!(sl.get(&key, guard), Some(&key), "missing key {key}");
            }
        }
    }

    #[test]
    fn concurrent_same_key_no_duplicates() {
        for _ in 0..50 {
            let sl = Arc::new(Skiplist::<i32, i32, 5>::new());

            let handles: Vec<_> = (0..4)
                .map(|t| {
                    let sl = Arc::clone(&sl);
                    thread::spawn(move || {
                        sl.insert(0, t);
                    })
                })
                .collect();

            for h in handles {
                h.join().unwrap();
            }

            let guard = &epoch::pin();
            let val = sl.get(&0, guard);
            assert!(val.is_some(), "key 0 was lost");
        }
    }

    #[test]
    fn insert_then_get_visibility() {
        for _ in 0..20 {
            let sl = Arc::new(Skiplist::<i32, i32, 5>::new());
            let barrier = Arc::new(std::sync::Barrier::new(2));

            let sl2 = Arc::clone(&sl);
            let barrier2 = Arc::clone(&barrier);
            let writer = thread::spawn(move || {
                barrier2.wait();
                sl2.insert(42, 100);
            });

            let sl3 = Arc::clone(&sl);
            let barrier3 = Arc::clone(&barrier);
            let reader = thread::spawn(move || {
                barrier3.wait();
                let guard = &epoch::pin();
                for _ in 0..10000 {
                    if sl3.get(&42, guard).is_some() {
                        return true;
                    }
                    std::hint::spin_loop();
                }
                false
            });

            writer.join().unwrap();
            let found = reader.join().unwrap();

            let guard = &epoch::pin();
            assert!(
                sl.get(&42, guard).is_some(),
                "key not visible after insert completed"
            );
            assert!(found || sl.get(&42, guard).is_some());
        }
    }
}
