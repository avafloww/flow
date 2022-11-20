// SPDX-License-Identifier: MIT

use core::alloc::{GlobalAlloc, Layout};
use core::cell::Cell;
use core::sync::atomic::{AtomicBool, Ordering};
use limine::{LimineMemmapEntry, LimineMemoryMapEntryType, NonNullPtr};
use crate::{info, mem};
use crate::mem::BOOTLOADER_HHDM_INFO;
use crate::sync::interface::{Mutex, ReadWriteEx};
use crate::sync::IRQSafeNullLock;

//--------------------------------------------------------------------------------------------------
// Public definitions
//--------------------------------------------------------------------------------------------------
#[global_allocator]
static ALLOCATOR: IRQSafeNullLock<&'static dyn GlobalAlloc> = IRQSafeNullLock::new(&BOOTSTRAP_ALLOCATOR);

//--------------------------------------------------------------------------------------------------
// Public code
//--------------------------------------------------------------------------------------------------
unsafe impl GlobalAlloc for IRQSafeNullLock<&'static dyn GlobalAlloc> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.lock(|alloc| alloc.alloc(layout))
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.lock(|alloc| alloc.dealloc(ptr, layout))
    }
}

#[alloc_error_handler]
fn alloc_error_handler(layout: Layout) -> ! {
    panic!("Memory allocation error: {:?}", layout);
}

//--------------------------------------------------------------------------------------------------
// Private definitions
//--------------------------------------------------------------------------------------------------

static BOOTSTRAP_ALLOCATOR: BootstrapAllocator = BootstrapAllocator::new();

/// A simple bump allocator to bootstrap the kernel's memory management system.
///
/// # Safety
///
/// This allocator is not thread-safe.
struct BootstrapAllocator {
    initialised: AtomicBool,
    start_address: Cell<usize>,
    end_address: Cell<usize>,
    next_address: Cell<usize>,
    alloc_count: Cell<usize>,
}

//--------------------------------------------------------------------------------------------------
// Private code
//--------------------------------------------------------------------------------------------------

/// # Safety
/// There is none. BootstrapAllocator is not thread-safe.
/// However, it will only be used in the early boot process, before other CPUs are started and
/// before the kernel's memory management system is initialised, so #yolo.
unsafe impl Sync for BootstrapAllocator {}

impl BootstrapAllocator {
    const fn new() -> Self {
        Self {
            initialised: AtomicBool::new(false),
            start_address: Cell::new(0),
            end_address: Cell::new(0),
            next_address: Cell::new(0),
            alloc_count: Cell::new(0),
        }
    }

    fn initialise(&self) {
        assert!(!self.initialised.load(Ordering::Relaxed), "Bootstrap allocator already initialised");
        let hhdm_offset = BOOTLOADER_HHDM_INFO.get_response().get().unwrap().offset as usize;

        if let Some(map_info) = mem::BOOTLOADER_MAP_INFO.get_response().get() {
            let mut best_candidate: Option<&NonNullPtr<LimineMemmapEntry>> = None;
            for entry in map_info.memmap() {
                if entry.typ == LimineMemoryMapEntryType::Usable && (best_candidate.is_none()
                    || (entry.len < best_candidate.unwrap().len && entry.len % 0x1000 == 0)) {
                    best_candidate = Some(entry);
                }
            }


            assert!(best_candidate.is_some(), "No suitable memory map entry found");

            let entry = best_candidate.unwrap();
            self.start_address.set(entry.base as usize + hhdm_offset);
            self.end_address.set(entry.base as usize + entry.len as usize + hhdm_offset);
            self.next_address.set(entry.base as usize + hhdm_offset);

            self.initialised.store(true, Ordering::Relaxed);

            info!("Initialised bootstrap allocator with range: {:16x} - {:16x}", self.start_address.get(), self.end_address.get());
        } else {
            panic!("Failed to get initial memory map from bootloader");
        }
    }
}

unsafe impl GlobalAlloc for BootstrapAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if !self.initialised.load(Ordering::Relaxed) {
            self.initialise();
        }

        let alloc_start = self.next_address.get();
        let alloc_end = alloc_start + layout.size();
        if alloc_end >= self.end_address.get() {
            return core::ptr::null_mut();
        }

        self.next_address.set(alloc_end);
        self.alloc_count.update(|x| x + 1);

        alloc_start as *mut u8
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        assert!(self.initialised.load(Ordering::Relaxed), "Attempted to deallocate before initialising BootstrapAllocator");

        if self.alloc_count.update(|x| x - 1) == 0 {
            self.next_address.set(self.start_address.get());
        }
    }
}
