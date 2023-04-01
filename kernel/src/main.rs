// SPDX-License-Identifier: MIT
#![no_std]
#![no_main]
#![feature(alloc_error_handler)]
#![feature(asm_const)]
#![feature(core_intrinsics)]
#![feature(format_args_nl)]
#![feature(panic_info_message)]
#![feature(unchecked_math)]
#![feature(const_option)]
#![feature(int_roundings)]
#![feature(cell_update)]
#![feature(const_mut_refs)]

extern crate alloc;

use core::sync::atomic::AtomicBool;

pub static EARLY_INIT_COMPLETE: AtomicBool = AtomicBool::new(false);

mod boot;
mod bsp;
mod console;
mod cpu;
mod driver;
mod exception;
mod mem;
mod panic;
mod print;
mod sync;
mod time;
mod util;
mod exec;
