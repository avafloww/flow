// SPDX-License-Identifier: MIT
use core::arch::asm;

use aarch64_cpu::registers::DAIF;
use tock_registers::fields::Field;
use tock_registers::interfaces::{Readable, Writeable};

// Public code
pub fn is_local_irq_masked() -> bool {
    !is_masked::<IRQ>()
}

#[inline(always)]
pub fn local_irq_unmask() {
    unsafe {
        asm!(
            "msr DAIFClr, {arg}",
            arg = const daif_bits::IRQ,
            options(nomem, nostack, preserves_flags)
        );
    }
}

#[inline(always)]
pub fn local_irq_mask() {
    unsafe {
        asm!(
            "msr DAIFSet, {arg}",
            arg = const daif_bits::IRQ,
            options(nomem, nostack, preserves_flags)
        );
    }
}

#[inline(always)]
pub fn local_irq_mask_save() -> u64 {
    let daif = DAIF.get();
    local_irq_mask();

    daif
}

#[inline(always)]
pub fn local_irq_restore(flags: u64) {
    DAIF.set(flags);
}

mod daif_bits {
    pub const IRQ: u8 = 0b0010;
}

trait DaifField {
    fn daif_field() -> Field<u64, DAIF::Register>;
}

struct Debug;

struct SError;

struct IRQ;

struct FIQ;

impl DaifField for Debug {
    fn daif_field() -> Field<u64, DAIF::Register> {
        DAIF::D
    }
}

impl DaifField for SError {
    fn daif_field() -> Field<u64, DAIF::Register> {
        DAIF::A
    }
}

impl DaifField for IRQ {
    fn daif_field() -> Field<u64, DAIF::Register> {
        DAIF::I
    }
}

impl DaifField for FIQ {
    fn daif_field() -> Field<u64, DAIF::Register> {
        DAIF::F
    }
}

fn is_masked<T>() -> bool where T: DaifField {
    DAIF.is_set(T::daif_field())
}
