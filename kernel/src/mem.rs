// SPDX-License-Identifier: MIT

//--------------------------------------------------------------------------------------------------
// Public definitions
//--------------------------------------------------------------------------------------------------

pub mod allocator;

use core::alloc::{GlobalAlloc, Layout};
use core::cell::Cell;
use limine::{LimineHhdmRequest, LimineMemmapRequest};
use crate::println;
use crate::sync::IRQSafeNullLock;

pub(crate) static BOOTLOADER_HHDM_INFO: LimineHhdmRequest = LimineHhdmRequest::new(0);
pub(crate) static BOOTLOADER_MAP_INFO: LimineMemmapRequest = LimineMemmapRequest::new(0);

pub struct KernelMemoryManager {
    inner: IRQSafeNullLock<KernelMemoryManagerInner>,
}

pub trait MemoryManager {
    /// Initialise the memory manager, switching from the bootloader-provided
    /// page tables to our own kernel-provided page tables.
    fn init_kernel_paging(&self) -> Result<(), &'static str>;
}

//--------------------------------------------------------------------------------------------------
// Public code
//--------------------------------------------------------------------------------------------------
impl MemoryManager for KernelMemoryManager {
    fn init_kernel_paging(&self) -> Result<(), &'static str> {
        todo!()
    }
}

//--------------------------------------------------------------------------------------------------
// Private definitions
//--------------------------------------------------------------------------------------------------
struct KernelMemoryManagerInner {

}

//--------------------------------------------------------------------------------------------------
// Private code
//--------------------------------------------------------------------------------------------------
