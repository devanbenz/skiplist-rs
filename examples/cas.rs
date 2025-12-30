// use std::sync::Arc;
// use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};
// use std::thread;
//
// #[derive(Debug)]
// struct Node<K, V, const N: usize> {
//     key: K,
//     value: V,
//     height: AtomicUsize,
//     forward: [AtomicPtr<Node<K, V, N>>; N],
// }
//
// impl<K, V, const N: usize> Node<K, V, N> {
//     fn new(key: K, value: V, height: AtomicUsize) -> Self {
//         let forward: [AtomicPtr<Node<K, V, N>>; N] =
//             std::array::from_fn(|_| AtomicPtr::new(std::ptr::null_mut()));
//         Self {
//             key,
//             value,
//             height,
//             forward,
//         }
//     }
//
//     // insert_at will return false when a compare_exchange fails.
//     // It is up to the client to decide how to deal with this failure.
//     pub fn insert_at(&self, value: Node<K, V, N>, index: usize) -> bool {
//         let ptr = Box::into_raw(Box::new(value));
//         match self.forward[index].compare_exchange(
//             std::ptr::null_mut(),
//             ptr,
//             Ordering::Release,
//             Ordering::Relaxed,
//         ) {
//             Ok(_) => true,
//             Err(v) => {
//                 unsafe {
//                     drop(Box::from_raw(ptr));
//                 }
//                 false
//             }
//         }
//     }
//
//     pub fn forward(&self, index: usize) -> &AtomicPtr<Node<K, V, N>> {
//         if index > N {
//             panic!("index out of bounds");
//         }
//         &self.forward[index]
//     }
//
//     pub fn height(&self) -> usize {
//         self.height.load(Ordering::Acquire)
//     }
// }

//
// struct SkipListNoLock<T, const N: usize> {
//     slots: [AtomicPtr<T>; N],
// }
//
// impl<T, const N: usize> SkipListNoLock<T, N> {
//     pub fn new() -> Self {
//         let slots: [AtomicPtr<T>; N] =
//             std::array::from_fn(|_| AtomicPtr::new(std::ptr::null_mut()));
//         Self { slots }
//     }
//
//     // insert_at will return false when a compare_exchange fails.
//     // It is up to the client to decide how to deal with this failure.
//     pub fn insert_at(&self, value: T, index: usize) -> bool {
//         let ptr = Box::into_raw(Box::new(value));
//         match self.slots[index].compare_exchange(
//             std::ptr::null_mut(),
//             ptr,
//             Ordering::Release,
//             Ordering::Relaxed,
//         ) {
//             Ok(_) => true,
//             Err(v) => {
//                 unsafe {
//                     drop(Box::from_raw(ptr));
//                 }
//                 false
//             }
//         }
//     }
//
//     pub fn at(&self, index: usize) -> &AtomicPtr<T> {
//         if index > N {
//             panic!("index out of bounds");
//         }
//         &self.slots[index]
//     }
// }
//
// fn main() {
//     let sl = Arc::new(SkipListNoLock::<Node<u64, u64>, 5>::new());
//     let mut handles = vec![];
//
//     // 4 threads 1 to 10 count each index should be 40.
//     for thread_id in 0..4 {
//         let sl = Arc::clone(&sl);
//         let handle = thread::spawn(move || {
//             for i in 1..=10 {
//                 if sl.at(0).load(Ordering::Acquire).is_null() {
//                     if !sl.insert_at(Node { key: 0, value: i }, 0) {
//                         loop {
//                             let v = sl.at(0).load(Ordering::Acquire);
//                             let update = Box::into_raw(Box::new(Node {
//                                 key: 0u64,
//                                 value: unsafe { v.read().value.saturating_add(1) },
//                             }));
//                             match sl.at(0).compare_exchange(
//                                 v,
//                                 update,
//                                 Ordering::Release,
//                                 Ordering::Acquire,
//                             ) {
//                                 Ok(_) => {
//                                     break;
//                                 }
//                                 Err(_) => {
//                                     println!(
//                                         "thread {}: failed the race while incrementing!",
//                                         thread_id
//                                     );
//                                     unsafe { drop(Box::from_raw(update)) }
//                                 }
//                             }
//                         }
//                     }
//                 } else {
//                     loop {
//                         let v = sl.at(0).load(Ordering::Acquire);
//                         let update = Box::into_raw(Box::new(Node {
//                             key: 0u64,
//                             value: unsafe { v.read().value.saturating_add(1) },
//                         }));
//                         match sl.at(0).compare_exchange(
//                             v,
//                             update,
//                             Ordering::Release,
//                             Ordering::Acquire,
//                         ) {
//                             Ok(_) => {
//                                 break;
//                             }
//                             Err(_) => {
//                                 println!(
//                                     "thread {}: failed the race while incrementing outside the insert!",
//                                     thread_id
//                                 );
//                                 unsafe { drop(Box::from_raw(update)) }
//                             }
//                         }
//                     }
//                 }
//             }
//         });
//         handles.push(handle);
//     }
//
//     for handle in handles {
//         handle.join().unwrap();
//     }
//
//     let ptr = sl.at(0).load(Ordering::Acquire);
//     assert!(!ptr.is_null());
//     let value = unsafe { ptr.read().value };
//     assert_eq!(value, 40);
//
//     println!("Done!");
// }

fn main() {}
