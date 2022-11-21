// SPDX-License-Identifier: MIT
//
// Portions of this file are derived from the aarch64-paging crate, which is redistributed in Flow
// under the MIT License. For more details, see: https://github.com/google/aarch64-paging

//! Generic aarch64 page table manipulation functionality which doesn't assume anything about how
//! addresses are mapped.

use alloc::alloc::{alloc_zeroed, dealloc, handle_alloc_error, Layout};
use core::fmt::{self, Debug, Display, Formatter};
use core::marker::PhantomData;
use core::ops::{Add, Range, Sub};
use core::ptr::NonNull;

use bitflags::bitflags;

use crate::mem::vm::MapError;

const PAGE_SHIFT: usize = 12;

/// The pagetable level at which all entries are page mappings.
const LEAF_LEVEL: usize = 3;

/// The page size in bytes assumed by this library, 4 KiB.
pub const PAGE_SIZE: usize = 1 << PAGE_SHIFT;

/// The number of address bits resolved in one level of page table lookup. This is a function of the
/// page size.
pub const BITS_PER_LEVEL: usize = PAGE_SHIFT - 3;

/// Which virtual address range a page table is for, i.e. which TTBR register to use for it.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum VaRange {
    /// The page table covers the bottom of the virtual address space (starting at address 0), so
    /// will be used with `TTBR0`.
    Lower,
    /// The page table covers the top of the virtual address space (ending at address
    /// 0xffff_ffff_ffff_ffff), so will be used with `TTBR1`.
    Upper,
}

/// An aarch64 virtual address, the input type of a stage 1 page table.
#[derive(Copy, Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct VirtualAddress(pub usize);

impl Display for VirtualAddress {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "{:#018x}", self.0)
    }
}

impl Debug for VirtualAddress {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "VirtualAddress({})", self)
    }
}

impl Sub for VirtualAddress {
    type Output = usize;

    fn sub(self, other: Self) -> Self::Output {
        self.0 - other.0
    }
}

impl Add<usize> for VirtualAddress {
    type Output = Self;

    fn add(self, other: usize) -> Self {
        Self(self.0 + other)
    }
}

impl Sub<usize> for VirtualAddress {
    type Output = Self;

    fn sub(self, other: usize) -> Self {
        Self(self.0 - other)
    }
}

/// A range of virtual addresses which may be mapped in a page table.
#[derive(Clone, Eq, PartialEq)]
pub struct MemoryRegion(Range<VirtualAddress>);

/// An aarch64 physical address or intermediate physical address, the output type of a stage 1 page
/// table.
#[derive(Copy, Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct PhysicalAddress(pub usize);

impl Display for PhysicalAddress {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "{:#018x}", self.0)
    }
}

impl Debug for PhysicalAddress {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "PhysicalAddress({})", self)
    }
}

impl Sub for PhysicalAddress {
    type Output = usize;

    fn sub(self, other: Self) -> Self::Output {
        self.0 - other.0
    }
}

impl Add<usize> for PhysicalAddress {
    type Output = Self;

    fn add(self, other: usize) -> Self {
        Self(self.0 + other)
    }
}

impl Sub<usize> for PhysicalAddress {
    type Output = Self;

    fn sub(self, other: usize) -> Self {
        Self(self.0 - other)
    }
}

/// Returns the size in bytes of the address space covered by a single entry in the page table at
/// the given level.
fn granularity_at_level(level: usize) -> usize {
    PAGE_SIZE << ((LEAF_LEVEL - level) * BITS_PER_LEVEL)
}

/// An implementation of this trait needs to be provided to the mapping routines, so that the
/// physical addresses used in the page tables can be converted into virtual addresses that can be
/// used to access their contents from the code.
pub trait Translation {
    /// Allocates a zeroed page, which is already mapped, to be used for a new subtable of some
    /// pagetable. Returns both a pointer to the page and its physical address.
    fn allocate_table(&self) -> (NonNull<PageTable>, PhysicalAddress);

    /// Deallocates the page which was previous allocated by [`allocate_table`](Self::allocate_table).
    ///
    /// # Safety
    ///
    /// The memory must have been allocated by `allocate_table` on the same `Translation`, and not
    /// yet deallocated.
    unsafe fn deallocate_table(&self, page_table: NonNull<PageTable>);

    /// Given the physical address of a subtable, returns the virtual address at which it is mapped.
    fn physical_to_virtual(&self, pa: PhysicalAddress) -> NonNull<PageTable>;
}

impl MemoryRegion {
    /// Constructs a new `MemoryRegion` for the given range of virtual addresses.
    ///
    /// The start is inclusive and the end is exclusive. Both will be aligned to the [`PAGE_SIZE`],
    /// with the start being rounded down and the end being rounded up.
    pub const fn new(start: usize, end: usize) -> MemoryRegion {
        MemoryRegion(
            VirtualAddress(align_down(start, PAGE_SIZE))..VirtualAddress(align_up(end, PAGE_SIZE)),
        )
    }

    /// Returns the first virtual address of the memory range.
    pub const fn start(&self) -> VirtualAddress {
        self.0.start
    }

    /// Returns the first virtual address after the memory range.
    pub const fn end(&self) -> VirtualAddress {
        self.0.end
    }

    /// Returns the length of the memory region in bytes.
    pub const fn len(&self) -> usize {
        self.0.end.0 - self.0.start.0
    }

    /// Returns whether the memory region contains exactly 0 bytes.
    pub const fn is_empty(&self) -> bool {
        self.0.start.0 == self.0.end.0
    }
}

impl From<Range<VirtualAddress>> for MemoryRegion {
    fn from(range: Range<VirtualAddress>) -> Self {
        Self::new(range.start.0, range.end.0)
    }
}

impl Display for MemoryRegion {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "{}..{}", self.0.start, self.0.end)
    }
}

impl Debug for MemoryRegion {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        Display::fmt(self, f)
    }
}

/// A complete hierarchy of page tables including all levels.
pub struct RootTable<T: Translation> {
    table: PageTableWithLevel<T>,
    translation: T,
    pa: PhysicalAddress,
    va_range: VaRange,
}

impl<T: Translation> RootTable<T> {
    /// Creates a new page table starting at the given root level.
    ///
    /// The level must be between 0 and 3. The value of `TCR_EL1.T0SZ` must be set appropriately
    /// to match.
    pub fn new(translation: T, level: usize, va_range: VaRange) -> Self {
        if level > LEAF_LEVEL {
            panic!("Invalid root table level {}.", level);
        }
        let (table, pa) = PageTableWithLevel::new(&translation, level);
        RootTable {
            table,
            translation,
            pa,
            va_range,
        }
    }

    /// Returns the size in bytes of the virtual address space which can be mapped in this page
    /// table.
    ///
    /// This is a function of the chosen root level.
    pub fn size(&self) -> usize {
        granularity_at_level(self.table.level) << BITS_PER_LEVEL
    }

    /// Recursively maps a range into the pagetable hierarchy starting at the root level, mapping
    /// the pages to the corresponding physical address range starting at `pa`.
    ///
    /// Returns an error if the virtual address range is out of the range covered by the page table.
    pub fn map_range(
        &mut self,
        range: &MemoryRegion,
        pa: PhysicalAddress,
        flags: Attributes,
    ) -> Result<(), MapError> {
        if range.end() < range.start() {
            return Err(MapError::RegionBackwards(range.clone()));
        }
        match self.va_range {
            VaRange::Lower => {
                if (range.start().0 as isize) < 0 {
                    return Err(MapError::AddressRange(range.start()));
                } else if range.end().0 > self.size() {
                    return Err(MapError::AddressRange(range.end()));
                }
            }
            VaRange::Upper => {
                if range.start().0 as isize >= 0
                    || (range.start().0 as isize).unsigned_abs() > self.size()
                {
                    return Err(MapError::AddressRange(range.start()));
                }
            }
        }

        self.table.map_range(&self.translation, range, pa, flags);

        Ok(())
    }

    /// Returns the physical address of the root table in memory.
    pub fn to_physical(&self) -> PhysicalAddress {
        self.pa
    }

    /// Returns the TTBR for which this table is intended.
    pub fn va_range(&self) -> VaRange {
        self.va_range
    }

    /// Returns a reference to the translation used for this page table.
    pub fn translation(&self) -> &T {
        &self.translation
    }

    /// Returns the level of mapping used for the given virtual address:
    /// - `None` if it is unmapped
    /// - `Some(LEAF_LEVEL)` if it is mapped as a single page
    /// - `Some(level)` if it is mapped as a block at `level`
    #[cfg(test)]
    pub(crate) fn mapping_level(&self, va: VirtualAddress) -> Option<usize> {
        self.table.mapping_level(&self.translation, va)
    }
}

impl<T: Translation> Debug for RootTable<T> {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        writeln!(
            f,
            "RootTable {{ pa: {}, level: {}, table:",
            self.pa, self.table.level
        )?;
        self.table.fmt_indented(f, &self.translation, 0)?;
        write!(f, "}}")
    }
}

impl<T: Translation> Drop for RootTable<T> {
    fn drop(&mut self) {
        self.table.free(&self.translation)
    }
}

struct ChunkedIterator<'a> {
    range: &'a MemoryRegion,
    granularity: usize,
    start: usize,
}

impl Iterator for ChunkedIterator<'_> {
    type Item = MemoryRegion;

    fn next(&mut self) -> Option<MemoryRegion> {
        if !self.range.0.contains(&VirtualAddress(self.start)) {
            return None;
        }
        let end = self
            .range
            .0
            .end
            .0
            .min((self.start | (self.granularity - 1)) + 1);
        let c = MemoryRegion::new(self.start, end);
        self.start = end;
        Some(c)
    }
}

impl MemoryRegion {
    fn split(&self, level: usize) -> ChunkedIterator {
        ChunkedIterator {
            range: self,
            granularity: granularity_at_level(level),
            start: self.0.start.0,
        }
    }

    /// Returns whether this region can be mapped at 'level' using block mappings only.
    fn is_block(&self, level: usize) -> bool {
        let gran = granularity_at_level(level);
        (self.0.start.0 | self.0.end.0) & (gran - 1) == 0
    }
}

bitflags! {
    /// Attribute bits for a mapping in a page table.
    pub struct Attributes: usize {
        const VALID         = 1 << 0;
        const TABLE_OR_PAGE = 1 << 1;

        // The following memory types assume that the MAIR registers
        // have been programmed accordingly.
        const DEVICE_NGNRE  = 0 << 2;
        const NORMAL        = 1 << 2 | 3 << 8; // inner shareable

        const USER          = 1 << 6;
        const READ_ONLY     = 1 << 7;
        const ACCESSED      = 1 << 10;
        const NON_GLOBAL    = 1 << 11;
        const EXECUTE_NEVER = 3 << 53;
    }
}

/// Smart pointer which owns a [`PageTable`] and knows what level it is at. This allows it to
/// implement `Debug` and `Drop`, as walking the page table hierachy requires knowing the starting
/// level.
#[derive(Debug)]
struct PageTableWithLevel<T: Translation> {
    table: NonNull<PageTable>,
    level: usize,
    _translation: PhantomData<T>,
}

impl<T: Translation> PageTableWithLevel<T> {
    /// Allocates a new, zeroed, appropriately-aligned page table with the given translation,
    /// returning both a pointer to it and its physical address.
    fn new(translation: &T, level: usize) -> (Self, PhysicalAddress) {
        assert!(level <= LEAF_LEVEL);
        let (table, pa) = translation.allocate_table();
        (
            // Safe because the pointer has been allocated with the appropriate layout, and the
            // memory is zeroed which is valid initialisation for a PageTable.
            Self::from_pointer(table, level),
            pa,
        )
    }

    fn from_pointer(table: NonNull<PageTable>, level: usize) -> Self {
        Self {
            table,
            level,
            _translation: PhantomData::default(),
        }
    }

    /// Returns a reference to the descriptor corresponding to a given virtual address.
    #[cfg(test)]
    fn get_entry(&self, va: VirtualAddress) -> &Descriptor {
        let shift = PAGE_SHIFT + (LEAF_LEVEL - self.level) * BITS_PER_LEVEL;
        let index = (va.0 >> shift) % (1 << BITS_PER_LEVEL);
        // Safe because we know that the pointer is properly aligned, dereferenced and initialised,
        // and nothing else can access the page table while we hold a mutable reference to the
        // PageTableWithLevel (assuming it is not currently active).
        let table = unsafe { self.table.as_ref() };
        &table.entries[index]
    }

    /// Returns a mutable reference to the descriptor corresponding to a given virtual address.
    fn get_entry_mut(&mut self, va: VirtualAddress) -> &mut Descriptor {
        let shift = PAGE_SHIFT + (LEAF_LEVEL - self.level) * BITS_PER_LEVEL;
        let index = (va.0 >> shift) % (1 << BITS_PER_LEVEL);
        // Safe because we know that the pointer is properly aligned, dereferenced and initialised,
        // and nothing else can access the page table while we hold a mutable reference to the
        // PageTableWithLevel (assuming it is not currently active).
        let table = unsafe { self.table.as_mut() };
        &mut table.entries[index]
    }

    /// Maps the the given virtual address range in this pagetable to the corresponding physical
    /// address range starting at the given `pa`, recursing into any subtables as necessary.
    ///
    /// Assumes that the entire range is within the range covered by this pagetable.
    ///
    /// Panics if the `translation` doesn't provide a corresponding physical address for some
    /// virtual address within the range, as there is no way to roll back to a safe state so this
    /// should be checked by the caller beforehand.
    fn map_range(
        &mut self,
        translation: &T,
        range: &MemoryRegion,
        mut pa: PhysicalAddress,
        flags: Attributes,
    ) {
        let level = self.level;
        let granularity = granularity_at_level(level);

        for chunk in range.split(level) {
            let entry = self.get_entry_mut(chunk.0.start);

            if level == LEAF_LEVEL {
                // Put down a page mapping.
                entry.set(pa, flags | Attributes::ACCESSED | Attributes::TABLE_OR_PAGE);
            } else if chunk.is_block(level)
                && !entry.is_table_or_page()
                && is_aligned(pa.0, granularity)
            {
                // Rather than leak the entire subhierarchy, only put down
                // a block mapping if the region is not already covered by
                // a table mapping.
                entry.set(pa, flags | Attributes::ACCESSED);
            } else {
                let mut subtable = if let Some(subtable) = entry.subtable(translation, level) {
                    subtable
                } else {
                    let old = *entry;
                    let (mut subtable, subtable_pa) = Self::new(translation, level + 1);
                    if let (Some(old_flags), Some(old_pa)) = (old.flags(), old.output_address()) {
                        // Old was a valid block entry, so we need to split it.
                        // Recreate the entire block in the newly added table.
                        let a = align_down(chunk.0.start.0, granularity);
                        let b = align_up(chunk.0.end.0, granularity);
                        subtable.map_range(
                            translation,
                            &MemoryRegion::new(a, b),
                            old_pa,
                            old_flags,
                        );
                    }
                    entry.set(subtable_pa, Attributes::TABLE_OR_PAGE);
                    subtable
                };
                subtable.map_range(translation, &chunk, pa, flags);
            }
            pa.0 += chunk.len();
        }
    }

    fn fmt_indented(
        &self,
        f: &mut Formatter,
        translation: &T,
        indentation: usize,
    ) -> Result<(), fmt::Error> {
        // Safe because we know that the pointer is aligned, initialised and dereferencable, and the
        // PageTable won't be mutated while we are using it.
        let table = unsafe { self.table.as_ref() };

        let mut i = 0;
        while i < table.entries.len() {
            if table.entries[i].0 == 0 {
                let first_zero = i;
                while i < table.entries.len() && table.entries[i].0 == 0 {
                    i += 1;
                }
                if i - 1 == first_zero {
                    writeln!(f, "{:indentation$}{}: 0", "", first_zero)?;
                } else {
                    writeln!(f, "{:indentation$}{}-{}: 0", "", first_zero, i - 1)?;
                }
            } else {
                writeln!(f, "{:indentation$}{}: {:?}", "", i, table.entries[i])?;
                if let Some(subtable) = table.entries[i].subtable(translation, self.level) {
                    subtable.fmt_indented(f, translation, indentation + 2)?;
                }
                i += 1;
            }
        }
        Ok(())
    }

    /// Frees the memory used by this pagetable and all subtables. It is not valid to access the
    /// page table after this.
    fn free(&mut self, translation: &T) {
        // Safe because we know that the pointer is aligned, initialised and dereferencable, and the
        // PageTable won't be mutated while we are freeing it.
        let table = unsafe { self.table.as_ref() };
        for entry in table.entries {
            if let Some(mut subtable) = entry.subtable(translation, self.level) {
                // Safe because the subtable was allocated by `PageTableWithLevel::new` with the
                // global allocator and appropriate layout.
                subtable.free(translation);
            }
        }
        // Safe because the table was allocated by `PageTableWithLevel::new` with the global
        // allocator and appropriate layout.
        unsafe {
            // Actually free the memory used by the `PageTable`.
            translation.deallocate_table(self.table);
        }
    }

    /// Returns the level of mapping used for the given virtual address:
    /// - `None` if it is unmapped
    /// - `Some(LEAF_LEVEL)` if it is mapped as a single page
    /// - `Some(level)` if it is mapped as a block at `level`
    #[cfg(test)]
    fn mapping_level(&self, translation: &T, va: VirtualAddress) -> Option<usize> {
        let entry = self.get_entry(va);
        if let Some(subtable) = entry.subtable(translation, self.level) {
            subtable.mapping_level(translation, va)
        } else {
            if entry.is_valid() {
                Some(self.level)
            } else {
                None
            }
        }
    }
}

/// A single level of a page table.
#[repr(C, align(4096))]
pub struct PageTable {
    entries: [Descriptor; 1 << BITS_PER_LEVEL],
}

impl PageTable {
    /// Allocates a new zeroed, appropriately-aligned pagetable on the heap using the global
    /// allocator and returns a pointer to it.
    pub fn new() -> NonNull<Self> {
        // Safe because the pointer has been allocated with the appropriate layout by the global
        // allocator, and the memory is zeroed which is valid initialisation for a PageTable.
        unsafe { allocate_zeroed() }
    }
}

/// An entry in a page table.
///
/// A descriptor may be:
///   - Invalid, i.e. the virtual address range is unmapped
///   - A page mapping, if it is in the lowest level page table.
///   - A block mapping, if it is not in the lowest level page table.
///   - A pointer to a lower level pagetable, if it is not in the lowest level page table.
#[derive(Clone, Copy)]
#[repr(C)]
struct Descriptor(usize);

impl Descriptor {
    fn output_address(&self) -> Option<PhysicalAddress> {
        if self.is_valid() {
            Some(PhysicalAddress(
                self.0 & (!(PAGE_SIZE - 1) & !(0xffff << 48)),
            ))
        } else {
            None
        }
    }

    fn flags(self) -> Option<Attributes> {
        if self.is_valid() {
            Attributes::from_bits(self.0 & ((PAGE_SIZE - 1) | (0xffff << 48)))
        } else {
            None
        }
    }

    fn is_valid(self) -> bool {
        (self.0 & Attributes::VALID.bits()) != 0
    }

    fn is_table_or_page(self) -> bool {
        if let Some(flags) = self.flags() {
            flags.contains(Attributes::TABLE_OR_PAGE)
        } else {
            false
        }
    }

    fn set(&mut self, pa: PhysicalAddress, flags: Attributes) {
        self.0 = pa.0 | (flags | Attributes::VALID).bits();
    }

    fn subtable<T: Translation>(
        &self,
        translation: &T,
        level: usize,
    ) -> Option<PageTableWithLevel<T>> {
        if level < LEAF_LEVEL && self.is_table_or_page() {
            if let Some(output_address) = self.output_address() {
                let table = translation.physical_to_virtual(output_address);
                return Some(PageTableWithLevel::from_pointer(table, level + 1));
            }
        }
        None
    }
}

impl Debug for Descriptor {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "{:#016x}", self.0)?;
        if let (Some(flags), Some(address)) = (self.flags(), self.output_address()) {
            write!(f, " ({}, {:?})", address, flags)?;
        }
        Ok(())
    }
}

/// Allocates appropriately aligned heap space for a `T` and zeroes it.
///
/// # Safety
///
/// It must be valid to initialise the type `T` by simply zeroing its memory.
unsafe fn allocate_zeroed<T>() -> NonNull<T> {
    let layout = Layout::new::<T>();
    // Safe because we know the layout has non-zero size.
    let pointer = alloc_zeroed(layout);
    if pointer.is_null() {
        handle_alloc_error(layout);
    }
    // Safe because we just checked that the pointer is non-null.
    NonNull::new_unchecked(pointer as *mut T)
}

/// Deallocates the heap space for a `T` which was previously allocated by `allocate_zeroed`.
///
/// # Safety
///
/// The memory must have been allocated by the global allocator, with the layout for `T`, and not
/// yet deallocated.
pub(crate) unsafe fn deallocate<T>(ptr: NonNull<T>) {
    let layout = Layout::new::<T>();
    dealloc(ptr.as_ptr() as *mut u8, layout);
}

const fn align_down(value: usize, alignment: usize) -> usize {
    value & !(alignment - 1)
}

const fn align_up(value: usize, alignment: usize) -> usize {
    ((value - 1) | (alignment - 1)) + 1
}

pub(crate) const fn is_aligned(value: usize, alignment: usize) -> bool {
    value & (alignment - 1) == 0
}

#[cfg(test)]
mod tests {
    use alloc::{format, string::ToString};

    use super::*;

    #[test]
    fn display_memory_region() {
        let region = MemoryRegion::new(0x1234, 0x56789);
        assert_eq!(
            &region.to_string(),
            "0x0000000000001000..0x0000000000057000"
        );
        assert_eq!(
            &format!("{:?}", region),
            "0x0000000000001000..0x0000000000057000"
        );
    }

    #[test]
    fn subtract_virtual_address() {
        let low = VirtualAddress(0x12);
        let high = VirtualAddress(0x1234);
        assert_eq!(high - low, 0x1222);
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic]
    fn subtract_virtual_address_overflow() {
        let low = VirtualAddress(0x12);
        let high = VirtualAddress(0x1234);

        // This would overflow, so should panic.
        let _ = low - high;
    }

    #[test]
    fn add_virtual_address() {
        assert_eq!(VirtualAddress(0x1234) + 0x42, VirtualAddress(0x1276));
    }

    #[test]
    fn subtract_physical_address() {
        let low = PhysicalAddress(0x12);
        let high = PhysicalAddress(0x1234);
        assert_eq!(high - low, 0x1222);
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic]
    fn subtract_physical_address_overflow() {
        let low = PhysicalAddress(0x12);
        let high = PhysicalAddress(0x1234);

        // This would overflow, so should panic.
        let _ = low - high;
    }

    #[test]
    fn add_physical_address() {
        assert_eq!(PhysicalAddress(0x1234) + 0x42, PhysicalAddress(0x1276));
    }
}

//--------------------------------------------------------------------------------------------------
// Public definitions
//--------------------------------------------------------------------------------------------------


//--------------------------------------------------------------------------------------------------
// Public code
//--------------------------------------------------------------------------------------------------


//--------------------------------------------------------------------------------------------------
// Private definitions
//--------------------------------------------------------------------------------------------------


//--------------------------------------------------------------------------------------------------
// Private code
//--------------------------------------------------------------------------------------------------
