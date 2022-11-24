// SPDX-License-Identifier: MIT

use core::alloc::{GlobalAlloc, Layout};
use core::cell::Cell;
use core::intrinsics::unlikely;
use core::sync::atomic::{AtomicBool, Ordering};

use limine::{LimineMemmapEntry, LimineMemoryMapEntryType, NonNullPtr};

use crate::{EARLY_INIT_COMPLETE, info, mem};
use crate::mem::{MemoryManager, virtual_memory_manager, VMM};
use crate::mem::allocator::bump::BumpAllocator;
use crate::mem::allocator::linked_list::LinkedListAllocator;
use crate::mem::allocator::physical_page::PhysicalPageAllocator;
use crate::mem::vm::paging::{PAGE_SIZE, VirtualAddress};
use crate::sync::interface::{Mutex, ReadWriteEx};
use crate::sync::IRQSafeNullLock;

pub mod linked_list;
pub mod bump;

pub mod physical_page;

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
pub(crate) static GLOBAL_ALLOCATOR: IRQSafeNullLock<KernelAllocator> = IRQSafeNullLock::new(KernelAllocator::new());

pub(crate) struct KernelAllocator {
    boot_allocator: BumpAllocator,
    main_allocator: LinkedListAllocator,
    use_main_allocator: bool,
}

//--------------------------------------------------------------------------------------------------
// Private code
//--------------------------------------------------------------------------------------------------

unsafe impl GlobalAlloc for IRQSafeNullLock<KernelAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.lock(|alloc| {
            if alloc.use_main_allocator {
                // first, attempt to allocate within what the kernel already has assigned to it
                let result = alloc.main_allocator.alloc(layout);
                if !result.is_null() {
                    return result;
                }

                // if that fails, ask vmm for additional memory
                // take additional memory in pages
                let (alloc_start, size)
                    = virtual_memory_manager().kernel_alloc(layout.pad_to_align().size());

                // add the new region to the allocator
                alloc.main_allocator.add_heap_region(alloc_start, size);

                // try to allocate again
                alloc.main_allocator.alloc(layout)
            } else {
                alloc.boot_allocator.alloc(layout)
            }
        })
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // todo: in the future, can we free pages from kernel space when they are no longer needed?
        self.lock(|alloc| {
            if alloc.use_main_allocator {
                alloc.main_allocator.dealloc(ptr, layout)
            } else {
                alloc.boot_allocator.dealloc(ptr, layout)
            }
        })
    }
}

impl KernelAllocator {
    pub const fn new() -> Self {
        Self {
            boot_allocator: BumpAllocator::new(),
            main_allocator: LinkedListAllocator::new(),
            use_main_allocator: false,
        }
    }

    pub(crate) unsafe fn add_heap_region(&mut self, heap_start: VirtualAddress, heap_size: usize) {
        if unlikely(EARLY_INIT_COMPLETE.load(Ordering::Relaxed)) {
            panic!("cannot manually add heap region after kernel has booted");
        }

        self.main_allocator.add_heap_region(heap_start, heap_size);
    }

    pub(crate) unsafe fn init_boot_allocator(&mut self, start: VirtualAddress, end: VirtualAddress) {
        self.boot_allocator.init(start, end);
    }

    /// Switches to the main allocator.
    /// Returns the amount of memory that was allocated by the boot allocator.
    ///
    /// If the allocator has already been switched, this function will panic.
    pub(crate) fn use_main_allocator(&mut self) -> usize {
        assert!(!self.use_main_allocator, "allocator already switched");

        self.use_main_allocator = true;
        self.boot_allocator.get_size()
    }
}
