use skiplist_rs::skiplist::Skiplist;

fn main() {
    let sl = Skiplist::<i32, i32, 6>::new();
    for i in 0..10_000 {
        println!("iteration {}", i);
        sl.insert(i, i);
    }

    let guard = &crossbeam_epoch::pin();
    for i in 0..10_000 {
        let v = sl.get(&i, guard);
        println!("{:?}", v);
    }
}
