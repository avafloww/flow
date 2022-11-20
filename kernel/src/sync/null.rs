// SPDX-License-Identifier: MIT

use core::cell::UnsafeCell;
use crate::sync::interface::{Mutex, ReadWriteEx};

//--------------------------------------------------------------------------------------------------
// Public definitions
//--------------------------------------------------------------------------------------------------
/// A very unsafe lock that does not actually lock anything.
///
/// # Safety
///
/// This lock is not thread safe. It is only safe to use in single-threaded environments.
pub struct NullLock<T> where T: ?Sized {
    data: UnsafeCell<T>,
}

//--------------------------------------------------------------------------------------------------
// Public code
//--------------------------------------------------------------------------------------------------
unsafe impl<T> Send for NullLock<T> where T: ?Sized + Send {}
unsafe impl<T> Sync for NullLock<T> where T: ?Sized + Send {}

impl<T> NullLock<T> {
    pub const fn new(data: T) -> Self {
        Self {
            data: UnsafeCell::new(data),
        }
    }
}

impl<T> Mutex for NullLock<T> {
    type Data = T;

    fn lock<'a, R>(&'a self, f: impl FnOnce(&'a mut Self::Data) -> R) -> R {
        // note: this is very obviously not thread safe
        // todo: implement concurrency later once we get to SMP/interrupts
        let data = unsafe { &mut *self.data.get() };

        f(data)
    }
}

impl<T> ReadWriteEx for NullLock<T> {
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
