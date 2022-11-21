// SPDX-License-Identifier: MIT
use core::fmt::Arguments;

use crate::console::interface::{All, Statistics, Write};
use crate::sync::interface::Mutex;
use crate::sync::IRQSafeNullLock;

pub mod interface {
    use core::fmt;

    pub trait Write {
        fn write_char(&self, c: char);

        fn write_fmt(&self, args: fmt::Arguments) -> fmt::Result;

        fn flush(&self);
    }

    pub trait Read {
        fn read_char(&self) -> char {
            ' '
        }

        fn clear_rx(&self);
    }

    pub trait Statistics {
        /// Returns the number of characters written to the console.
        fn get_tx_count(&self) -> usize {
            0
        }

        /// Returns the number of characters read from the console.
        fn get_rx_count(&self) -> usize {
            0
        }
    }

    pub trait All: Write + Statistics {}
}

struct NullConsole;

impl NullConsole {
    pub const fn new() -> NullConsole {
        NullConsole
    }
}

impl Write for NullConsole {
    fn write_char(&self, _c: char) {}

    fn write_fmt(&self, _args: Arguments) -> core::fmt::Result {
        Ok(())
    }

    fn flush(&self) {}
}

impl Statistics for NullConsole {}

impl All for NullConsole {}

static NULL_CONSOLE: NullConsole = NullConsole::new();
static CUR_CONSOLE: IRQSafeNullLock<&'static (dyn All + Sync)> =
    IRQSafeNullLock::new(&NULL_CONSOLE);

pub fn console() -> &'static dyn All {
    CUR_CONSOLE.lock(|con| *con)
}

pub fn register_console(con: &'static (dyn All + Sync)) {
    CUR_CONSOLE.lock(|cur| *cur = con);
}
