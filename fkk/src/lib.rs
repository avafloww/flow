#![no_std]
extern crate alloc;

use core::ops;

/// The Flow Kernel Kit, or FKK, is a collection of libraries and utilities
/// for building components of the Flow kernel. It provides some semi-stable
/// abstractions for interfacing with high-level kernel components, such as
/// the clock and the scheduler.
pub trait FKKit {}

// todo: this is extremely temporary, need to fix when we enable SMP
/// # Safety
/// There is none lol lmao xd
#[macro_export]
macro_rules! fake_sync {
    // empty (base case for the recursion)
    () => {};

    // process multiple declarations
    ($(#[$attr:meta])* $vis:vis static $name:ident: $ty:ty = $val:expr; $($rest:tt)*) => (
        $(#[$attr])* $vis static $name: ::fkk::Syncify<$ty> = unsafe { ::fkk::Syncify::new($val) };
        ::fkk::fake_sync!($($rest)*);
    );

    // handle a single declaration
    ($(#[$attr:meta])* $vis:vis static $name:ident: $ty:ty = $val:expr) => (
        $(#[$attr])* $vis static $name: ::fkk::Syncify<$ty> = unsafe { ::fkk::Syncify::new($val) };
    );
}

// TODO: hook this up to kernel
#[macro_export]
macro_rules! println {
    () => {};
    ($($arg:tt)*) => {};
}

/// Add `Sync` to an arbitrary type. This is EXTREMELY, INCREDIBLY unsafe in anything other
/// than a single-threaded environment!
pub struct Syncify<T>(T);

impl<T> Syncify<T> {
    /// Create a new `Syncify` wrapper.
    ///
    /// # Safety
    ///
    /// This is invariant-breaking and thus unsafe.
    pub const unsafe fn new(inner: T) -> Syncify<T> {
        Syncify(inner)
    }

    pub fn with<F, R>(&'static self, f: F) -> R
    where
        F: FnOnce(&T) -> R,
    {
        f(&self.0)
    }
}

impl<T> ops::Deref for Syncify<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

unsafe impl<T> Sync for Syncify<T> {}
