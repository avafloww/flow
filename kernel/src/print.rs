// SPDX-License-Identifier: MIT
use core::fmt;
use crate::console;

#[doc(hidden)]
pub fn kprint(args: fmt::Arguments) {
    console::console().write_fmt(args).unwrap();
}

/// Prints without a newline.
///
/// Carbon copy from <https://doc.rust-lang.org/src/std/macros.rs.html>
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::print::kprint(format_args!($($arg)*)));
}

/// Prints with a newline.
///
/// Carbon copy from <https://doc.rust-lang.org/src/std/macros.rs.html>
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ({
        $crate::print::kprint(format_args_nl!($($arg)*));
    })
}

/// A non-fatal todo macro.
#[macro_export]
macro_rules! todo_print {
    () => {
        let timestamp = $crate::time::time_manager().uptime_kernel();

        $crate::println!("[  {:>3}.{:06}] TODO: {}:{}:{}",
            timestamp.as_secs(),
            timestamp.subsec_micros(),
            file!(),
            line!(),
            column!()
        );
    };
    ($($arg:tt)*) => {
        let timestamp = $crate::time::time_manager().uptime_kernel();

        $crate::println!(
            "[  {:>3}.{:06}] TODO: {}:{}:{}: {}",
            timestamp.as_secs(),
            timestamp.subsec_micros(),
            file!(),
            line!(),
            column!(),
            format_args!($($arg)*)
        );
    };
}

/// Prints an info, with a newline.
#[macro_export]
macro_rules! info {
    ($string:expr) => ({
        let timestamp = $crate::time::time_manager().uptime_kernel();

        $crate::print::kprint(format_args_nl!(
            concat!("[  {:>3}.{:06}] ", $string),
            timestamp.as_secs(),
            timestamp.subsec_micros(),
        ));
    });
    ($format_string:expr, $($arg:tt)*) => ({
        let timestamp = $crate::time::time_manager().uptime_kernel();

        $crate::print::kprint(format_args_nl!(
            concat!("[  {:>3}.{:06}] ", $format_string),
            timestamp.as_secs(),
            timestamp.subsec_micros(),
            $($arg)*
        ));
    })
}

/// Prints a warning, with a newline.
#[macro_export]
macro_rules! warn {
    ($string:expr) => ({
        let timestamp = $crate::time::time_manager().uptime_kernel();

        $crate::print::kprint(format_args_nl!(
            concat!("[W {:>3}.{:06}] ", $string),
            timestamp.as_secs(),
            timestamp.subsec_micros(),
        ));
    });
    ($format_string:expr, $($arg:tt)*) => ({
        let timestamp = $crate::time::time_manager().uptime_kernel();

        $crate::print::kprint(format_args_nl!(
            concat!("[W {:>3}.{:06}] ", $format_string),
            timestamp.as_secs(),
            timestamp.subsec_micros(),
            $($arg)*
        ));
    })
}
