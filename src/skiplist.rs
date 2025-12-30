use std::sync::atomic;
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};

pub trait Min<T> {
    fn min() -> T;
}

#[derive(Debug)]
struct Node<K, V, const N: usize> {
    key: K,
    value: Option<V>,
    height: AtomicUsize,
    forward: [AtomicPtr<Node<K, V, N>>; N],
}

impl<K, V, const N: usize> Node<K, V, N> {
    fn new(key: K, value: Option<V>, height: AtomicUsize) -> Self {
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

struct SkipList<K, V, const N: usize>
where
    K: Min<K>,
{
    max_height: AtomicUsize,
    cur_height: AtomicUsize,
    head: AtomicPtr<Node<K, V, N>>,
}

impl<K, V, const N: usize> SkipList<K, V, N>
where
    K: Min<K>,
{
    pub fn new() -> Self {
        let node_ptr: AtomicPtr<Node<K, V, N>> = AtomicPtr::new(std::ptr::null_mut());
        let new_node = Box::into_raw(Box::new(Node::<K, V, N>::new(
            K::min(),
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

    pub fn insert(&self, key: K, value: V) {}

    pub fn get(&self, key: &K) -> Option<&V> {
        None
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

    #[test]
    fn new_skiplist() {
        let sl = SkipList::<i32, i32, 5>::new();
        assert_eq!(sl.max_height.load(Ordering::Acquire), 5);
        assert_eq!(sl.cur_height.load(Ordering::Acquire), 0);
        assert_eq!(sl.head().key(), &i32::MIN);
        assert_eq!(sl.head().value(), None);
    }
}
