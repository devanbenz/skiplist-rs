use std::ptr::null_mut;
use std::sync::Arc;
use std::sync::atomic::{AtomicPtr, Ordering};

struct Node<K, V> {
    key: K,
    value: V,
}

impl<K, V> Node<K, V> {
    fn new(key: K, value: V) -> Self {
        Self { key, value }
    }
}

fn main() {
    let v = Arc::new(AtomicPtr::<Node<i32, i32>>::new(null_mut()));
    v.store(Box::into_raw(Box::new(Node::new(10, 0))), Ordering::SeqCst);

    let mut handles = vec![];
    for _ in 0..7 {
        let r = Arc::clone(&v);
        handles.push(std::thread::spawn(move || {
            for _ in 0..100000 {
                let mut n: *mut Node<i32, i32> = null_mut();
                let old_data = r
                    .fetch_update(Ordering::Acquire, Ordering::SeqCst, |node| unsafe {
                        if !n.is_null() {
                            drop(Box::from_raw(n));
                        }
                        n = Box::into_raw(Box::new(Node::new((*node).key, (*node).value + 1)));
                        Some(n)
                    })
                    .expect("could not fetch_update");
                drop(unsafe { Box::from_raw(old_data) })
            }
        }))
    }

    for h in handles {
        h.join().unwrap();
    }

    unsafe {
        let final_ptr = v.load(Ordering::Acquire);
        println!("{:#?}", final_ptr.read().value);
        drop(Box::from_raw(final_ptr));
    }
}
