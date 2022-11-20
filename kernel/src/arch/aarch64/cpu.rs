// SPDX-License-Identifier: MIT
use core::arch::{asm, global_asm};
use core::num::NonZeroU64;
use aarch64_cpu::asm;
use aarch64_cpu::registers::{CNTFRQ_EL0, CNTPCT_EL0, MPIDR_EL1};
use tock_registers::interfaces::{Readable, Writeable};
use crate::time::{KERNEL_TIMER_DATA, KernelTimerData};

pub static BOOT_CORE_ID: u64 = 0;

/// The entry point for the kernel.
///
/// # Safety
///
/// Expected state at start:
/// - Current execution level is EL1.
/// - MMU enabled, and kernel loaded into the higher half of the address space.
/// - SP_EL1 set to the top of the kernel stack (the start of kernel code).
/// - BSS is zeroed.
#[no_mangle]
pub unsafe extern "C" fn _start() -> ! {
    // re-enter EL1h, since Limine drops us in EL1t
    asm!("msr spsel, #1");

    // Only proceed on the boot core for now
    if core_id::<u64>() != BOOT_CORE_ID {
        loop {
            asm::wfe();
        }
    }

    // set up some kernel constants
    KERNEL_TIMER_DATA.set(KernelTimerData::new(
        CNTFRQ_EL0.get(),
        CNTPCT_EL0.get()
    ));

    // Start the rest of the kernel init process
    crate::boot::kernel_init()
}

#[inline(always)]
pub fn wait_forever() -> ! {
    loop {
        asm::wfe();
    }
}

#[inline(always)]
pub fn nop() {
    asm::nop()
}

#[inline(always)]
pub fn core_id<T>() -> T where T: From<u8> {
    const CORE_MASK: u64 = 0b11;
    T::from((MPIDR_EL1.get() & CORE_MASK) as u8)
}
