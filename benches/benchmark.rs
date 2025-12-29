use skiplist_rs::SkipList;
use skiplist_rs::array::LockFreeArray;
use skiplist_rs::sl_lock_free::SkipListNoLock;
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};
use std::thread;

fn main() {
    divan::main();
}

#[divan::bench]
fn skiplist() {
    fn compute(key: i32, value: i32) {
        let sl = SkipList::<i32, i32>::new(5);
        sl.insert(key, value);
    }

    compute(divan::black_box(10), divan::black_box(10))
}

#[divan::bench]
fn skiplist_no_lock() {
    fn compute(key: i32, value: i32) {
        let sl = SkipListNoLock::<i32, i32, 6>::new();
        sl.insert(key, value);
    }

    compute(divan::black_box(10), divan::black_box(10))
}

#[divan::bench]
fn std_btree() {
    fn compute(key: i32, value: i32) {
        let sl = Arc::new(RwLock::new(BTreeMap::<i32, i32>::new()));
        sl.write().unwrap().insert(key, value);
    }

    compute(divan::black_box(10), divan::black_box(10))
}

#[divan::bench]
fn lock_free_array() {
    fn compute(key: i32) {
        let lfa = LockFreeArray::<i32, 1000>::new();
        lfa.try_insert(key).expect("insert failed");
    }

    compute(divan::black_box(10))
}

#[divan::bench(consts = [1, 2, 4, 8])]
fn btreemap_concurrent_insert<const THREADS: usize>(bencher: divan::Bencher) {
    bencher
        .with_inputs(|| Arc::new(RwLock::new(BTreeMap::<u64, u64>::new())))
        .bench_values(|map| {
            let mut handles = vec![];
            let items_per_thread = 1000;

            for thread_id in 0..THREADS {
                let map = Arc::clone(&map);
                let handle = thread::spawn(move || {
                    let start = thread_id as u64 * items_per_thread;
                    let end = start + items_per_thread;

                    for i in start..end {
                        map.write().unwrap().insert(i, i * 2);
                    }
                });
                handles.push(handle);
            }

            for handle in handles {
                handle.join().unwrap();
            }
        });
}

#[divan::bench(consts = [1, 2, 4, 8])]
fn btreemap_concurrent_read<const THREADS: usize>(bencher: divan::Bencher) {
    bencher
        .with_inputs(|| {
            let map = Arc::new(RwLock::new(BTreeMap::<u64, u64>::new()));
            // Pre-populate with data
            {
                let mut m = map.write().unwrap();
                for i in 0..10000 {
                    m.insert(i, i * 2);
                }
            }
            map
        })
        .bench_values(|map| {
            let mut handles = vec![];
            let reads_per_thread = 1000;

            for _ in 0..THREADS {
                let map = Arc::clone(&map);
                let handle = thread::spawn(move || {
                    for i in 0..reads_per_thread {
                        let key = (i * 7) % 10000; // Pseudo-random access
                        divan::black_box(map.read().unwrap().get(&key));
                    }
                });
                handles.push(handle);
            }

            for handle in handles {
                handle.join().unwrap();
            }
        });
}

#[divan::bench(consts = [1, 2, 4, 8])]
fn btreemap_mixed_workload<const THREADS: usize>(bencher: divan::Bencher) {
    bencher
        .with_inputs(|| {
            let map = Arc::new(RwLock::new(BTreeMap::<u64, u64>::new()));
            // Pre-populate
            {
                let mut m = map.write().unwrap();
                for i in 0..5000 {
                    m.insert(i, i * 2);
                }
            }
            map
        })
        .bench_values(|map| {
            let mut handles = vec![];
            let ops_per_thread = 1000;

            for thread_id in 0..THREADS {
                let map = Arc::clone(&map);
                let handle = thread::spawn(move || {
                    for i in 0..ops_per_thread {
                        let key = thread_id as u64 * ops_per_thread + i;

                        // 70% reads, 30% writes
                        if i % 10 < 7 {
                            divan::black_box(map.read().unwrap().get(&(key % 5000)));
                        } else {
                            map.write().unwrap().insert(key + 5000, key * 2);
                        }
                    }
                });
                handles.push(handle);
            }

            for handle in handles {
                handle.join().unwrap();
            }
        });
}

// #[divan::bench(consts = [1, 2, 4, 8])]
// fn btreemap_range_scan<const THREADS: usize>(bencher: divan::Bencher) {
//     bencher
//         .with_inputs(|| {
//             let map = Arc::new(RwLock::new(BTreeMap::<u64, u64>::new()));
//             {
//                 let mut m = map.write().unwrap();
//                 for i in 0..10000 {
//                     m.insert(i, i * 2);
//                 }
//             }
//             map
//         })
//         .bench_values(|map| {
//             let mut handles = vec![];
//             let scans_per_thread = 100;
//
//             for thread_id in 0..THREADS {
//                 let map = Arc::clone(&map);
//                 let handle = thread::spawn(move || {
//                     for i in 0..scans_per_thread {
//                         let start = (thread_id as u64 * scans_per_thread + i) % 9000;
//                         let end = start + 100;
//
//                         let m = map.read().unwrap();
//                         let count: usize = m.range(start..end).count();
//                         divan::black_box(count);
//                     }
//                 });
//                 handles.push(handle);
//             }
//
//             for handle in handles {
//                 handle.join().unwrap();
//             }
//         });
// }
//
// #[divan::bench(consts = [1, 2, 4, 8])]
// fn skiplist_concurrent_insert<const THREADS: usize>(bencher: divan::Bencher) {
//     bencher
//         .with_inputs(|| Arc::new(RwLock::new(SkipList::<u64, u64>::new(16))))
//         .bench_values(|skiplist| {
//             let mut handles = vec![];
//             let items_per_thread = 1000;
//
//             for thread_id in 0..THREADS {
//                 let mut skiplist = Arc::clone(&skiplist);
//                 let handle = thread::spawn(move || {
//                     let start = thread_id as u64 * items_per_thread;
//                     let end = start + items_per_thread;
//
//                     for i in start..end {
//                         skiplist.write().expect("cannot insert").insert(i, i * 2);
//                     }
//                 });
//                 handles.push(handle);
//             }
//
//             for handle in handles {
//                 handle.join().unwrap();
//             }
//         });
// }
//
// #[divan::bench(consts = [1, 2, 4, 8])]
// fn skiplist_concurrent_read<const THREADS: usize>(bencher: divan::Bencher) {
//     bencher
//         .with_inputs(|| {
//             let skiplist = Arc::new(RwLock::new(SkipList::<u64, u64>::new(16)));
//             // Pre-populate with data
//             for i in 0..10000 {
//                 skiplist.write().expect("cannot insert").insert(i, i * 2);
//             }
//             skiplist
//         })
//         .bench_values(|skiplist| {
//             let mut handles = vec![];
//             let reads_per_thread = 1000;
//
//             for _ in 0..THREADS {
//                 let skiplist = Arc::clone(&skiplist);
//                 let handle = thread::spawn(move || {
//                     for i in 0..reads_per_thread {
//                         let key = (i * 7) % 10000; // Pseudo-random access
//                         divan::black_box(skiplist.read().expect("cannot read").get(&key));
//                     }
//                 });
//                 handles.push(handle);
//             }
//
//             for handle in handles {
//                 handle.join().unwrap();
//             }
//         });
// }
//
// #[divan::bench(consts = [1, 2, 4, 8])]
// fn skiplist_mixed_workload<const THREADS: usize>(bencher: divan::Bencher) {
//     bencher
//         .with_inputs(|| {
//             let skiplist = Arc::new(SkipList::<u64, u64>::new(16));
//             // Pre-populate
//             for i in 0..5000 {
//                 skiplist.insert(i, i * 2);
//             }
//             skiplist
//         })
//         .bench_values(|skiplist| {
//             let mut handles = vec![];
//             let ops_per_thread = 1000;
//
//             for thread_id in 0..THREADS {
//                 let skiplist = Arc::clone(&skiplist);
//                 let handle = thread::spawn(move || {
//                     for i in 0..ops_per_thread {
//                         let key = thread_id as u64 * ops_per_thread + i;
//
//                         // 70% reads, 30% writes
//                         if i % 10 < 7 {
//                             divan::black_box(skiplist.get(&(key % 5000)));
//                         } else {
//                             skiplist.insert(key + 5000, key * 2);
//                         }
//                     }
//                 });
//                 handles.push(handle);
//             }
//
//             for handle in handles {
//                 handle.join().unwrap();
//             }
//         });
// }
// #[divan::bench(consts = [1, 2, 4, 8])]
// fn skiplist_range_scan<const THREADS: usize>(bencher: divan::Bencher) {
//     bencher
//         .with_inputs(|| {
//             let skiplist = Arc::new(RwLock::new(SkipList::<u64, u64>::new(16)));
//             for i in 0..10000 {
//                 skiplist.write().expect("cannot insert").insert(i, i * 2);
//             }
//             skiplist
//         })
//         .bench_values(|skiplist| {
//             let mut handles = vec![];
//             let scans_per_thread = 100;
//
//             for thread_id in 0..THREADS {
//                 let skiplist = Arc::clone(&skiplist);
//                 let handle = thread::spawn(move || {
//                     for i in 0..scans_per_thread {
//                         let start = (thread_id as u64 * scans_per_thread + i) % 9000;
//                         let end = start + 100;
//
//                         // Adjust this if your SkipList has a different range API
//                         let count: usize = skiplist.range(start..end).count();
//                         divan::black_box(count);
//                     }
//                 });
//                 handles.push(handle);
//             }
//
//             for handle in handles {
//                 handle.join().unwrap();
//             }
//         });
// }

#[divan::bench(consts = [1, 2, 4, 8])]
fn skiplist_no_lock_concurrent_insert<const THREADS: usize>(bencher: divan::Bencher) {
    bencher
        .with_inputs(|| Arc::new(SkipListNoLock::<u64, u64, 17>::new()))
        .bench_values(|skiplist| {
            let mut handles = vec![];
            let items_per_thread = 1000;

            for thread_id in 0..THREADS {
                let skiplist = Arc::clone(&skiplist);
                let handle = thread::spawn(move || {
                    let start = thread_id as u64 * items_per_thread;
                    let end = start + items_per_thread;

                    for i in start..end {
                        skiplist.insert(i, i * 2);
                    }
                });
                handles.push(handle);
            }

            for handle in handles {
                handle.join().unwrap();
            }
        });
}

#[divan::bench(consts = [1, 2, 4, 8])]
fn skiplist_no_lock_concurrent_read<const THREADS: usize>(bencher: divan::Bencher) {
    bencher
        .with_inputs(|| {
            let skiplist = Arc::new(SkipListNoLock::<u64, u64, 17>::new());
            // Pre-populate with data
            for i in 0..10000 {
                skiplist.insert(i, i * 2);
            }
            skiplist
        })
        .bench_values(|skiplist| {
            let mut handles = vec![];
            let reads_per_thread = 1000;

            for _ in 0..THREADS {
                let skiplist = Arc::clone(&skiplist);
                let handle = thread::spawn(move || {
                    for i in 0..reads_per_thread {
                        let key = (i * 7) % 10000; // Pseudo-random access
                        divan::black_box(skiplist.get(&key));
                    }
                });
                handles.push(handle);
            }

            for handle in handles {
                handle.join().unwrap();
            }
        });
}

#[divan::bench(consts = [1, 2, 4, 8])]
fn skiplist_no_lock_mixed_workload<const THREADS: usize>(bencher: divan::Bencher) {
    bencher
        .with_inputs(|| {
            let skiplist = Arc::new(SkipListNoLock::<u64, u64, 17>::new());
            // Pre-populate
            for i in 0..5000 {
                skiplist.insert(i, i * 2);
            }
            skiplist
        })
        .bench_values(|skiplist| {
            let mut handles = vec![];
            let ops_per_thread = 1000;

            for thread_id in 0..THREADS {
                let skiplist = Arc::clone(&skiplist);
                let handle = thread::spawn(move || {
                    for i in 0..ops_per_thread {
                        let key = thread_id as u64 * ops_per_thread + i;

                        // 70% reads, 30% writes
                        if i % 10 < 7 {
                            divan::black_box(skiplist.get(&(key % 5000)));
                        } else {
                            skiplist.insert(key + 5000, key * 2);
                        }
                    }
                });
                handles.push(handle);
            }

            for handle in handles {
                handle.join().unwrap();
            }
        });
}
// #[divan::bench(consts = [1, 2, 4, 8])]
// fn skiplist_range_scan<const THREADS: usize>(bencher: divan::Bencher) {
//     bencher
//         .with_inputs(|| {
//             let skiplist = Arc::new(RwLock::new(SkipList::<u64, u64>::new(16)));
//             for i in 0..10000 {
//                 skiplist.write().expect("cannot insert").insert(i, i * 2);
//             }
//             skiplist
//         })
//         .bench_values(|skiplist| {
//             let mut handles = vec![];
//             let scans_per_thread = 100;
//
//             for thread_id in 0..THREADS {
//                 let skiplist = Arc::clone(&skiplist);
//                 let handle = thread::spawn(move || {
//                     for i in 0..scans_per_thread {
//                         let start = (thread_id as u64 * scans_per_thread + i) % 9000;
//                         let end = start + 100;
//
//                         // Adjust this if your SkipList has a different range API
//                         let count: usize = skiplist.range(start..end).count();
//                         divan::black_box(count);
//                     }
//                 });
//                 handles.push(handle);
//             }
//
//             for handle in handles {
//                 handle.join().unwrap();
//             }
//         });
// }
