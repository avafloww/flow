// SPDX-License-Identifier: MIT
use core::num::{NonZeroU128, NonZeroU32, NonZeroU64};
use core::ops::{Add, Div, Sub};
use core::time::Duration;

use aarch64_cpu::asm::barrier;
use aarch64_cpu::registers::CNTPCT_EL0;
use tock_registers::interfaces::Readable;

use crate::sync::OnceCell;
use crate::warn;

const NANOSEC_PER_SEC: NonZeroU64 = NonZeroU64::new(1_000_000_000).unwrap();

#[derive(Copy, Clone, PartialOrd, PartialEq)]
struct GenericTimerCounterValue(u64);

// safety: these are set once at kernel boot time and never modified again
pub(crate) static KERNEL_TIMER_DATA: OnceCell<KernelTimerData> = OnceCell::new();

pub struct KernelTimerData {
    arch_timer_counter_frequency: NonZeroU64,
    kernel_boot_time: GenericTimerCounterValue,
}

impl KernelTimerData {
    pub const fn new(
        arch_timer_counter_frequency: u64,
        kernel_boot_time: u64,
    ) -> Self {
        Self {
            arch_timer_counter_frequency: NonZeroU64::new(arch_timer_counter_frequency).unwrap(),
            kernel_boot_time: GenericTimerCounterValue(kernel_boot_time),
        }
    }
}

impl GenericTimerCounterValue {
    pub const MAX: Self = GenericTimerCounterValue(u64::MAX);
}

impl Add for GenericTimerCounterValue {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        GenericTimerCounterValue(self.0.wrapping_add(rhs.0))
    }
}

impl Sub for GenericTimerCounterValue {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        GenericTimerCounterValue(self.0.wrapping_sub(rhs.0))
    }
}

impl From<GenericTimerCounterValue> for Duration {
    fn from(value: GenericTimerCounterValue) -> Self {
        if value.0 == 0 {
            return Duration::ZERO;
        }

        let freq: NonZeroU64 = KERNEL_TIMER_DATA.arch_timer_counter_frequency;
        // Div<NonZeroU64> implementation for u64 cannot panic.
        let secs = value.0.div(freq);
        let subsec = value.0 % freq;

        // This is safe, because frequency can never be greater than u32::MAX, which means the
        // largest theoretical value for sub_second_counter_value is (u32::MAX - 1). Therefore,
        // (sub_second_counter_value * NANOSEC_PER_SEC) cannot overflow an u64.
        //
        // The subsequent division ensures the result fits into u32, since the max result is smaller
        // than NANOSEC_PER_SEC. Therefore, just cast it to u32 using `as`.
        let nanos = unsafe { subsec.unchecked_mul(u64::from(NANOSEC_PER_SEC)) }.div(freq) as u32;

        Duration::new(secs, nanos)
    }
}

fn max_duration() -> Duration {
    Duration::from(GenericTimerCounterValue::MAX)
}

impl TryFrom<Duration> for GenericTimerCounterValue {
    type Error = &'static str;

    fn try_from(value: Duration) -> Result<Self, Self::Error> {
        if value < resolution() {
            return Ok(GenericTimerCounterValue(0));
        }

        if value > max_duration() {
            return Err("duration too large");
        }

        let freq: u128 = <u64 as Into<u128>>::into(u64::from(KERNEL_TIMER_DATA.arch_timer_counter_frequency));
        let duration: u128 = value.as_nanos();

        // This is safe, because frequency can't exceed u32::MAX, and (Duration::MAX.as_nanos() * u32::MAX)
        // is less than u128::MAX.
        let counter_value = unsafe { duration.unchecked_mul(freq) }.div(NonZeroU128::from(NANOSEC_PER_SEC));

        // Cast to u64, since we're <= max_duration() already.
        Ok(GenericTimerCounterValue(counter_value as u64))
    }
}

#[inline(always)]
fn read_cntpct() -> GenericTimerCounterValue {
    // Prevent reordering of instructions from reading the counter ahead of time.
    barrier::isb(barrier::SY);
    let cnt = CNTPCT_EL0.get();

    GenericTimerCounterValue(cnt)
}

// Public code
pub fn resolution() -> Duration {
    Duration::from(GenericTimerCounterValue(1))
}

pub fn uptime_sys() -> Duration {
    read_cntpct().into()
}

pub fn uptime_kernel() -> Duration {
    let uptime = read_cntpct() - KERNEL_TIMER_DATA.kernel_boot_time;

    uptime.into()
}

pub fn spin_for(duration: Duration) {
    let start = read_cntpct();
    let delta: GenericTimerCounterValue = match duration.try_into() {
        Err(msg) => {
            warn!("spin_for: {}", msg);
            return;
        }
        Ok(val) => val
    };
    let target = start + delta;

    while GenericTimerCounterValue(CNTPCT_EL0.get()) < target {}
}
