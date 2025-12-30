use skiplist_rs::skiplist;
use std::collections::BTreeMap;
use std::sync::atomic::AtomicUsize;
use std::sync::{Arc, RwLock};
use std::thread;

fn main() {
    divan::main();
}

#[divan::bench(consts = [1, 2, 4, 8, 16, 32])]
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

#[divan::bench(consts = [1, 2, 4, 8, 16, 32])]
fn lock_free_skiplist_mixed_workload<const THREADS: usize>(bencher: divan::Bencher) {
    bencher
        .with_inputs(|| {
            let map = Arc::new(skiplist::SkipList::<u64, u64, 5>::new());
            // Pre-populate
            {
                for i in 0..5000 {
                    map.insert(i, i * 2);
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
                            divan::black_box(map.get(&(key % 5000)));
                        } else {
                            map.insert(key + 5000, key * 2);
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
