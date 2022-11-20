// SPDX-License-Identifier: MIT
use core::sync::atomic::{AtomicBool, Ordering};
use crate::{console, driver};
use crate::bsp::exception::asynchronous::irq_map;
use crate::bsp::mem::map::mmio;
use crate::driver::interrupt::gicv2::GICv2;
use crate::driver::uart::PL011Uart;
use crate::exception::asynchronous::IRQNumber;

static INTERRUPT_CONTROLLER: GICv2 = unsafe {
    GICv2::new(mmio::GICD_START, mmio::GICC_START)
};

static PL011_UART: PL011Uart = unsafe {
    PL011Uart::new(0x0900_0000)
};

fn post_init_uart() -> Result<(), &'static str> {
    console::register_console(&PL011_UART);
    Ok(())
}

fn post_init_interrupt_controller() -> Result<(), &'static str> {
    crate::exception::asynchronous::register_irq_manager(&INTERRUPT_CONTROLLER);

    Ok(())
}

fn driver_interrupt_controller() -> Result<(), &'static str> {
    let descriptor = driver::DeviceDriverDescriptor::new(
        &INTERRUPT_CONTROLLER,
        Some(post_init_interrupt_controller),
        None,
    );
    driver::driver_manager().register(descriptor);

    Ok(())
}

fn driver_uart() -> Result<(), &'static str> {
    let uart_descriptor = driver::DeviceDriverDescriptor::new(
        &PL011_UART,
        Some(post_init_uart),
        Some(&irq_map::PL011_UART),
    );
    driver::driver_manager().register(uart_descriptor);

    Ok(())
}

// fn driver_fw_cfg() -> Result<(), &'static str> {
//     let fw_cfg_descriptor = driver::DeviceDriverDescriptor::new(&FW_CFG, None);
//     driver::driver_manager().register(fw_cfg_descriptor);
//
//     Ok(())
// }

pub unsafe fn init() -> Result<(), &'static str> {
    static INIT_DONE: AtomicBool = AtomicBool::new(false);
    if INIT_DONE.load(Ordering::Relaxed) {
        return Err("driver::init() called more than once");
    }

    driver_interrupt_controller()?;
    driver_uart()?;
    // driver_fw_cfg()?;
    INIT_DONE.store(true, Ordering::Relaxed);
    Ok(())
}
