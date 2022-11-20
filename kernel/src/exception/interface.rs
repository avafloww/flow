// SPDX-License-Identifier: MIT
use crate::exception::asynchronous::{CriticalSection, IRQHandlerDescriptor};

pub trait IRQHandler {
    fn handle(&self) -> Result<(), &'static str>;
}

pub trait IRQManager {
    type IRQNumberType: Copy;

    fn register_handler(
        &self,
        ih_desc: IRQHandlerDescriptor<Self::IRQNumberType>,
    ) -> Result<(), &'static str>;

    fn enable(&self, irq_number: &Self::IRQNumberType);

    fn print_handlers(&self) {}

    /// Handles pending interrupts. This is called directly from the CPU's IRQ exception vector.
    /// This function cannot be preempted by other interrupts.
    fn handle_pending_irqs<'cs>(
        &'cs self,
        cs: &CriticalSection<'cs>,
    );
}
