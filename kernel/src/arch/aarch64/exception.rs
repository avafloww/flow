use core::arch::global_asm;
use core::cell::UnsafeCell;

use aarch64_cpu::asm::barrier;
use aarch64_cpu::registers::VBAR_EL1;
use tock_registers::interfaces::Writeable;

use context::ExceptionContext;

use crate::exception;

// SPDX-License-Identifier: MIT
#[path = "exception/context.rs"]
mod context;

global_asm!(include_str!("exception/exception.S"));

/// Initialises exception handling.
///
/// # Safety
///
/// - Changes hardware state of the executing core.
/// - The vector table and "__exception_vector_start" in the linker script must adhere to the
///   ARMv8-A spec.
pub unsafe fn init() {
    extern "Rust" {
        // Defined in exception.S
        static __exception_vector_start: UnsafeCell<()>;
    }

    VBAR_EL1.set(__exception_vector_start.get() as u64);
    barrier::isb(barrier::SY);

    exception::asynchronous::setup_critical_section_handler();
}

fn default_exception_handler(exc: &ExceptionContext) {
    panic!("Unhandled CPU exception occurred!\n\n{}", exc);
}

// Current, EL0
#[no_mangle]
extern "C" fn eh_cel0_sync(_exc: &mut ExceptionContext) {
    panic!("Use of SP_EL0 in EL1 is not allowed!");
}

#[no_mangle]
extern "C" fn eh_cel0_irq(_exc: &mut ExceptionContext) {
    panic!("Use of SP_EL0 in EL1 is not allowed!");
}

#[no_mangle]
extern "C" fn eh_cel0_serror(_exc: &mut ExceptionContext) {
    panic!("Use of SP_EL0 in EL1 is not allowed!");
}

// Current, ELx
#[no_mangle]
extern "C" fn eh_celx_sync(exc: &mut ExceptionContext) {
    default_exception_handler(exc);
}

#[no_mangle]
extern "C" fn eh_celx_irq(_exc: &mut ExceptionContext) {
    let token = unsafe { &exception::asynchronous::CriticalSection::new() };
    exception::asynchronous::irq_manager().handle_pending_irqs(token);
}

#[no_mangle]
extern "C" fn eh_celx_serror(exc: &mut ExceptionContext) {
    default_exception_handler(exc);
}

// Lower, AArch64
#[no_mangle]
extern "C" fn eh_lower_aa64_sync(exc: &mut ExceptionContext) {
    default_exception_handler(exc);
}

#[no_mangle]
extern "C" fn eh_lower_aa64_irq(exc: &mut ExceptionContext) {
    default_exception_handler(exc);
}

#[no_mangle]
extern "C" fn eh_lower_aa64_serror(exc: &mut ExceptionContext) {
    default_exception_handler(exc);
}

// Lower, AArch32
#[no_mangle]
extern "C" fn eh_lower_aa32_sync(exc: &mut ExceptionContext) {
    default_exception_handler(exc);
}

#[no_mangle]
extern "C" fn eh_lower_aa32_irq(exc: &mut ExceptionContext) {
    default_exception_handler(exc);
}

#[no_mangle]
extern "C" fn eh_lower_aa32_serror(exc: &mut ExceptionContext) {
    default_exception_handler(exc);
}
