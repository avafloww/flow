pub use common::*;
pub use descriptor::*;
pub use manager::*;

mod common;
mod descriptor;
mod manager;

pub mod interrupt;
pub mod uart;

pub mod interface {
    use core::fmt;

    use crate::driver::DriverLoadOrder;

    pub trait DeviceDriver {
        type IRQNumberType: fmt::Display;

        /// Describes the load order of the driver.
        fn load_order(&self) -> DriverLoadOrder;

        /// A string describing the device driver.
        fn compatible(&self) -> &'static str;

        /// Called by the kernel to bring up the device.
        unsafe fn init(
            &'static self,
            _irq_number: Option<&Self::IRQNumberType>,
        ) -> Result<(), &'static str> {
            Ok(())
        }
    }
}

#[allow(dead_code)]
#[derive(Debug, Eq, PartialEq)]
pub enum DriverLoadOrder {
    /// The interrupt controller driver is always loaded first.
    InterruptController,

    /// The driver is loaded very early in the boot process, after the interrupt controller.
    /// This is useful for drivers that can help report errors in the boot process, such as the UART.
    Early,

    /// The driver is loaded at the normal probe-and-load stage of the boot process.
    Normal,

    /// The driver is not loaded at boot, and must be loaded manually.
    Manual,
}
