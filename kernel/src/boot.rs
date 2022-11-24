// SPDX-License-Identifier: MIT
use alloc::boxed::Box;
use limine::LimineBootInfoRequest;

use crate::mem::{virtual_memory_manager, MemoryManager};
use crate::{bsp, cpu, driver, exception, info, mem, println, EARLY_INIT_COMPLETE};

static BOOTLOADER_INFO: LimineBootInfoRequest = LimineBootInfoRequest::new(0);

/// # Safety
/// - MMU & caching must be initialised first.
pub unsafe fn kernel_init() -> ! {
    // set up exception handling, since we're about to invalidate the lower half of the address space
    exception::init();

    virtual_memory_manager().init();

    // init the bsp drivers
    if let Err(x) = bsp::driver::init() {
        panic!("Failed to init bsp drivers: {}", x);
    }

    // init the interrupt controller first, so other drivers can register interrupts
    driver::driver_manager().init_interrupt_controller();

    // unmask interrupts on the boot core
    exception::asynchronous::local_irq_unmask();

    // init early drivers, so we can print debug information
    driver::driver_manager().init_early();

    // lock any init state locks
    EARLY_INIT_COMPLETE.store(true, core::sync::atomic::Ordering::Relaxed);

    // serial out is now usable, load other drivers
    driver::driver_manager().init_normal();

    // exiting unsafe code, time to bootstrap the rest of the system
    kernel_main()
}

fn kernel_main() -> ! {
    println!(
        r#"
    ______
   / __/ /___ _      __
  / /_/ / __ \ | /| / /
 / __/ / /_/ / |/ |/ /
/_/ /_/\____/|__/|__/

flow v{}, built at {}"#,
        env!("CARGO_PKG_VERSION"),
        include_str!(concat!(env!("OUT_DIR"), "/timestamp.txt"))
    );
    if let Some(bootinfo) = BOOTLOADER_INFO.get_response().get() {
        println!(
            "booted by {} v{}",
            bootinfo.name.to_str().unwrap().to_str().unwrap(),
            bootinfo.version.to_str().unwrap().to_str().unwrap(),
        );
    }

    println!();

    mem::print_physical_memory_map();

    info!("Loaded drivers:");
    driver::driver_manager().enumerate();

    info!("Registered interrupts:");
    exception::asynchronous::irq_manager().print_handlers();

    info!("Allocating some memory...");
    let mut x = Box::new(42);
    info!("x = {}", x);
    *x = 43;
    info!("x = {}", x);

    info!("Entering infinite idle loop.");
    cpu::wait_forever()
}
