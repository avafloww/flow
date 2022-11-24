// SPDX-License-Identifier: MIT
use core::fmt;
use core::fmt::Formatter;

use aarch64_cpu::registers::{ESR_EL1, FAR_EL1, SPSR_EL1};
use tock_registers::interfaces::Readable;
use tock_registers::registers::InMemoryRegister;

#[repr(transparent)]
struct SpsrEL1(InMemoryRegister<u64, SPSR_EL1::Register>);

struct EsrEL1(InMemoryRegister<u64, ESR_EL1::Register>);

#[repr(C)]
pub struct ExceptionContext {
    /// General purpose registers
    gpr: [u64; 30],

    /// x30 - link register
    lr: u64,

    /// Exception link register ($pc at time of exception)
    elr_el1: u64,

    /// Saved program status register
    spsr_el1: SpsrEL1,

    /// Exception syndrome register
    esr_el1: EsrEL1,
}

impl fmt::Display for SpsrEL1 {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        writeln!(f, "SPSR_EL1: {:#010x}", self.0.get())?;

        let to_flag_str = |x| -> _ {
            if x {
                "*"
            } else {
                " "
            }
        };

        writeln!(
            f,
            "    Flags: (N)egative[{}] (Z)ero[{}] (C)arry[{}] O(V)erflow[{}]",
            to_flag_str(self.0.is_set(SPSR_EL1::N)),
            to_flag_str(self.0.is_set(SPSR_EL1::Z)),
            to_flag_str(self.0.is_set(SPSR_EL1::C)),
            to_flag_str(self.0.is_set(SPSR_EL1::V))
        )?;

        let to_mask_str = |x| -> _ {
            if x {
                "M"
            } else {
                "U"
            }
        };

        writeln!(
            f,
            "    Exception state: (D)ebug[{}] (A)Serror[{}] (I)RQ[{}] (F)IQ[{}]",
            to_mask_str(self.0.is_set(SPSR_EL1::D)),
            to_mask_str(self.0.is_set(SPSR_EL1::A)),
            to_mask_str(self.0.is_set(SPSR_EL1::I)),
            to_mask_str(self.0.is_set(SPSR_EL1::F))
        )?;

        write!(
            f,
            "    (IL)legalExecState[{}]",
            to_flag_str(self.0.is_set(SPSR_EL1::IL))
        )
    }
}

impl EsrEL1 {
    #[inline(always)]
    fn exception_class(&self) -> Option<ESR_EL1::EC::Value> {
        self.0.read_as_enum(ESR_EL1::EC)
    }
}

impl fmt::Display for EsrEL1 {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        writeln!(f, "ESR_EL1: {:#010x}", self.0.get())?;
        let ec_desc = match self.exception_class() {
            Some(ESR_EL1::EC::Value::DataAbortCurrentEL) => "Data abort (current EL)",
            _ => "Unknown",
        };
        writeln!(
            f,
            "    Exception class: {:#x} - {}",
            self.0.read(ESR_EL1::EC),
            ec_desc
        )?;
        write!(
            f,
            "    Instruction Specific Syndrome (ISS): {:#x}",
            self.0.read(ESR_EL1::ISS)
        )
    }
}

impl ExceptionContext {
    #[inline(always)]
    fn exception_class(&self) -> Option<ESR_EL1::EC::Value> {
        self.esr_el1.exception_class()
    }

    #[inline(always)]
    fn fault_address_valid(&self) -> bool {
        use ESR_EL1::EC::Value::*;

        match self.exception_class() {
            None => false,
            Some(ec) => matches!(
                ec,
                InstrAbortLowerEL
                    | InstrAbortCurrentEL
                    | PCAlignmentFault
                    | DataAbortLowerEL
                    | DataAbortCurrentEL
                    | WatchpointLowerEL
                    | WatchpointCurrentEL
            ),
        }
    }
}

impl fmt::Display for ExceptionContext {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}", self.esr_el1)?;
        if self.fault_address_valid() {
            writeln!(f, "    FAR_EL1: {:#018x}", FAR_EL1.get() as usize)?;
        }

        writeln!(f, "{}", self.spsr_el1)?;
        writeln!(f, "ELR_EL1: {:#018x}", self.elr_el1)?;
        writeln!(f)?;
        writeln!(f, "Registers:")?;
        write!(f, "    ")?;

        let alternating = |x| -> _ {
            if x % 2 == 0 {
                "    "
            } else {
                "\n    "
            }
        };

        for (i, reg) in self.gpr.iter().enumerate() {
            write!(f, "x{: <2}: {: >#018x}{}", i, reg, alternating(i))?;
        }
        write!(f, "lr : {:#018x}", self.lr)
    }
}
