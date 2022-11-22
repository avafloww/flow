// SPDX-License-Identifier: MIT
//
// Portions of this file are derived from the aarch64-paging crate, which is redistributed in Flow
// under the MIT License. For more details, see: https://github.com/google/aarch64-paging

//--------------------------------------------------------------------------------------------------
// Public definitions
//--------------------------------------------------------------------------------------------------

#[cfg(target_arch = "aarch64")]
use core::arch::asm;
use core::fmt::{self, Display, Formatter};
use aarch64_cpu::asm::barrier;

use paging::{
    Attributes, VirtualMemoryRegion, PhysicalAddress, RootPageTable, Translation, VaRange, VirtualAddress,
};

pub mod paging;

/// An error attempting to map some range in the page table.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MapError {
    /// The address requested to be mapped was out of the range supported by the page table
    /// configuration.
    AddressRange(VirtualAddress),
    /// The address requested to be mapped was not valid for the mapping in use.
    InvalidVirtualAddress(VirtualAddress),
    /// The end of the memory region is before the start.
    RegionBackwards(VirtualMemoryRegion),
}

impl Display for MapError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Self::AddressRange(va) => write!(f, "Virtual address {} out of range", va),
            Self::InvalidVirtualAddress(va) => {
                write!(f, "Invalid virtual address {} for mapping", va)
            }
            Self::RegionBackwards(region) => {
                write!(f, "End of memory region {} is before start.", region)
            }
        }
    }
}

//--------------------------------------------------------------------------------------------------
// Public code
//--------------------------------------------------------------------------------------------------


//--------------------------------------------------------------------------------------------------
// Private definitions
//--------------------------------------------------------------------------------------------------


//--------------------------------------------------------------------------------------------------
// Private code
//--------------------------------------------------------------------------------------------------

