// SPDX-License-Identifier: MIT
use core::time::Duration;

pub(crate) use arch_time::KernelTimerData;
pub(crate) use arch_time::KERNEL_TIMER_DATA;

#[cfg(target_arch = "aarch64")]
#[path = "arch/aarch64/time.rs"]
mod arch_time;

pub struct TimeManager;

static TIME_MANAGER: TimeManager = TimeManager::new();

pub fn time_manager() -> &'static TimeManager {
    &TIME_MANAGER
}

#[allow(unused)]
impl TimeManager {
    pub const fn new() -> Self {
        Self
    }

    /// The timer resolution.
    pub fn resolution(&self) -> Duration {
        arch_time::resolution()
    }

    /// The system uptime, including time consumed by firmware and bootloaders.
    pub fn uptime_sys(&self) -> Duration {
        arch_time::uptime_sys()
    }

    /// The system uptime, excluding time consumed by firmware and bootloaders.
    /// This is the time since the kernel was loaded.
    pub fn uptime_kernel(&self) -> Duration {
        arch_time::uptime_kernel()
    }

    /// Spin for the given duration.
    pub fn spin_for(&self, duration: Duration) {
        arch_time::spin_for(duration)
    }
}
