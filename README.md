## skiplist-rs

Simple rust skiplist implementation to be used with ibis (todo).

Currently, this implementation is lock free. I need to add some additional tests for race condition detection as I
expect there to be some races in skiplist.insert

Example usage

```rust
use skiplist_rs::skiplist::SkipList;

fn main() {
    // <K, V, H> where K = key type, V = value type, and H = height 
    let sl = SkipList::<i32, i32, 6>::new();
    for i in 0..10_000 {
        sl.insert(i, i);
    }

    for i in 0..10_000 {
        let v = sl.get(&i);
        println!("{:?}", v);
    }
}
```

Run benchmarks

```shell
cargo bench --package skiplist-rs
```

Performance compared to `std::collections::BtreeMap` in a multi-threaded environment

![plot](benchmark_comparison.png)
