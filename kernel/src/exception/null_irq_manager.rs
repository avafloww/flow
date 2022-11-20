// SPDX-License-Identifier: MIT
use crate::exception::asynchronous;
use crate::exception::asynchronous::{CriticalSection, IRQHandlerDescriptor};
use crate::exception::interface::IRQManager;

pub struct NullIRQManager;

pub static NULL_IRQ_MANAGER: NullIRQManager = NullIRQManager {};

impl IRQManager for NullIRQManager {
    type IRQNumberType = asynchronous::IRQNumber;

    fn register_handler(
        &self,
        _ih_desc: IRQHandlerDescriptor<Self::IRQNumberType>,
    ) -> Result<(), &'static str> {
        panic!("IRQ manager not registered yet!");
    }

    fn enable(&self, _irq_number: &Self::IRQNumberType) {
        panic!("IRQ manager not registered yet!");
    }

    fn handle_pending_irqs<'cs>(&'cs self, _cs: &CriticalSection<'cs>) {
        panic!("IRQ manager not registered yet!");
    }
}
