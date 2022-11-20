// SPDX-License-Identifier: MIT
#[cfg(target_arch = "aarch64")]
#[path = "../arch/aarch64/exception/asynchronous.rs"]
mod arch_asynchronous;

use core::marker::PhantomData;
use critical_section::{RawRestoreState, set_impl};
use crate::bsp;
use crate::exception::{interface, null_irq_manager};
use crate::sync::{InitStateLock, IRQSafeNullLock};

pub use arch_asynchronous::{
    is_local_irq_masked, local_irq_mask, local_irq_mask_save, local_irq_restore, local_irq_unmask,
};
use crate::sync::interface::ReadWriteEx;

pub type IRQNumber = bsp::exception::asynchronous::IRQNumber;

#[derive(Copy, Clone)]
pub struct IRQHandlerDescriptor<T> where T: Copy {
    number: T,
    name: &'static str,
    handler: &'static (dyn interface::IRQHandler + Sync),
}

/// An instance of this type indicates that the local core is currently executing in IRQ
/// context, aka executing an interrupt vector or subcalls of it.
///
/// Concept and implementation derived from the `CriticalSection` introduced in
/// <https://github.com/rust-embedded/bare-metal>
#[derive(Clone, Copy)]
pub struct CriticalSection<'cs> {
    _0: PhantomData<&'cs ()>,
}

static CURRENT_IRQ_MANAGER: InitStateLock<
    &'static (dyn interface::IRQManager<IRQNumberType = IRQNumber> + Sync),
> = InitStateLock::new(&null_irq_manager::NULL_IRQ_MANAGER);

impl<T> IRQHandlerDescriptor<T> where T: Copy {
    pub const fn new(
        number: T,
        name: &'static str,
        handler: &'static (dyn interface::IRQHandler + Sync),
    ) -> Self {
        Self {
            number,
            name,
            handler,
        }
    }

    pub fn number(&self) -> T {
        self.number
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn handler(&self) -> &'static (dyn interface::IRQHandler + Sync) {
        self.handler
    }
}

impl<'cs> CriticalSection<'cs> {
    /// Enters a critical section.
    ///
    /// # Safety
    ///
    /// - Creation is only allowed in interrupt vector functions.
    /// - The lifetime `'cs` is unconstrained. User code must not be able to influence the lifetime
    ///   for this type, otherwise it might become inferred to `'static`.
    #[inline(always)]
    pub unsafe fn new() -> Self {
        Self { _0: PhantomData }
    }
}

unsafe impl<'cs> critical_section::Impl for CriticalSection<'cs> {
    unsafe fn acquire() -> RawRestoreState {
        local_irq_mask_save()
    }

    unsafe fn release(restore_state: RawRestoreState) {
        local_irq_restore(restore_state);
    }
}

#[inline(always)]
pub fn exec_with_masked_irqs<T>(f: impl FnOnce() -> T) -> T {
    let saved = local_irq_mask_save();
    let ret = f();
    local_irq_restore(saved);

    ret
}

pub fn setup_critical_section_handler() {
    set_impl!(CriticalSection);
}

/// Register a new IRQ manager.
pub fn register_irq_manager(
    new_manager: &'static (dyn interface::IRQManager<IRQNumberType = IRQNumber> + Sync),
) {
    CURRENT_IRQ_MANAGER.write(|manager| *manager = new_manager);
}

/// Return a reference to the currently registered IRQ manager.
///
/// This is the IRQ manager used by the architectural interrupt handling code.
pub fn irq_manager() -> &'static dyn interface::IRQManager<IRQNumberType = IRQNumber> {
    CURRENT_IRQ_MANAGER.read(|manager| *manager)
}
