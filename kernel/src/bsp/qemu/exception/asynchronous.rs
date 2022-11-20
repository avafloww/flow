// SPDX-License-Identifier: MIT
pub use crate::driver::interrupt::gicv2::IRQNumber;

pub(in crate::bsp) mod irq_map {
    use super::IRQNumber;

    pub const PL011_UART: IRQNumber = IRQNumber::new(33);
}
