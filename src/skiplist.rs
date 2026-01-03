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
    value: Option<AtomicPtr<V>>,
    height: AtomicUsize,
    forward: [AtomicPtr<Node<K, V, N>>; N],
}

impl<K, V, const N: usize> Node<K, V, N> {
    pub fn new(key: K, value: Option<AtomicPtr<V>>, height: AtomicUsize) -> Self {
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

    pub fn value(&self) -> Option<&AtomicPtr<V>> {
        self.value.as_ref()
    }

    pub fn set_value<F>(&self, f: F)
    where
        F: Fn(&mut V) -> V,
    {
        if let Some(value) = self.value.as_ref() {
            let new_val = value
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |v| {
                    f(unsafe { &mut *v });
                    Some(v)
                })
                .expect("could not update value");
            let ok = match value.compare_exchange(
                value.load(Ordering::Acquire),
                new_val,
                Ordering::Release,
                Ordering::Relaxed,
            ) {
                Ok(_) => {}
                Err(_) => {}
            };
        }
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
    K: Min<K> + PartialOrd<K> + Eq + Ord + Copy,
    V: Copy,
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

    pub fn insert(&self, key: K, value: V) {
        let value_ptr = Box::into_raw(Box::new(value));
        self.insert_inner(key, AtomicPtr::new(value_ptr));
    }

    // TODO: Make this actually thread safe
    pub fn insert_inner(&self, key: K, value: AtomicPtr<V>) {
        let head_ptr = self.head.load(Ordering::Acquire);
        let mut curr_ptr = head_ptr;
        let mut update: [*mut Node<K, V, N>; N] = [std::ptr::null_mut(); N];
        let current_height = self.height();

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

        let next = unsafe { (*curr_ptr).forward(0).load(Ordering::Acquire) };
        if !next.is_null() && unsafe { (*next).key() == &key } {
            unsafe {
                (*next).set_value(|mut v| {
                    v = &mut *value.load(Ordering::Acquire);
                    return *v;
                });
            }
            return;
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
                    Ordering::Release,
                    Ordering::Relaxed,
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
                if update_ptr.is_null() {
                    panic!("update pointer must not be null");
                }
                let expected = (*update_ptr).forward(i).load(Ordering::Acquire);
                (*new_node).forward(i).store(expected, Ordering::Release);
                (*update_ptr).forward(i).store(new_node, Ordering::Release)
            }
        }
    }

    pub fn get(&self, key: &K) -> Option<V> {
        if let Some(val) = self.get_inner(key) {
            unsafe { Some(val.load(Ordering::Acquire).read()) }
        } else {
            None
        }
    }

    pub fn get_inner(&self, key: &K) -> Option<&AtomicPtr<V>> {
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
                            let val = unsafe { (*fwd).value() };
                            return val;
                        }
                        std::cmp::Ordering::Greater => break,
                    }
                }
            }
        }

        None
    }

    pub fn get_node_ref(&self, key: &K) -> Option<AtomicPtr<Node<K, V, N>>> {
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
                            return Some(AtomicPtr::new(fwd));
                        }
                        std::cmp::Ordering::Greater => break,
                    }
                }
            }
        }
        None
    }

    pub fn get_and_update<F>(&self, key: &K, f: F) -> Option<AtomicPtr<V>>
    where
        F: Fn(Option<&AtomicPtr<V>>) -> AtomicPtr<V>,
    {
        let old_value = self.get_inner(key);
        let new_value = f(old_value);
        old_value.map(|old_value| {
            loop {
                match old_value.compare_exchange(
                    old_value.load(Ordering::Acquire),
                    new_value.load(Ordering::Acquire),
                    Ordering::Release,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => break,
                    Err(_) => {}
                }
            }

            new_value
        })
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
        let value = AtomicPtr::new(Box::into_raw(Box::new(1)));
        let ok = node.forward_insert(
            Node::<i32, i32, 5>::new(0, Some(value), AtomicUsize::new(1)),
            0,
        );
        assert!(ok);
        unsafe {
            assert!(!node.forward(0).load(Ordering::Acquire).is_null());
            assert_eq!(
                node.forward(0)
                    .load(Ordering::Acquire)
                    .read()
                    .value()
                    .unwrap()
                    .load(Ordering::Relaxed)
                    .read(),
                1
            );
        }
    }

    fn add_one(i: &mut u64) -> u64 {
        i.saturating_add(1)
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
                for _ in 1..=10 {
                    if sl.forward(0).load(Ordering::Acquire).is_null() {
                        if !sl.forward_insert(
                            Node::<u64, u64, 5>::new(
                                u64::MIN,
                                Some(AtomicPtr::new(Box::into_raw(Box::new(0)))),
                                AtomicUsize::new(1),
                            ),
                            0,
                        ) {
                            unsafe {
                                (*sl.forward(0).load(Ordering::Acquire)).set_value(add_one);
                            }
                        }
                    } else {
                        unsafe {
                            (*sl.forward(0).load(Ordering::Acquire)).set_value(add_one);
                        }
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
        let value = unsafe { ptr.read().value.unwrap().load(Ordering::Relaxed).read() };
        assert_eq!(value, 39);
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
        assert!(sl.head().value().is_none());
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
        assert!(sl.head().value().is_none());

        assert_eq!(sl.get(&0), Some(0));
        assert_eq!(sl.get(&1), Some(1));
        assert_eq!(sl.get(&2), Some(2));
        assert_eq!(sl.get(&3), Some(2));
        assert_eq!(sl.get(&4), Some(2));
    }

    fn run_data_race(iteration: usize) {
        let sl = Arc::new(SkipList::<i32, i32, 5>::new());
        let mut handles = vec![];

        for thread_id in 0..4 {
            let sl = Arc::clone(&sl);
            let handle = thread::spawn(move || {
                for i in 1..=10 {
                    if sl.get_inner(&0).is_none() {
                        sl.insert(0, i);
                    } else {
                        let v = sl.get(&0).unwrap();
                        sl.insert(0, (v + 1));
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
            Some(40),
            "data race detected, failed on iteration={iteration}"
        );
    }

    #[test]
    fn skiplist_data_race_test() {
        for i in 0..100 {
            run_data_race(i);
        }
    }

    #[test]
    fn skiplist_node_refs() {
        let sl = Arc::new(SkipList::<i32, i32, 2>::new());
        sl.insert(10, 2);

        let v = sl.get_node_ref(&10);
        assert!(v.is_some());
        let node = v.unwrap();
        let ret = unsafe { node.load(Ordering::Acquire).as_mut() }
            .unwrap()
            .set_value(|mut v| {
                let _ = std::mem::replace(v, 9999);
                return *v;
            });
        assert_eq!(sl.get(&10), Some(9999));
    }

    #[test]
    fn skiplist_same_key_insert() {
        let sl = SkipList::<i32, i32, 3>::new();
        sl.insert(0, 0);
        sl.insert(0, 1);
        sl.insert(0, 2);
        sl.insert(0, 3);
        assert_eq!(sl.get(&0), Some(3));
        sl.insert(2, 0);
        sl.insert(2, 1);
        sl.insert(2, 2);
        sl.insert(2, 900);
        assert_eq!(sl.get(&2), Some(900));
    }
}
