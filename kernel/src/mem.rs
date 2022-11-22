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
use core::cell::{Cell, UnsafeCell};
use aarch64_cpu::registers::TCR_EL1;

use limine::{LimineHhdmRequest, LimineMemmapRequest, LimineMemoryMapEntryType};
use tock_registers::interfaces::Writeable;

use crate::{info, println};
use crate::mem::allocator::align_up;
use crate::mem::allocator::linked_list::LinkedListAllocator;
use crate::mem::vm::paging::{Attributes, VirtualMemoryRegion, PAGE_SIZE, PhysicalAddress, VaRange, RootPageTable};
use crate::sync::interface::{Mutex, ReadWriteEx};
use crate::sync::{IRQSafeNullLock, OnceCell};
use crate::util::size_human_readable_ceil;

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
    /// If this operation fails, the kernel will panic.
    unsafe fn init(&self);

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
#[inline(always)]
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

    info!("Higher half direct map address: {:#x}", direct_map_virt_offset());
}

impl MemoryManager for VirtualMemoryManager {
    unsafe fn init(&self) {
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
// Symbols from the linker script, and functions to ease their retrieval.
extern "Rust" {
    static __kernel_binary_start: UnsafeCell<()>;
    static __kernel_binary_end: UnsafeCell<()>;
    static __kernel_code_start: UnsafeCell<()>;
    static __kernel_code_end: UnsafeCell<()>;
    static __kernel_data_start: UnsafeCell<()>;
    static __kernel_data_end: UnsafeCell<()>;
    static __kernel_stack_start: UnsafeCell<()>;
    static __kernel_stack_end: UnsafeCell<()>;
    static __kernel_heap_start: UnsafeCell<()>;
    static __kernel_heap_end: UnsafeCell<()>;
}

#[inline(always)]
fn kernel_binary_start() -> usize {
    unsafe { __kernel_binary_start.get() as usize }
}

#[inline(always)]
fn kernel_binary_end() -> usize {
    unsafe { __kernel_binary_end.get() as usize }
}

#[inline(always)]
fn kernel_code_start() -> usize {
    unsafe { __kernel_code_start.get() as usize }
}

#[inline(always)]
fn kernel_code_end() -> usize {
    unsafe { __kernel_code_end.get() as usize }
}

#[inline(always)]
fn kernel_data_start() -> usize {
    unsafe { __kernel_data_start.get() as usize }
}

#[inline(always)]
fn kernel_data_end() -> usize {
    unsafe { __kernel_data_end.get() as usize }
}

#[inline(always)]
fn kernel_stack_start() -> usize {
    unsafe { __kernel_stack_start.get() as usize }
}

#[inline(always)]
fn kernel_stack_end() -> usize {
    unsafe { __kernel_stack_end.get() as usize }
}

#[inline(always)]
fn kernel_heap_start() -> usize {
    unsafe { __kernel_heap_start.get() as usize }
}

#[inline(always)]
fn kernel_heap_end() -> usize {
    unsafe { __kernel_heap_end.get() as usize }
}

struct VirtualMemoryManagerInner {
    physical_allocator: LinkedListAllocator,
    kernel_page_table: OnceCell<IRQSafeNullLock<RootPageTable>>,
}

//--------------------------------------------------------------------------------------------------
// Private code
//--------------------------------------------------------------------------------------------------
struct MemoryMapResult {
    highest_physical_address: PhysicalAddress,
    kernel_physical_address: PhysicalAddress,
}

impl VirtualMemoryManagerInner {
    const fn new() -> Self {
        Self {
            physical_allocator: LinkedListAllocator::new(),
            // we can't allocate the page table yet, so we use OnceCell here
            kernel_page_table: OnceCell::new(),
        }
    }

    unsafe fn init(&mut self) {
        let highest_physical_address = self.init_memory_map();
        self.init_kernel_paging(highest_physical_address);
    }

    /// Initialises the kernel's memory map by parsing the memory map provided by the bootloader.
    /// The kernel's memory map is then used to initialise the physical page allocator.
    ///
    /// Returns the highest (likely final) physical address in the memory map.
    unsafe fn init_memory_map(&mut self) -> MemoryMapResult {
        // 1. iterate through the bootloader-provided memory map and find usable regions
        // 2. for each usable region, track its physical address and size
        //    - each usable region is guaranteed to be at least 1 page (4KB)
        //    - usable regions are guaranteed to not overlap
        let mut result = MemoryMapResult {
            highest_physical_address: PhysicalAddress(0),
            kernel_physical_address: PhysicalAddress(0),
        };

        for entry in BOOTLOADER_MAP_INFO.get_response().get().unwrap().memmap() {
            // entries are guaranteed to be sorted by physical address, lowest to highest
            result.highest_physical_address = PhysicalAddress((entry.base + entry.len) as usize);

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
                    result.kernel_physical_address = PhysicalAddress(entry.base as usize);
                }
                _ => {}
            }
        }

        return result;
    }

    fn with_kernel_page_table<'a>(&'a self, f: impl FnOnce(&'a mut RootPageTable)) {
        self.kernel_page_table.get().unwrap().lock(f);
    }

    /// Initialises the kernel's page tables and switches the MMU to use them.
    /// This function also attempts to map all of physical memory at the same higher-half direct
    /// mapped virtual address that the bootloader set up for us.
    ///
    /// Limine's typical higher-half direct map address is 0xFFFF_8000_0000_0000.
    /// If the start of the kernel heap is at 0xFFFF_FFFF_8000_0000, this means our current
    /// memory management implementation can tolerate up to 0x7FFF_8000_0000 bytes, or ~128TB,
    /// of physical memory. I don't think we'll be seeing anywhere close to those numbers on any
    /// system running Flow, but we do a sanity check and panic if we exceed this limit anyways :)
    unsafe fn init_kernel_paging(&mut self, memory_map_result: MemoryMapResult) {
        let max_phys_mem = kernel_binary_start() - direct_map_virt_offset();
        if memory_map_result.highest_physical_address.0 > max_phys_mem {
            let (size, unit) = size_human_readable_ceil(max_phys_mem);
            panic!(
                "this system has too much addressable memory; only systems with less than {} {} are supported",
                size, unit
            );
        }

        // create a new page table
        self.kernel_page_table.set(IRQSafeNullLock::new(RootPageTable::new(0, VaRange::Upper)));
        self.with_kernel_page_table(|kernel_table| {
            // direct map all of physical memory (RW)
            let dm_offset = direct_map_virt_offset();
            kernel_table.map_range(
                &VirtualMemoryRegion::new(dm_offset, dm_offset + memory_map_result.highest_physical_address.0),
                PhysicalAddress(0),
                Attributes::DEVICE_NGNRNE | Attributes::EXECUTE_NEVER
            ).unwrap();

            // map the kernel code (RX)
            kernel_table.map_range(
                &VirtualMemoryRegion::new(kernel_code_start(), kernel_code_end()),
                memory_map_result.kernel_physical_address,
                Attributes::NORMAL | Attributes::READ_ONLY
            ).unwrap();

            // map the kernel data (RW)
            kernel_table.map_range(
                &VirtualMemoryRegion::new(kernel_data_start(), kernel_data_end()),
                memory_map_result.kernel_physical_address + (kernel_data_start() - kernel_binary_start()),
                Attributes::NORMAL | Attributes::EXECUTE_NEVER
            ).unwrap();

            // configure TCR_EL1
            TCR_EL1.write(
                TCR_EL1::TBI0::Used
                    + TCR_EL1::IPS::Bits_48
                    + TCR_EL1::TG1::KiB_4
                    + TCR_EL1::SH1::Outer
                    + TCR_EL1::ORGN1::WriteBack_ReadAlloc_WriteAlloc_Cacheable
                    + TCR_EL1::IRGN1::WriteBack_ReadAlloc_WriteAlloc_Cacheable
                    + TCR_EL1::EPD1::EnableTTBR1Walks
                    + TCR_EL1::A1::TTBR0
                    + TCR_EL1::T1SZ.val(16)
                    + TCR_EL1::SH1::Outer
                    + TCR_EL1::ORGN0::WriteBack_ReadAlloc_WriteAlloc_Cacheable
                    + TCR_EL1::IRGN0::WriteBack_ReadAlloc_WriteAlloc_Cacheable
                    + TCR_EL1::EPD0::EnableTTBR0Walks
                    + TCR_EL1::T0SZ.val(16)
            );

            // activate the new page table
            kernel_table.activate();
        })
    }

    fn kernel_alloc(&mut self, size: usize) -> (usize, usize) {
        let size = align_up(size, PAGE_SIZE);
        if let Some(alloc_start) = self.physical_allocator.allocate(size, PAGE_SIZE) {
            return (alloc_start, size);
        }

        panic!("kernel_alloc: failed to allocate {} bytes to kernel heap", size);
    }
}
