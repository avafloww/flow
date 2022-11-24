// SPDX-License-Identifier: MIT
use core::cell::UnsafeCell;

use crate::sync::interface::ReadWriteEx;
use crate::{exception, EARLY_INIT_COMPLETE};

pub struct InitStateLock<T>
where
    T: ?Sized,
{
    data: UnsafeCell<T>,
}

unsafe impl<T> Send for InitStateLock<T> where T: ?Sized + Send {}
unsafe impl<T> Sync for InitStateLock<T> where T: ?Sized + Send {}

impl<T> InitStateLock<T> {
    pub const fn new(data: T) -> Self {
        Self {
            data: UnsafeCell::new(data),
        }
    }
}

impl<T> ReadWriteEx for InitStateLock<T> {
    type Data = T;

    fn write<'a, R>(&'a self, f: impl FnOnce(&'a mut Self::Data) -> R) -> R {
        assert!(
            !EARLY_INIT_COMPLETE.load(core::sync::atomic::Ordering::Relaxed),
            "Attempted to write to init state lock after early init complete"
        );

        assert!(
            !exception::asynchronous::is_local_irq_masked(),
            "cannot write to InitStateLock while interrupts are unmasked"
        );

        let data = unsafe { &mut *self.data.get() };
        f(data)
    }

    fn read<'a, R>(&'a self, f: impl FnOnce(&'a Self::Data) -> R) -> R {
        let data = unsafe { &*self.data.get() };
        f(data)
    }
}
