// SPDX-License-Identifier: MIT

//--------------------------------------------------------------------------------------------------
// Public definitions
//--------------------------------------------------------------------------------------------------
pub trait Mutex {
    type Data;

    fn lock<'a, R>(&'a self, f: impl FnOnce(&'a mut Self::Data) -> R) -> R;
}

/// A reader-writer exclusion type.
/// The implementing object allows either a number of readers or at most one writer at any point
/// in time.
pub trait ReadWriteEx {
    type Data;

    /// Grants temporary mutable access to the data.
    fn write<'a, R>(&'a self, f: impl FnOnce(&'a mut Self::Data) -> R) -> R;

    /// Grants temporary immutable access to the data.
    fn read<'a, R>(&'a self, f: impl FnOnce(&'a Self::Data) -> R) -> R;
}
