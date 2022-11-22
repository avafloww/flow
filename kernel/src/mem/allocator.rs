// SPDX-License-Identifier: MIT

use core::alloc::{GlobalAlloc, Layout};
use core::cell::Cell;
use core::sync::atomic::{AtomicBool, Ordering};

use limine::{LimineMemmapEntry, LimineMemoryMapEntryType, NonNullPtr};

use crate::{info, mem};
use crate::mem::{MemoryManager, virtual_memory_manager, VMM};
use crate::mem::allocator::linked_list::LinkedListAllocator;
use crate::sync::interface::{Mutex, ReadWriteEx};
use crate::sync::IRQSafeNullLock;

pub mod linked_list;

//--------------------------------------------------------------------------------------------------
// Public code
//--------------------------------------------------------------------------------------------------
#[alloc_error_handler]
fn alloc_error_handler(layout: Layout) -> ! {
    panic!("kernel memory allocation failed: {:?}", layout);
}

/// Align downwards. Returns the greatest x with alignment `align`
/// so that x <= addr. The alignment must be a power of 2.
pub const fn align_down(size: usize, align: usize) -> usize {
    if align.is_power_of_two() {
        size & !(align - 1)
    } else if align == 0 {
        size
    } else {
        panic!("`align` must be a power of 2");
    }
}

/// Align the given address upwards to the given alignment.
///
/// Requires that the alignment is a power of two.
pub const fn align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}

//--------------------------------------------------------------------------------------------------
// Private definitions
//--------------------------------------------------------------------------------------------------
#[global_allocator]
static GLOBAL_ALLOCATOR: IRQSafeNullLock<KernelAllocator> = IRQSafeNullLock::new(KernelAllocator::new());

struct KernelAllocator {
    allocator: LinkedListAllocator,
}

//--------------------------------------------------------------------------------------------------
// Private code
//--------------------------------------------------------------------------------------------------

unsafe impl GlobalAlloc for IRQSafeNullLock<KernelAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.lock(|alloc| {
            // first, attempt to allocate within what the kernel already has assigned to it
            let result = alloc.allocator.alloc(layout);
            if !result.is_null() {
                return result;
            }

            // if that fails, ask vmm for additional memory
            // take additional memory in pages
            let (alloc_start, size)
                = virtual_memory_manager().kernel_alloc(layout.pad_to_align().size());

            // add the new region to the allocator
            alloc.allocator.add_heap_region(alloc_start, size);

            // try to allocate again
            alloc.allocator.alloc(layout)
        })
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // todo: in the future, can we free pages from kernel space when they are no longer needed?
        self.lock(|alloc| {
            alloc.allocator.dealloc(ptr, layout)
        })
    }
}

impl KernelAllocator {
    pub const fn new() -> Self {
        Self {
            allocator: LinkedListAllocator::new(),
        }
    }
}
