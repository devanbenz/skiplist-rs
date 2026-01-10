use std::hint::spin_loop;
use std::sync::atomic::{AtomicPtr, Ordering};

const LOCK_BIT: usize = 1;

pub struct AdaptiveSwap<T> {
    inner: AtomicPtr<T>,
}

impl<T> AdaptiveSwap<T> {
    pub fn new(value: T) -> AdaptiveSwap<T> {
        AdaptiveSwap {
            inner: AtomicPtr::new(Box::into_raw(Box::new(value))),
        }
    }

    fn pack(ptr: *mut T, locked: bool) -> *mut T {
        let addr = ptr as usize;
        (if locked { addr | LOCK_BIT } else { addr }) as *mut T
    }

    fn unpack(ptr: *mut T) -> (*mut T, bool) {
        let addr = ptr as usize;
        ({ addr & !LOCK_BIT } as *mut T, { addr & LOCK_BIT != 0 })
    }

    fn swap_locked(&self, new_ptr: *mut T) -> T {
        loop {
            let current = self.inner.load(Ordering::Acquire);
            let (curr_ptr, locked) = Self::unpack(current);

            if locked {
                spin_loop();
                continue;
            }

            // Acquire lock
            let locked_curr = Self::pack(curr_ptr, true);
            if self
                .inner
                .compare_exchange_weak(current, locked_curr, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                // Have lock - do the swap
                self.inner.store(new_ptr, Ordering::Release);
                return unsafe { *Box::from_raw(curr_ptr) };
            }
        }
    }

    pub fn swap(&self, new_value: T) -> T {
        let new_ptr = Box::into_raw(Box::new(new_value));
        let mut current = self.inner.load(Ordering::Acquire);

        for _ in 0..5 {
            let (curr_ptr, locked) = Self::unpack(current);

            if locked {
                break;
            }

            match self.inner.compare_exchange_weak(
                current,
                new_ptr,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    return unsafe { *Box::from_raw(curr_ptr) };
                }
                Err(actual) => current = actual,
            }
        }

        self.swap_locked(new_ptr)
    }

    pub fn data(&self) -> *mut T {
        self.inner.load(Ordering::Acquire)
    }
}

impl<T> Drop for AdaptiveSwap<T> {
    fn drop(&mut self) {
        let inner_ptr = self.inner.load(Ordering::Acquire);
        drop(unsafe { Box::from_raw(inner_ptr) });
    }
}

#[cfg(test)]
mod adaptive_swap_test {
    use super::*;

    #[test]
    fn test_pack_unpack() {
        let a = AdaptiveSwap::new(10);
        let packed = AdaptiveSwap::pack(a.inner.load(std::sync::atomic::Ordering::Acquire), true);
        let (unpacked_ptr, locked) = AdaptiveSwap::unpack(packed);
        assert_eq!(unsafe { *unpacked_ptr }, 10);
        assert!(locked);
    }

    struct Foo {
        key: i32,
        value: i32,
        data: Vec<i32>,
    }

    #[test]
    fn test_swap() {
        let a = AdaptiveSwap::new(10);
        let old_a = a.swap(20);
        assert_eq!(old_a, 10);
        assert_eq!(unsafe { *a.inner.load(Ordering::Acquire) }, 20);
        let foo_new = Foo {
            key: 2,
            value: 4,
            data: vec![9, 10],
        };

        let b = AdaptiveSwap::new(Foo {
            key: 1,
            value: 2,
            data: vec![3, 4],
        });

        let c = vec![b.data(), b.data(), b.data(), b.data()];

        b.inner
            .store(Box::into_raw(Box::new(foo_new)), Ordering::Release);
        let a = unsafe { c[0].read() };
        assert_eq!(a.key, 1);
        let c = vec![b.data(), b.data(), b.data(), b.data()];
        println!("done");
    }

    #[test]
    fn test_swap_concurrent() {
        let a = AdaptiveSwap::new(10);
    }
}
