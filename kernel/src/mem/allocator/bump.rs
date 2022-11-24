// SPDX-License-Identifier: MIT

use crate::mem::allocator::align_up;
use crate::mem::vm::paging::VirtualAddress;
use core::alloc::{GlobalAlloc, Layout};
use core::cell::Cell;

//--------------------------------------------------------------------------------------------------
// Public definitions
//--------------------------------------------------------------------------------------------------
pub struct BumpAllocator {
    start: Cell<VirtualAddress>,
    end: Cell<VirtualAddress>,
    next: Cell<VirtualAddress>,
    allocations: Cell<usize>,
}

//--------------------------------------------------------------------------------------------------
// Public code
//--------------------------------------------------------------------------------------------------
impl BumpAllocator {
    pub const fn new() -> Self {
        Self {
            start: Cell::new(VirtualAddress(0)),
            end: Cell::new(VirtualAddress(0)),
            next: Cell::new(VirtualAddress(0)),
            allocations: Cell::new(0),
        }
    }

    pub(crate) unsafe fn init(&self, start: VirtualAddress, end: VirtualAddress) {
        assert_eq!(self.start.get().0, 0, "Bump allocator already initialised");

        self.start.set(start);
        self.end.set(end);
        self.next.set(start);
    }

    pub(crate) fn get_size(&self) -> usize {
        self.next.get().0 - self.start.get().0
    }
}

unsafe impl GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        assert_ne!(self.start.get().0, 0, "BumpAllocator not initialised");

        let alloc_start = VirtualAddress(align_up(self.next.get().0, layout.align()));
        let alloc_end = alloc_start + layout.size();

        if alloc_end >= self.end.get() {
            core::ptr::null_mut()
        } else {
            self.next.set(alloc_end);
            self.allocations.update(|x| x + 1);

            alloc_start.0 as *mut u8
        }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        assert_ne!(self.start.get().0, 0, "BumpAllocator not initialised");

        if self.allocations.update(|x| x - 1) == 0 {
            self.next.set(self.start.get());
        }
    }
}
