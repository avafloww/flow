// SPDX-License-Identifier: MIT
#![no_std]
#![no_main]
#![feature(alloc_error_handler)]
#![feature(asm_const)]
#![feature(core_intrinsics)]
#![feature(format_args_nl)] // for print/println
#![feature(panic_info_message)] // for panic handler
#![feature(unchecked_math)] // for timer speediness
#![feature(const_option)]
#![feature(int_roundings)]
#![feature(cell_update)]
#![feature(const_mut_refs)]

extern crate alloc;

use core::sync::atomic::AtomicBool;

pub static EARLY_INIT_COMPLETE: AtomicBool = AtomicBool::new(false);

mod bsp;
mod console;
mod cpu;
mod panic;
mod sync;
mod print;
mod boot;
mod driver;
mod time;
mod util;
mod exception;
mod mem;
