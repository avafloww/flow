// SPDX-License-Identifier: MIT

//--------------------------------------------------------------------------------------------------
// Public definitions
//--------------------------------------------------------------------------------------------------


// 0xFFFF_FFFF_8000_0000 - 0xFFFF_FFFF_FAFF_FFFF (1968MB) - kernel heap (RW)
// 0xFFFF_FFFF_FB00_0000 - 0xFFFF_FFFF_FBFF_FFFF (16MB) - kernel stack (RW)
// 0xFFFF_FFFF_FC00_0000 - 0xFFFF_FFFF_FFFF_FFFF (64MB) - kernel code (RX) + kernel .data/.bss (RW)

// Kernel allocation scheme:
// Physical page allocator (kernel)
// - Uses direct mapped physical memory + linked list allocator
// - Allocates pages to the virtual memory heap
//   - user space applications: 0x0 -> ...
//   - kernel: 0xFFFF_FFFF_8000_0000 -> 0xFFFF_FFFF_FAFF_FFFF (kernel heap)
//
// Rust global allocator
// - Uses virtual memory heap already allocated to kernel
// - Allocation error handler requests additional memory from the physical page allocator
//   - if granted, the vm alloc request is retried
//   - if not granted, the kernel panics

use core::alloc::{GlobalAlloc, Layout};
use core::cell::Cell;

use limine::{LimineHhdmRequest, LimineMemmapRequest, LimineMemoryMapEntryType};

use crate::{info, println};
use crate::mem::allocator::align_up;
use crate::mem::allocator::linked_list::LinkedListAllocator;
use crate::mem::vm::paging::PAGE_SIZE;
use crate::sync::interface::{Mutex, ReadWriteEx};
use crate::sync::IRQSafeNullLock;

pub mod allocator;
pub mod vm;

static BOOTLOADER_HHDM_INFO: LimineHhdmRequest = LimineHhdmRequest::new(0);
static BOOTLOADER_MAP_INFO: LimineMemmapRequest = LimineMemmapRequest::new(0);

static VMM: VirtualMemoryManager = VirtualMemoryManager::new();

#[inline(always)]
pub fn virtual_memory_manager() -> &'static VirtualMemoryManager {
    &VMM
}

pub struct VirtualMemoryManager {
    inner: IRQSafeNullLock<VirtualMemoryManagerInner>,
}

pub trait MemoryManager {
    /// Initialise the memory manager, switching from the bootloader-provided
    /// page tables to our own kernel-provided page tables.
    unsafe fn init(&self) -> Result<(), &'static str>;

    /// Attempts to allocate a block of memory from the kernel heap.
    /// Upon success, a tuple is returned containing the virtual address of
    /// the allocated block, as well as its size.
    /// If allocation fails, the kernel will panic.
    fn kernel_alloc(&self, size: usize) -> (usize, usize);
}

//--------------------------------------------------------------------------------------------------
// Public code
//--------------------------------------------------------------------------------------------------
/// Returns the offset virtual address to add to a physical address to get its kernel-space
/// direct mapped equivalent. This allows for additional performance during the mapping process
/// as the kernel does not need to perform a lookup in the page tables.
fn direct_map_virt_offset() -> usize {
    return BOOTLOADER_HHDM_INFO.get_response().get().unwrap().offset as usize;
}

pub(crate) fn print_physical_memory_map() {
    info!("Physical memory map provided by bootloader:");
    for entry in BOOTLOADER_MAP_INFO.get_response().get().unwrap().memmap() {
        info!(
            "  {:>8x} - {:>8x} | {:?}",
            entry.base,
            entry.base + entry.len,
            entry.typ
        );
    }
}

impl MemoryManager for VirtualMemoryManager {
    unsafe fn init(&self) -> Result<(), &'static str> {
        self.inner.lock(|inner| inner.init())
    }

    fn kernel_alloc(&self, size: usize) -> (usize, usize) {
        self.inner.lock(|inner| inner.kernel_alloc(size))
    }
}

impl VirtualMemoryManager {
    const fn new() -> VirtualMemoryManager {
        VirtualMemoryManager {
            inner: IRQSafeNullLock::new(VirtualMemoryManagerInner::new()),
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Private definitions
//--------------------------------------------------------------------------------------------------
struct VirtualMemoryManagerInner {
    physical_allocator: LinkedListAllocator,
}

//--------------------------------------------------------------------------------------------------
// Private code
//--------------------------------------------------------------------------------------------------
impl VirtualMemoryManagerInner {
    const fn new() -> Self {
        // todo: probably implement our own implementation of translation instead of using idmap/linearmap
        // Self {
        //     kernel_map: LinearMap::new(1, 1, ),
        // }
        Self {
            physical_allocator: LinkedListAllocator::new(),
        }
    }

    unsafe fn init(&mut self) -> Result<(), &'static str> {
        // 1. iterate through the bootloader-provided memory map and find usable regions
        // 2. for each usable region, track its physical address and size
        //    - each usable region is guaranteed to be at least 1 page (4KB)
        //    - usable regions are guaranteed to not overlap
        for entry in BOOTLOADER_MAP_INFO.get_response().get().unwrap().memmap() {
            match entry.typ {
                LimineMemoryMapEntryType::Usable => {
                    // use the direct map offset for now
                    self.physical_allocator.add_heap_region(
                        direct_map_virt_offset() + entry.base as usize,
                        entry.len as usize,
                    );
                }
                LimineMemoryMapEntryType::KernelAndModules => {
                    // we've found where the kernel itself is mapped
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn kernel_alloc(&mut self, size: usize) -> (usize, usize) {
        let size = align_up(size, PAGE_SIZE);
        if let Some(alloc_start) = self.physical_allocator.allocate(size, PAGE_SIZE) {
            return (alloc_start, size);
        }

        panic!("kernel_alloc: failed to allocate {} bytes to kernel heap", size);
    }
}
