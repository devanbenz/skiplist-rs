use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};

pub trait Min<T> {
    fn min() -> T;
}

impl Min<u64> for u64 {
    fn min() -> u64 {
        u64::MIN
    }
}

#[derive(Debug)]
pub struct Node<K, V, const N: usize> {
    key: K,
    value: Option<V>,
    height: AtomicUsize,
    forward: [AtomicPtr<Node<K, V, N>>; N],
}

impl<K, V, const N: usize> Node<K, V, N> {
    pub fn new(key: K, value: Option<V>, height: AtomicUsize) -> Self {
        let forward: [AtomicPtr<Node<K, V, N>>; N] =
            std::array::from_fn(|_| AtomicPtr::new(std::ptr::null_mut()));
        Self {
            key,
            value,
            height,
            forward,
        }
    }

    // insert_at will return false when a compare_exchange fails.
    // It is up to the client to decide how to deal with this failure.
    pub fn forward_insert(&self, value: Node<K, V, N>, index: usize) -> bool {
        let ptr = Box::into_raw(Box::new(value));
        match self.forward[index].compare_exchange(
            std::ptr::null_mut(),
            ptr,
            Ordering::Release,
            Ordering::Relaxed,
        ) {
            Ok(_) => true,
            Err(_) => {
                unsafe {
                    drop(Box::from_raw(ptr));
                }
                false
            }
        }
    }

    pub fn key(&self) -> &K {
        &self.key
    }

    pub fn value(&self) -> Option<&V> {
        self.value.as_ref()
    }

    pub fn forward(&self, index: usize) -> &AtomicPtr<Node<K, V, N>> {
        if index > N {
            panic!("index out of bounds");
        }
        &self.forward[index]
    }

    pub fn height(&self) -> usize {
        self.height.load(Ordering::Acquire)
    }
}

pub struct SkipList<K, V, const N: usize>
where
    K: Min<K>,
{
    max_height: AtomicUsize,
    cur_height: AtomicUsize,
    head: AtomicPtr<Node<K, V, N>>,
}

impl<K, V, const N: usize> SkipList<K, V, N>
where
    K: Min<K> + PartialOrd<K> + Eq + Ord,
{
    pub fn new() -> Self {
        let node_ptr: AtomicPtr<Node<K, V, N>> = AtomicPtr::new(std::ptr::null_mut());
        let new_node = Box::into_raw(Box::new(Node::<K, V, N>::new(
            <K as Min<K>>::min(),
            None,
            AtomicUsize::new(N),
        )));
        node_ptr.store(new_node, Ordering::Release);

        Self {
            max_height: AtomicUsize::new(N),
            cur_height: AtomicUsize::new(0),
            head: node_ptr,
        }
    }

    pub fn max_height(&self) -> usize {
        self.max_height.load(Ordering::Acquire)
    }

    pub fn height(&self) -> usize {
        self.cur_height.load(Ordering::Acquire)
    }

    pub fn head(&self) -> Node<K, V, N> {
        let v = self.head.load(Ordering::Acquire);
        if v.is_null() {
            panic!("head pointer must not be null");
        }
        unsafe { v.read() }
    }

    // TODO: Make this actually thread safe
    pub fn insert(&self, key: K, value: V) {
        let head_ptr = self.head.load(Ordering::Acquire);
        let mut update: [*mut Node<K, V, N>; N] = [std::ptr::null_mut(); N];
        let current_height = self.height();
        let mut curr_ptr = head_ptr;

        for i in (0..=current_height).rev() {
            loop {
                unsafe {
                    match (*curr_ptr).forward.get(i) {
                        Some(next_atomic) => {
                            let next_ptr = next_atomic.load(Ordering::Acquire);

                            if next_ptr.is_null() {
                                break;
                            }

                            if (*next_ptr).key() < &key {
                                curr_ptr = next_ptr;
                            } else {
                                break;
                            }
                        }
                        None => break,
                    }
                }
            }
            update[i] = curr_ptr;
        }

        let next = unsafe { (*head_ptr).forward(0).load(Ordering::Acquire) };
        if !next.is_null() {
            // TODO: Compare and swap instead of assignment
            unsafe {
                if (*next).key == key {
                    (*next).value = Some(value);
                    return;
                }
            };
        }

        let lvl = Self::random_level(self.max_height());
        if lvl > self.height() {
            for i in self.height() + 1..=lvl {
                update[i] = head_ptr;
            }
            loop {
                match self.cur_height.compare_exchange(
                    self.height(),
                    lvl,
                    Ordering::AcqRel,
                    Ordering::Acquire,
                ) {
                    Ok(_) => break,
                    Err(_) => {}
                }
            }
        }

        let new_node = Box::into_raw(Box::new(Node::<K, V, N>::new(
            key,
            Some(value),
            AtomicUsize::from(lvl + 1),
        )));
        for i in 0..=lvl {
            unsafe {
                let update_ptr = update[i];
                (*new_node).forward(i).store(
                    (*update_ptr).forward(i).load(Ordering::Acquire),
                    Ordering::Release,
                );
                (*update_ptr).forward(i).store(new_node, Ordering::Release);
            }
        }
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        let mut curr = self.head.load(Ordering::Acquire);
        for i in (0..=self.height()).rev() {
            if unsafe { i < (*curr).height() } {
                loop {
                    let fwd = unsafe { (*curr).forward(i).load(Ordering::Acquire) };
                    if fwd.is_null() {
                        break;
                    };

                    match unsafe { (*fwd).key.cmp(key) } {
                        std::cmp::Ordering::Less => {
                            curr = fwd;
                        }
                        std::cmp::Ordering::Equal => {
                            let val = unsafe { (*fwd).value.as_ref() };
                            return val;
                        }
                        std::cmp::Ordering::Greater => break,
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
        // Geometric distribution: 50% chance to go up each level
        while level < max_level - 1 && rng.random_bool(0.5) {
            level += 1;
        }
        level
    }
}

#[cfg(test)]
mod node_tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_node_insert_and_get() {
        let node = Node::<i32, i32, 5>::new(i32::MIN, None, AtomicUsize::new(5));
        assert!(node.value().is_none());
        assert_eq!(node.height(), 5);
        assert_eq!(node.key(), &i32::MIN);
        let ok = node.forward_insert(Node::<i32, i32, 5>::new(0, Some(1), AtomicUsize::new(1)), 0);
        assert!(ok);
        unsafe {
            assert!(!node.forward(0).load(Ordering::Acquire).is_null());
            assert_eq!(
                node.forward(0).load(Ordering::Acquire).read().value(),
                Some(&1)
            );
        }
    }

    fn update_node(sl: Arc<Node<u64, u64, 5>>) {
        loop {
            let v = sl.forward(0).load(Ordering::Acquire);
            let update = Box::into_raw(Box::new(Node::<u64, u64, 5>::new(
                0u64,
                unsafe { Option::from(v.read().value.unwrap().saturating_add(1)) },
                AtomicUsize::new(1),
            )));
            match sl
                .forward(0)
                .compare_exchange(v, update, Ordering::Release, Ordering::Acquire)
            {
                Ok(_) => {
                    break;
                }
                Err(_) => unsafe { drop(Box::from_raw(update)) },
            }
        }
    }

    #[test]
    fn test_node_thread_data_race() {
        let sl = Arc::new(Node::<u64, u64, 5>::new(
            u64::MIN,
            None,
            AtomicUsize::new(5),
        ));
        let mut handles = vec![];

        for _ in 0..4 {
            let sl = Arc::clone(&sl);
            let handle = thread::spawn(move || {
                for i in 1..=10 {
                    if sl.forward(0).load(Ordering::Acquire).is_null() {
                        if !sl.forward_insert(
                            Node::<u64, u64, 5>::new(0, Some(i), AtomicUsize::new(1)),
                            0,
                        ) {
                            update_node(Arc::clone(&sl));
                        }
                    } else {
                        update_node(Arc::clone(&sl));
                    }
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let ptr = sl.forward(0).load(Ordering::Acquire);
        assert!(!ptr.is_null());
        let value = unsafe { ptr.read().value };
        assert_eq!(value, Some(40));
    }
}

impl Min<i32> for i32 {
    fn min() -> i32 {
        i32::MIN
    }
}

#[cfg(test)]
mod skiplist_tests {
    use super::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn new_skiplist() {
        let sl = SkipList::<i32, i32, 5>::new();
        assert_eq!(sl.max_height.load(Ordering::Acquire), 5);
        assert_eq!(sl.cur_height.load(Ordering::Acquire), 0);
        assert_eq!(sl.head().key(), &i32::MIN);
        assert_eq!(sl.head().value(), None);
    }

    #[test]
    fn skiplist_insert_and_get() {
        let sl = SkipList::<i32, i32, 5>::new();
        sl.insert(0, 0);
        sl.insert(1, 1);
        sl.insert(2, 2);
        sl.insert(3, 2);
        sl.insert(4, 2);
        assert_eq!(sl.max_height.load(Ordering::Acquire), 5);
        assert_ne!(sl.cur_height.load(Ordering::Acquire), 0);
        assert_eq!(sl.head().key(), &i32::MIN);
        assert_eq!(sl.head().value(), None);

        assert_eq!(sl.get(&0), Some(&0));
        assert_eq!(sl.get(&1), Some(&1));
        assert_eq!(sl.get(&2), Some(&2));
        assert_eq!(sl.get(&3), Some(&2));
        assert_eq!(sl.get(&4), Some(&2));
    }

    fn run_data_race(iteration: usize) {
        let sl = Arc::new(SkipList::<i32, i32, 5>::new());
        let mut handles = vec![];

        for thread_id in 0..4 {
            let sl = Arc::clone(&sl);
            let handle = thread::spawn(move || {
                for i in 1..=10 {
                    if sl.get(&0).is_none() {
                        sl.insert(0, i);
                        println!(
                            "thread_id={} inserting={}, new data! Should only see this once",
                            thread_id, i
                        );
                    } else {
                        let v = sl.get(&0).unwrap();
                        println!("thread_id={} get={}", thread_id, v);
                        println!("thread_id={} inserting={}", thread_id, v + 1);
                        sl.insert(0, v + 1);
                    }
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        let val = sl.get(&0);
        assert!(val.is_some());
        assert_eq!(
            val,
            Some(&40),
            "data race detected, failed on iteration={iteration}"
        );
    }

    #[test]
    fn skiplist_data_race_test() {
        for i in 0..100 {
            run_data_race(i);
        }
    }
}
