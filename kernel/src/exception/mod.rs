// SPDX-License-Identifier: MIT
#[cfg(target_arch = "aarch64")]
#[path = "../arch/aarch64/exception.rs"]
mod arch_exception;
mod null_irq_manager;

pub mod asynchronous;
pub mod interface;

pub use arch_exception::init;
