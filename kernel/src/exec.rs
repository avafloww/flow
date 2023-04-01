// SPDX-License-Identifier: MIT

use crate::mem::allocator::align_up;
use crate::mem::vm::paging::{Attributes, RootPageTable, VirtualMemoryRegion};
use crate::mem::{virtual_memory_manager, MemoryManager};
use crate::sync::interface::Mutex;
use crate::sync::{IRQSafeNullLock, OnceCell};
use crate::{info, println};
use alloc::borrow::ToOwned;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::slice::SliceIndex;
use object::elf::{FileHeader64, PF_R, PF_W, PF_X, PT_LOAD};
use object::read::elf::{FileHeader, ProgramHeader};
use object::{
    Architecture, BinaryFormat, Endianness, File, FileKind, LittleEndian, Object, ObjectComdat,
    ObjectKind, ObjectSection, ObjectSegment, ObjectSymbol,
};

//--------------------------------------------------------------------------------------------------
// Public definitions
//--------------------------------------------------------------------------------------------------
const TEST_EXECUTABLE: &[u8] = include_bytes!("../../flow-init-stub");
static PROCESS_MANAGER: ProcessManager = ProcessManager::new();

#[inline(always)]
pub fn process_manager() -> &'static ProcessManager {
    &PROCESS_MANAGER
}

pub struct ProcessManager {
    inner: IRQSafeNullLock<ProcessManagerInner>,
}

pub struct Process {
    pid: usize,
    name: String,
    asid: u16,
    address_space: IRQSafeNullLock<RootPageTable>,
}

//--------------------------------------------------------------------------------------------------
// Public code
//--------------------------------------------------------------------------------------------------

impl ProcessManager {
    pub const fn new() -> Self {
        Self {
            inner: IRQSafeNullLock::new(ProcessManagerInner::new()),
        }
    }

    pub fn create_process(&self, name: &str) -> Result<(usize, &Process), ()> {
        self.inner.lock(|pm| pm.create_process(name))
    }
}

impl Process {
    pub fn new(pid: usize, name: String) -> Self {
        let (asid, address_space) = virtual_memory_manager().new_address_space();

        Self {
            pid,
            name,
            asid,
            address_space: IRQSafeNullLock::new(address_space),
        }
    }

    /// # Safety
    /// Changes the lower half of the address space to the address space of this process.
    unsafe fn with_context<'a>(&'a self, f: impl FnOnce(&'a Process) -> ()) {
        self.with_page_table(|pt: &mut RootPageTable| {
            pt.activate();
            f(self);
            pt.deactivate();
        });
    }

    fn with_page_table<'a>(&'a self, f: impl FnOnce(&'a mut RootPageTable)) {
        self.address_space.lock(f)
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        virtual_memory_manager()
            .free_address_space(self.asid)
            .expect("failed to free address space");
    }
}

pub fn read_test_executable() {
    info!("read_test_executable: start");
    let binary = File::parse(TEST_EXECUTABLE).unwrap();
    if binary.format() != BinaryFormat::Elf {
        info!("read_test_executable: not an ELF file");
        return;
    }

    if binary.architecture() != Architecture::Aarch64 {
        info!("read_test_executable: not an AArch64 file");
        return;
    }

    if binary.endianness() != Endianness::Little {
        info!("read_test_executable: not a little endian file");
        return;
    }

    let elf = Elf::parse(TEST_EXECUTABLE).unwrap();

    info!("Flags: {:x?}", binary.flags());
    info!(
        "Relative Address Base: {:x?}",
        binary.relative_address_base()
    );
    info!("Entry Address: {:x?}", binary.entry());

    match binary.mach_uuid() {
        Ok(Some(uuid)) => info!("Mach UUID: {:x?}", uuid),
        Ok(None) => {}
        Err(err) => info!("Failed to parse Mach UUID: {}", err),
    }
    match binary.build_id() {
        Ok(Some(build_id)) => info!("Build ID: {:x?}", build_id),
        Ok(None) => {}
        Err(err) => info!("Failed to parse build ID: {}", err),
    }
    match binary.gnu_debuglink() {
        Ok(Some((filename, crc))) => info!(
            "GNU debug link: {} CRC: {:08x}",
            String::from_utf8_lossy(filename),
            crc,
        ),
        Ok(None) => {}
        Err(err) => info!("Failed to parse GNU debug link: {}", err),
    }
    match binary.gnu_debugaltlink() {
        Ok(Some((filename, build_id))) => info!(
            "GNU debug alt link: {}, build ID: {:x?}",
            String::from_utf8_lossy(filename),
            build_id,
        ),
        Ok(None) => {}
        Err(err) => info!("Failed to parse GNU debug alt link: {}", err),
    }
    match binary.pdb_info() {
        Ok(Some(info)) => info!(
            "PDB file: {}, GUID: {:x?}, Age: {}",
            String::from_utf8_lossy(info.path()),
            info.guid(),
            info.age()
        ),
        Ok(None) => {}
        Err(err) => info!("Failed to parse PE CodeView info: {}", err),
    }

    for phdr in elf.program_headers(LittleEndian, TEST_EXECUTABLE).unwrap() {
        info!("Program Header: {:?}", phdr);
    }

    for segment in binary.segments() {
        info!(
            "Segment name: {:?}",
            segment.name().unwrap_or(Some("<no name>"))
        );
        info!("{:x?}", segment);
    }

    for section in binary.sections() {
        info!("{}: {:x?}", section.index().0, section);
    }

    info!("Symbols");
    for symbol in binary.symbols() {
        info!("{}: {:x?}", symbol.index().0, symbol);
    }

    for section in binary.sections() {
        if section.relocations().next().is_some() {
            info!(
                "\n{} relocations",
                section.name().unwrap_or("<invalid name>")
            );
            for relocation in section.relocations() {
                info!("{:x?}", relocation);
            }
        }
    }

    println!();

    info!("Dynamic symbols");
    for symbol in binary.dynamic_symbols() {
        info!("{}: {:x?}", symbol.index().0, symbol);
    }

    if let Some(relocations) = binary.dynamic_relocations() {
        println!();
        info!("Dynamic relocations");
        for relocation in relocations {
            info!("{:x?}", relocation);
        }
    }

    let imports = binary.imports().unwrap();
    if !imports.is_empty() {
        println!();
        for import in imports {
            info!("{:?}", import);
        }
    }

    let exports = binary.exports().unwrap();
    if !exports.is_empty() {
        println!();
        for export in exports {
            info!("{:x?}", export);
        }
    }
}

pub fn load_test_executable() {
    info!("load_test_executable: start");
    let binary = File::parse(TEST_EXECUTABLE).unwrap();
    if binary.format() != BinaryFormat::Elf {
        info!("load_test_executable: not an ELF file");
        return;
    }

    if binary.architecture() != Architecture::Aarch64 {
        info!("load_test_executable: not an AArch64 file");
        return;
    }

    if binary.endianness() != Endianness::Little {
        info!("load_test_executable: not a little endian file");
        return;
    }

    let process = process_manager().create_process("test_executable");
    if let Err(err) = process {
        info!("load_test_executable: failed to create process");
        return;
    }
    let process = process.unwrap().1;
    let elf = Elf::parse(TEST_EXECUTABLE).unwrap();

    // first iteration through: gather total needed phys mem size
    let mut load_size: usize = 0;
    for phdr in elf.program_headers(LittleEndian, TEST_EXECUTABLE).unwrap() {
        if phdr.p_type(LittleEndian) == PT_LOAD {
            load_size = align_up(load_size, phdr.p_align(LittleEndian) as usize);
            load_size += phdr.p_memsz(LittleEndian) as usize;
        }
    }

    info!("load_test_executable: load_size: {} bytes", load_size);

    // allocate the memory to load the process into
    let (process_phys, process_virt_dm, alloc_size) =
        virtual_memory_manager().process_alloc(load_size);
    let process_virt: OnceCell<usize> = OnceCell::new();
    let mut phys_offset: usize = 0;

    // second iteration: set up the page tables for the process
    process.with_page_table(|pt: &mut RootPageTable| {
        for phdr in elf.program_headers(LittleEndian, TEST_EXECUTABLE).unwrap() {
            info!("Program Header: {:?}", phdr);
            if phdr.p_type(LittleEndian) == PT_LOAD {
                let flags = phdr.p_flags(LittleEndian);
                let flag_r = flags & PF_R != 0;
                let flag_w = flags & PF_W != 0;
                let flag_x = flags & PF_X != 0;
                let flags_string = format!(
                    "{}{}{}",
                    if flag_r { "R" } else { "-" },
                    if flag_w { "W" } else { "-" },
                    if flag_x { "X" } else { "-" }
                );
                info!("PT_LOAD section with flags: {}", flags_string);

                let start_virt = phdr.p_vaddr(LittleEndian) as usize;
                let end_virt = start_virt + phdr.p_memsz(LittleEndian) as usize;
                let start_phys = phdr.p_paddr(LittleEndian) as usize;

                // todo: this isn't really correct I think (not guaranteed to be first?)
                if process_virt.get().is_none() {
                    process_virt.set(start_virt);
                }

                info!(
                    "VA: {:>8x}; PA: {:>8x}; size: {:x}",
                    start_virt,
                    start_phys,
                    end_virt - start_virt
                );

                // determine pt flags
                let mut pt_flags = Attributes::NORMAL | Attributes::USER | Attributes::NON_GLOBAL;
                if !flag_w && flag_r {
                    pt_flags |= Attributes::READ_ONLY;
                }

                if !flag_x {
                    pt_flags |= Attributes::EXECUTE_NEVER;
                }

                info!("Page table flags: {:?}", pt_flags);

                // map the pages
                pt.map_range(
                    &VirtualMemoryRegion::new(start_virt, end_virt),
                    process_phys + phys_offset,
                    pt_flags,
                )
                .unwrap();

                phys_offset += end_virt - start_virt;

                // copy the data from the file into the process
                let executable_addr = TEST_EXECUTABLE.as_ptr();
                let start_file = phdr.p_offset(LittleEndian) as usize;
                let end_file = start_file + phdr.p_filesz(LittleEndian) as usize;

                // not even gonna pretend this is safe right now
                unsafe {
                    // todo: need to zero bss here
                    core::ptr::copy_nonoverlapping(
                        (executable_addr as usize + start_file) as *const u8,
                        process_virt_dm.0 as *mut u8,
                        end_file - start_file,
                    );
                }
            }
        }
    });

    // enter process context
    unsafe {
        process.with_context(|process| {
            info!("load_test_executable: entering process context");

            // execute it!
            let entry_addr = elf.e_entry(LittleEndian) as usize;
            let entry: extern "C" fn() = core::mem::transmute(entry_addr);

            info!(
                "load_test_executable: entering via entry point: 0x{:08x}",
                entry_addr
            );
            entry();

            info!("load_test_executable: exiting process context");
        });
    }
}
//--------------------------------------------------------------------------------------------------
// Private definitions
//--------------------------------------------------------------------------------------------------
type Elf = FileHeader64<LittleEndian>;
struct ProcessManagerInner {
    processes: Vec<Process>,
    next_pid: usize,
}

//--------------------------------------------------------------------------------------------------
// Private code
//--------------------------------------------------------------------------------------------------
impl ProcessManagerInner {
    const fn new() -> Self {
        Self {
            processes: Vec::new(),
            next_pid: 1,
        }
    }

    fn create_process(&mut self, name: &str) -> Result<(usize, &Process), ()> {
        let pid = self.next_pid;
        self.next_pid += 1;
        let process = Process::new(pid, name.to_owned());
        self.processes.push(process);
        Ok((pid, self.processes.last().unwrap()))
    }
}
