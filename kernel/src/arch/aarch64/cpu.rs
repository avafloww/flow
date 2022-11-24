// SPDX-License-Identifier: MIT
use core::arch::asm;

use crate::mem;
use aarch64_cpu::asm;
use aarch64_cpu::registers::{CNTFRQ_EL0, CNTPCT_EL0, MPIDR_EL1};
use tock_registers::interfaces::Readable;

use crate::time::{KernelTimerData, KERNEL_TIMER_DATA};

pub static BOOT_CORE_ID: u64 = 0;

/// The entry point for the kernel.
///
/// # Safety
///
/// Expected state at start:
/// - Current execution level is EL1.
/// - MMU enabled, and kernel loaded into the higher half of the address space.
/// - BSS is zeroed.
#[no_mangle]
pub unsafe extern "C" fn _start() -> ! {
    // re-enter EL1h, since Limine drops us in EL1t
    asm!("msr spsel, #1");

    // add the direct map offset to the current stack pointer
    asm!(
        "mov x9, {dm_offset}",
        "add sp, sp, x9",
        dm_offset = in(reg) mem::direct_map_virt_offset(),
    );

    // Only proceed on the boot core for now
    if core_id::<u64>() != BOOT_CORE_ID {
        loop {
            asm::wfe();
        }
    }

    // set up some kernel constants
    KERNEL_TIMER_DATA.set(KernelTimerData::new(CNTFRQ_EL0.get(), CNTPCT_EL0.get()));

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
pub fn core_id<T>() -> T
where
    T: From<u8>,
{
    const CORE_MASK: u64 = 0b11;
    T::from((MPIDR_EL1.get() & CORE_MASK) as u8)
}
