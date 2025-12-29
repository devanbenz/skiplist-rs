use std::sync::atomic::{AtomicPtr, Ordering};

struct SkipListNoLock<T, const N: usize> {
    slots: [AtomicPtr<T>; N],
}

impl<T, const N: usize> SkipListNoLock<T, N> {
    pub fn new() -> Self {
        let slots: [AtomicPtr<T>; N] =
            std::array::from_fn(|_| AtomicPtr::new(std::ptr::null_mut()));
        Self { slots }
    }

    pub fn cas()
}

fn main() {}
