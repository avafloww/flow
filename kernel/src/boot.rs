use alloc::boxed::Box;
// SPDX-License-Identifier: MIT
use core::borrow::BorrowMut;
use aarch64_cpu::registers::TTBR1_EL1;
use limine::LimineBootInfoRequest;
use tock_registers::interfaces::Readable;
use crate::{bsp, cpu, driver, EARLY_INIT_COMPLETE, exception, info, println};

static BOOTLOADER_INFO: LimineBootInfoRequest = LimineBootInfoRequest::new(0);

/// # Safety
/// - MMU & caching must be initialised first.
pub unsafe fn kernel_init() -> ! {
    exception::init();

    // MMU.write(|mmu| mmu.enable_mmu_and_caching()).unwrap();
    // if let Err(string) = MMU.enable_mmu_and_caching() {
    //     panic!("MMU enable error: {}", string);
    // }

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
    println!(r#"
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

    // if let Some(mem_map) = MEMORY_MAP.get_response().get() {
    //     println!("memory map:");
    //     for entry in mem_map.memmap() {
    //         println!(
    //             "  {:>8x} - {:>8x} | {:?}",
    //             entry.base,
    //             entry.base + entry.len,
    //             entry.typ
    //         );
    //     }
    // }

    println!("TTBR1_EL1: {:8x}", TTBR1_EL1.get_baddr());
    // info!("MMU regions:");
    // bsp::mem::mmu::virt_mem_layout().print_layout();

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
