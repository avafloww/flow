// SPDX-License-Identifier: MIT
//
// Portions of this file are derived from the aarch64-paging crate, which is redistributed in Flow
// under the MIT License. For more details, see: https://github.com/google/aarch64-paging

//--------------------------------------------------------------------------------------------------
// Public definitions
//--------------------------------------------------------------------------------------------------

use core::fmt::{self, Display, Formatter};

use paging::{VirtualAddress, VirtualMemoryRegion};

pub mod paging;

/// An error attempting to map some range in the page table.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MapError {
    /// The address requested to be mapped was out of the range supported by the page table
    /// configuration.
    AddressRange(VirtualAddress),
    /// The end of the memory region is before the start.
    RegionBackwards(VirtualMemoryRegion),
}

impl Display for MapError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Self::AddressRange(va) => write!(f, "Virtual address {} out of range", va),
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
