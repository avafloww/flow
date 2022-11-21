// SPDX-License-Identifier: MIT

use core::ptr::NonNull;

use crate::mem::vm::paging::{PageTable, PhysicalAddress, Translation};

//--------------------------------------------------------------------------------------------------
// Public definitions
//--------------------------------------------------------------------------------------------------
pub struct KernelTranslation {
    /// The offset from a virtual address to the intermediate physical address.
    offset: isize,
}

//--------------------------------------------------------------------------------------------------
// Public code
//--------------------------------------------------------------------------------------------------
impl Translation for KernelTranslation {
    fn allocate_table(&self) -> (NonNull<PageTable>, PhysicalAddress) {
        todo!()
    }

    unsafe fn deallocate_table(&self, page_table: NonNull<PageTable>) {
        todo!()
    }

    fn physical_to_virtual(&self, pa: PhysicalAddress) -> NonNull<PageTable> {
        todo!()
    }
}

//--------------------------------------------------------------------------------------------------
// Private definitions
//--------------------------------------------------------------------------------------------------


//--------------------------------------------------------------------------------------------------
// Private code
//--------------------------------------------------------------------------------------------------

