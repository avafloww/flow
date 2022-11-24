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

use alloc::rc::Rc;
use alloc::vec::Vec;
use core::alloc::{GlobalAlloc, Layout};
use core::arch::asm;
use core::cell::{Cell, UnsafeCell};
use core::intrinsics::unlikely;
use core::mem;
use aarch64_cpu::registers::TCR_EL1;

use limine::{LimineHhdmRequest, LimineMemmapRequest, LimineMemoryMapEntryType};
use tock_registers::interfaces::Writeable;

use crate::{info, println};
use crate::mem::allocator::{align_down, align_up};
use crate::mem::allocator::linked_list::{LinkedListAllocator, LIST_NODE_SIZE};
use crate::mem::allocator::physical_page::PhysicalPageAllocator;
use crate::mem::vm::paging::{Attributes, VirtualMemoryRegion, PAGE_SIZE, PhysicalAddress, VaRange, RootPageTable, VirtualAddress};
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
    ///
    /// Takes the base address of the kernel's stack, which is used to relocate the
    /// stack to a new location.
    unsafe fn init(&self, base_sp: usize);

    /// Attempts to allocate a block of memory from the kernel heap.
    /// Upon success, a tuple is returned containing the virtual address of
    /// the allocated block, as well as its size.
    /// If allocation fails, the kernel will panic.
    fn kernel_alloc(&self, size: usize) -> (VirtualAddress, usize);
}

//--------------------------------------------------------------------------------------------------
// Public code
//--------------------------------------------------------------------------------------------------
/// Returns the offset virtual address to add to a physical address to get its kernel-space
/// direct mapped equivalent. This allows for additional performance during the mapping process
/// as the kernel does not need to perform a lookup in the page tables.
#[inline(always)]
pub(crate) fn direct_map_virt_offset() -> usize {
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
    unsafe fn init(&self, base_sp: usize) {
        self.inner.lock(|inner| inner.init(base_sp))
    }

    fn kernel_alloc(&self, size: usize) -> (VirtualAddress, usize) {
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
    physical_allocator: PhysicalPageAllocator,
    kernel_page_table: OnceCell<IRQSafeNullLock<RootPageTable>>,
    use_kernel_heap_addresses: bool,
}

//--------------------------------------------------------------------------------------------------
// Private code
//--------------------------------------------------------------------------------------------------
#[derive(Clone, Copy)]
struct MemoryMapResult {
    highest_physical_address: PhysicalAddress,
    kernel_physical_address: PhysicalAddress,
}

impl VirtualMemoryManagerInner {
    const fn new() -> Self {
        Self {
            physical_allocator: PhysicalPageAllocator::new(),
            // we can't allocate the page table yet, so we use OnceCell here
            kernel_page_table: OnceCell::new(),
            use_kernel_heap_addresses: false,
        }
    }

    unsafe fn init(&mut self, base_sp: usize) {
        // 1. Initialise the physical memory allocator with the Limine memory map
        let memory_map = self.init_memory_map();

        // 2. Manually allocate a bit of memory to bootstrap the kernel page tables
        // Note: as of 23/Nov/2022, we needed just over 28KB of memory here.
        // We'll allocate 64KB to allow for the second stage bootstrapping.
        const INITIAL_ALLOC_SIZE: usize = 64 * 1024;
        let (alloc_start, alloc_size) = self.kernel_alloc_unchecked(INITIAL_ALLOC_SIZE);

        // Now, make the Rust global allocator aware of the memory we just allocated
        allocator::GLOBAL_ALLOCATOR.lock(|alloc| {
            let alloc_start_virt: VirtualAddress = alloc_start.into();
            let alloc_end_virt = VirtualAddress(alloc_start_virt.0 + alloc_size);

            alloc.init_boot_allocator(alloc_start_virt, alloc_end_virt);
        });

        // 2. Initialise the initial kernel page table to ensure that heap/stack are mapped
        let bootstrap_table = Rc::new(self.bootstrap_kernel_page_table(memory_map, alloc_start, alloc_size));

        // make sure the page tables do not go out of scope - otherwise they will be dropped,
        // causing the ttbr1 to fallback to the bootloader provided tables, which are mapped
        // to the lower half (and as such, will not work properly after our mapping)

        // 3. Move the stack to the new kernel stack location
        // self.migrate_kernel_stack(base_sp, &bootstrap_table); // TODO: this corrupts the stack, needs fixed or commented

        // 4. Manually allocate a little bit more memory to bootstrap the actual page tables
        //    At the same time, switch allocators to use the kernel heap
        allocator::GLOBAL_ALLOCATOR.lock(|alloc| {
            let used_size = alloc.use_main_allocator();
            let start_offset = align_up(used_size, PAGE_SIZE);

            alloc.add_heap_region(VirtualAddress(kernel_heap_start() + start_offset), alloc_size - start_offset);
        });
        self.use_kernel_heap_addresses = true;

        // 5. Re-allocate the kernel table with only heap addresses instead of direct-maps
        self.create_kernel_page_table(memory_map, alloc_start, alloc_size);

        // 6. Drop the old tables (TTBR0 + TTBR1)
        //    (this happens automatically at the end of this function)
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
                    self.physical_allocator.add_heap_region(
                        PhysicalAddress(entry.base as usize),
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
    unsafe fn bootstrap_kernel_page_table(&mut self,
                                          memory_map_result: MemoryMapResult,
                                          initial_alloc_start: PhysicalAddress,
                                          initial_alloc_size: usize) -> IRQSafeNullLock<RootPageTable> {
        let max_phys_mem = kernel_binary_start() - direct_map_virt_offset();
        if memory_map_result.highest_physical_address.0 > max_phys_mem {
            let (size, unit) = size_human_readable_ceil(max_phys_mem);
            panic!(
                "this system has too much addressable memory; only systems with less than {} {} are supported",
                size, unit
            );
        }

        // create a new root table, but don't set it as the kernel page table
        // this initial table is temporary to bootstrap the real kernel page table, so we'll drop it soon
        let bootstrap_table = IRQSafeNullLock::new(RootPageTable::new(0, VaRange::Upper));
        bootstrap_table.lock(|table| {
            self.fill_kernel_page_table(table, memory_map_result, initial_alloc_start, initial_alloc_size);

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
                    + TCR_EL1::SH0::Outer
                    + TCR_EL1::ORGN0::WriteBack_ReadAlloc_WriteAlloc_Cacheable
                    + TCR_EL1::IRGN0::WriteBack_ReadAlloc_WriteAlloc_Cacheable
                    + TCR_EL1::EPD0::EnableTTBR0Walks
                    + TCR_EL1::T0SZ.val(16)
            );

            // invalidate the previous TTBR that the bootloader provided, as we don't want to switch
            // to that when we drop this temporary table
            table.invalidate_previous_ttbr();
        });

        bootstrap_table
    }

    /// Creates the real kernel page table on the kernel heap, and switches to it.
    unsafe fn create_kernel_page_table(&mut self,
                                       memory_map_result: MemoryMapResult,
                                       initial_alloc_start: PhysicalAddress,
                                       initial_alloc_size: usize) {
        let table = IRQSafeNullLock::new(RootPageTable::new(0, VaRange::Upper));
        table.lock(|table| {
            self.fill_kernel_page_table(table, memory_map_result, initial_alloc_start, initial_alloc_size);

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
                    + TCR_EL1::EPD0::DisableTTBR0Walks
            );
        });

        self.kernel_page_table.set(table);
    }

    fn fill_kernel_page_table(&self,
                              kernel_table: &mut RootPageTable,
                              memory_map_result: MemoryMapResult,
                              initial_alloc_start: PhysicalAddress,
                              initial_alloc_size: usize) {
        // direct map all of physical memory (RW)
        let dm_offset = direct_map_virt_offset();
        kernel_table.map_range(
            &VirtualMemoryRegion::new(dm_offset, dm_offset + memory_map_result.highest_physical_address.0),
            PhysicalAddress(0),
            Attributes::DEVICE_NGNRNE | Attributes::EXECUTE_NEVER,
        ).unwrap();

        // map the kernel code (RX)
        kernel_table.map_range(
            &VirtualMemoryRegion::new(kernel_code_start(), kernel_code_end()),
            memory_map_result.kernel_physical_address,
            Attributes::NORMAL | Attributes::READ_ONLY,
        ).unwrap();

        // map the kernel data (RW)
        kernel_table.map_range(
            &VirtualMemoryRegion::new(kernel_data_start(), kernel_data_end()),
            memory_map_result.kernel_physical_address + (kernel_data_start() - kernel_binary_start()),
            Attributes::NORMAL | Attributes::EXECUTE_NEVER,
        ).unwrap();

        // map kernel heap (RW)
        kernel_table.map_range(
            &VirtualMemoryRegion::new(kernel_heap_start(), kernel_heap_start() + initial_alloc_size),
            initial_alloc_start,
            Attributes::NORMAL | Attributes::EXECUTE_NEVER,
        ).unwrap();

        // activate the new page table
        kernel_table.activate();
    }

    /// Migrates the kernel stack from the bootloader's stack to the kernel's stack location.
    unsafe fn migrate_kernel_stack(&mut self, base_sp: usize, bootstrap_table: &Rc<IRQSafeNullLock<RootPageTable>>) {
        // get the kernel_stack_end first, then the stack pointer
        // function calls otherwise can result in big stack corruption
        let kernel_stack_end = kernel_stack_end() + 1;

        // Get the current stack pointer
        let current_sp: usize;
        asm!("mov {}, sp", out(reg) current_sp);

        // stack grows downwards
        let stack_size = base_sp - current_sp;

        // allocate pages for the stack
        // we overallocate a little bit here to account for stack growth
        let (alloc_start, alloc_size) = self.kernel_alloc_unchecked(stack_size + 32);

        // map the stack downwards from its start
        let stack_start = align_down(kernel_stack_end - alloc_size, PAGE_SIZE);
        bootstrap_table.lock(|table| table.map_range(
            &VirtualMemoryRegion::new(stack_start, kernel_stack_end - 1),
            alloc_start,
            Attributes::NORMAL | Attributes::EXECUTE_NEVER,
        ).unwrap());

        // copy the stack to the new location
        // asm!("
        //         mov x0, sp
        //         mov x1, {}
        //         add x2, xzr, {}
        //     l:  ldr x3, [x0], 8
        //
        //         bl memcpy
        //      ", in(reg) base_sp,
        // in(reg) current_sp,
        // in(reg) kernel_stack_end,
        // options(nomem, nostack)
        // );

        core::ptr::copy_nonoverlapping(
            current_sp as *const u8,
            (kernel_stack_end - stack_size) as *mut u8,
            stack_size
        );
        // for offset in (0..stack_size).step_by(mem::size_of::<usize>()) {
        //     let old_stack_loc = (current_sp + offset) as *const usize;
        //     let new_stack_loc = (kernel_stack_end - stack_size + offset) as *mut usize;
        //     *new_stack_loc = *old_stack_loc;
        // }

        // set the stack pointer to the new location
        asm!("mov sp, {}", in(reg) kernel_stack_end - stack_size);
    }

    /// Allocates memory from the kernel's physical page allocator.
    /// If the allocation fails, the kernel will panic.
    ///
    /// Returns a tuple containing the allocation start address and allocation size, in that order.
    pub fn kernel_alloc(&mut self, size: usize) -> (VirtualAddress, usize) {
        if unlikely(self.kernel_page_table.get().is_none()) {
            // we haven't yet initialised the permanent kernel page table, so we can't allocate memory
            panic!("kernel_alloc called before kernel page table initialised");
        }

        // Safe because we've already checked that the kernel page table is initialised.
        let (alloc_start, alloc_size) = unsafe { self.kernel_alloc_unchecked(size) };

        (
            if self.use_kernel_heap_addresses {
                VirtualAddress(alloc_start.0 + kernel_heap_start())
            } else {
                alloc_start.into()
            },
            alloc_size
        )
    }

    /// Allocates memory from the kernel's physical page allocator.
    /// If the allocation fails, the kernel will panic.
    ///
    /// Returns a tuple containing the allocation start address and allocation size, in that order.
    ///
    /// # Safety
    ///
    /// Unsafe because the kernel page table is not checked for proper state before the allocation.
    /// This should only be directly called during the kernel's initialisation.
    unsafe fn kernel_alloc_unchecked(&mut self, size: usize) -> (PhysicalAddress, usize) {
        let size = align_up(size, PAGE_SIZE);
        if let Some(alloc_start) = self.physical_allocator.allocate(size) {
            return (alloc_start, size);
        }

        panic!("kernel_alloc: failed to allocate {} bytes to kernel heap", size);
    }
}
