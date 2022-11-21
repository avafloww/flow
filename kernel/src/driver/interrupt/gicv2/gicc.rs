// SPDX-License-Identifier: MIT
//! GICC Driver - GIC CPU interface.

use tock_registers::{
    interfaces::{Readable, Writeable},
    register_bitfields, register_structs,
    registers::ReadWrite,
};

use crate::driver::MMIODerefWrapper;
use crate::exception;

//--------------------------------------------------------------------------------------------------
// Private Definitions
//--------------------------------------------------------------------------------------------------

register_bitfields! {
    u32,

    /// CPU Interface Control Register
    CTLR [
        Enable OFFSET(0) NUMBITS(1) []
    ],

    /// Interrupt Priority Mask Register
    PMR [
        Priority OFFSET(0) NUMBITS(8) []
    ],

    /// Interrupt Acknowledge Register
    IAR [
        InterruptID OFFSET(0) NUMBITS(10) []
    ],

    /// End of Interrupt Register
    EOIR [
        EOIINTID OFFSET(0) NUMBITS(10) []
    ]
}

register_structs! {
    #[allow(non_snake_case)]
    pub RegisterBlock {
        (0x000 => CTLR: ReadWrite<u32, CTLR::Register>),
        (0x004 => PMR: ReadWrite<u32, PMR::Register>),
        (0x008 => _reserved1),
        (0x00C => IAR: ReadWrite<u32, IAR::Register>),
        (0x010 => EOIR: ReadWrite<u32, EOIR::Register>),
        (0x014  => @END),
    }
}

/// Abstraction for the associated MMIO registers.
type Registers = MMIODerefWrapper<RegisterBlock>;

//--------------------------------------------------------------------------------------------------
// Public Definitions
//--------------------------------------------------------------------------------------------------

/// Representation of the GIC CPU interface.
pub struct GICC {
    registers: Registers,
}

//--------------------------------------------------------------------------------------------------
// Public Code
//--------------------------------------------------------------------------------------------------

impl GICC {
    /// Create an instance.
    ///
    /// # Safety
    ///
    /// - The user must ensure to provide a correct MMIO start address.
    pub const unsafe fn new(mmio_start_addr: usize) -> Self {
        Self {
            registers: Registers::new(mmio_start_addr),
        }
    }

    /// Accept interrupts of any priority.
    ///
    /// Quoting the GICv2 Architecture Specification:
    ///
    ///   "Writing 255 to the GICC_PMR always sets it to the largest supported priority field
    ///    value."
    ///
    /// # Safety
    ///
    /// - GICC MMIO registers are banked per CPU core. It is therefore safe to have `&self` instead
    ///   of `&mut self`.
    pub fn priority_accept_all(&self) {
        self.registers.PMR.write(PMR::Priority.val(255)); // Comment in arch spec.
    }

    /// Enable the interface - start accepting IRQs.
    ///
    /// # Safety
    ///
    /// - GICC MMIO registers are banked per CPU core. It is therefore safe to have `&self` instead
    ///   of `&mut self`.
    pub fn enable(&self) {
        self.registers.CTLR.write(CTLR::Enable::SET);
    }

    /// Extract the number of the highest-priority pending IRQ.
    ///
    /// Can only be called from a critical section, which is ensured by taking an `CriticalSection` token.
    ///
    /// # Safety
    ///
    /// - GICC MMIO registers are banked per CPU core. It is therefore safe to have `&self` instead
    ///   of `&mut self`.
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn pending_irq_number<'cs>(
        &self,
        _ic: &exception::asynchronous::CriticalSection<'cs>,
    ) -> usize {
        self.registers.IAR.read(IAR::InterruptID) as usize
    }

    /// Complete handling of the currently active IRQ.
    ///
    /// Can only be called from a critical section, which is ensured by taking an `CriticalSection` token.
    ///
    /// To be called after `pending_irq_number()`.
    ///
    /// # Safety
    ///
    /// - GICC MMIO registers are banked per CPU core. It is therefore safe to have `&self` instead
    ///   of `&mut self`.
    #[allow(clippy::trivially_copy_pass_by_ref)]
    pub fn mark_completed<'cs>(
        &self,
        irq_number: u32,
        _ic: &exception::asynchronous::CriticalSection<'cs>,
    ) {
        self.registers.EOIR.write(EOIR::EOIINTID.val(irq_number));
    }
}
