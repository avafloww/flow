// SPDX-License-Identifier: MIT
#[cfg(feature = "bsp_qemu")]
mod qemu;
#[cfg(feature = "bsp_qemu")]
pub use qemu::*;
