// SPDX-License-Identifier: MIT

//--------------------------------------------------------------------------------------------------
// Public definitions
//--------------------------------------------------------------------------------------------------
#[rustfmt::skip]
pub(super) mod map {
    // todo: this is garbage, but temporary because of dt discovery coming soon
    pub const DIRECT_MAP_OFFSET: usize = 0xFFFF_8000_0000_0000;

    /// The inclusive end address of the memory map.
    ///
    /// End address + 1 must be power of two.

    /// Physical devices.
    pub mod mmio {
        use super::*;

        pub const PL011_UART_START: usize =         0x0900_0000 + DIRECT_MAP_OFFSET;
        pub const GICD_START:       usize =         0x0800_0000 + DIRECT_MAP_OFFSET;
        pub const GICC_START:       usize =         0x0801_0000 + DIRECT_MAP_OFFSET;
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

