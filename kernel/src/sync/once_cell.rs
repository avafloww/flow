// SPDX-License-Identifier: MIT
use core::cell::UnsafeCell;
use core::ops::Deref;

//--------------------------------------------------------------------------------------------------
// Public definitions
//--------------------------------------------------------------------------------------------------
pub struct OnceCell<T> {
    data: UnsafeCell<Option<T>>,
}

//--------------------------------------------------------------------------------------------------
// Public code
//--------------------------------------------------------------------------------------------------
unsafe impl<T> Send for OnceCell<T> where T: Send {}
unsafe impl<T> Sync for OnceCell<T> where T: Send {}

impl<T> OnceCell<T> {
    pub const fn new() -> Self {
        Self {
            data: UnsafeCell::new(None),
        }
    }

    pub fn set(&self, value: T) {
        let data = unsafe { &mut *self.data.get() };
        assert!(data.is_none(), "OnceCell already initialized");
        *data = Some(value);
    }

    pub fn get(&self) -> Option<&T> {
        let data = unsafe { &*self.data.get() };
        data.as_ref()
    }
}

impl<T> Deref for OnceCell<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.get().unwrap_or_else(|| panic!("OnceCell not initialized"))
    }
}
