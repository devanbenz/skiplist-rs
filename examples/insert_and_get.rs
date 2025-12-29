use skiplist_rs::sl_lock_free::SkipListNoLock;

fn main() {
    let sl = SkipListNoLock::<i32, i32, 6>::new();
    for i in 0..10_000 {
        println!("iteration {}", i);
        sl.insert(i, i);
    }

    for i in 0..10_000 {
        let v = sl.get(&i);
        println!("{:?}", v);
    }
}
