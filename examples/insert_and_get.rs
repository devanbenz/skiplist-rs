use skiplist_rs::skiplist::SkipList;
use std::sync::atomic::AtomicPtr;

fn main() {
    let sl = SkipList::<i32, i32, 6>::new();
    for i in 0..10_000 {
        println!("iteration {}", i);
        let value = AtomicPtr::new(Box::into_raw(Box::new(i)));
        sl.insert_inner(i, value);
    }

    for i in 0..10_000 {
        let v = sl.get_inner(&i);
        println!("{:?}", v);
    }
}
