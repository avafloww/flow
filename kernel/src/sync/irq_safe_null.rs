// SPDX-License-Identifier: MIT
use core::cell::UnsafeCell;

use crate::exception;
use crate::sync::interface::{Mutex, ReadWriteEx};

pub struct IRQSafeNullLock<T>
where
    T: ?Sized,
{
    data: UnsafeCell<T>,
}

unsafe impl<T> Send for IRQSafeNullLock<T> where T: ?Sized {}
unsafe impl<T> Sync for IRQSafeNullLock<T> where T: ?Sized {}

impl<T> IRQSafeNullLock<T> {
    pub const fn new(data: T) -> Self {
        Self {
            data: UnsafeCell::new(data),
        }
    }
}

impl<T> Mutex for IRQSafeNullLock<T> {
    type Data = T;

    fn lock<'a, R>(&'a self, f: impl FnOnce(&'a mut Self::Data) -> R) -> R {
        // note: this is very obviously not thread safe
        // todo: implement concurrency later once we get to SMP/interrupts
        let data = unsafe { &mut *self.data.get() };

        exception::asynchronous::exec_with_masked_irqs(|| f(data))
    }
}

impl<T> ReadWriteEx for IRQSafeNullLock<T> {
    type Data = T;

    fn write<'a, R>(&'a self, f: impl FnOnce(&'a mut Self::Data) -> R) -> R {
        let data = unsafe { &mut *self.data.get() };
        f(data)
    }

    fn read<'a, R>(&'a self, f: impl FnOnce(&'a Self::Data) -> R) -> R {
        let data = unsafe { &mut *self.data.get() };
        f(data)
    }
}
