// SPDX-License-Identifier: MIT

use core::alloc::{GlobalAlloc, Layout};
use core::intrinsics::unlikely;
use core::mem;
use core::sync::atomic::AtomicBool;

use crate::info;
use crate::mem::allocator::align_up;
use crate::mem::direct_map_virt_offset;
use crate::mem::vm::paging::{PAGE_SIZE, PhysicalAddress, VirtualAddress};
use crate::sync::interface::Mutex;
use crate::sync::IRQSafeNullLock;

//--------------------------------------------------------------------------------------------------
// Public definitions
//--------------------------------------------------------------------------------------------------
pub struct PhysicalPageAllocator {
    head: ListNode,
}

//--------------------------------------------------------------------------------------------------
// Public code
//--------------------------------------------------------------------------------------------------
impl PhysicalPageAllocator {
    pub const fn new() -> Self {
        Self {
            head: ListNode::new(0),
        }
    }

    /// Adds a physical memory region to the allocator.
    pub unsafe fn add_heap_region(&mut self, heap_start: PhysicalAddress, heap_size: usize) {
        self.add_free_region(heap_start.into(), heap_size);
    }

    /// Adds a direct-mapped virtual address to the physical allocator.
    unsafe fn add_free_region(&mut self, addr: VirtualAddress, size: usize) {
        assert_eq!(align_up(addr.0, mem::align_of::<ListNode>()), addr.0);
        assert!(size >= mem::size_of::<ListNode>());

        let mut node = ListNode::new(size);
        node.next = self.head.next.take();

        let node_ptr = addr.0 as *mut ListNode;
        node_ptr.write(node);
        self.head.next = Some(&mut *node_ptr)
    }

    /// Finds a free region with the given size, removes it from the list, and returns
    /// its start physical address from the direct-map.
    pub fn allocate(&mut self, size: usize) -> Option<PhysicalAddress> {
        self.find_region(size).map(|alloc_start| PhysicalAddress(alloc_start.0 - direct_map_virt_offset()))
    }

    /// Finds a free region with the given size and alignment, removes it from the list, and returns
    /// the list node and its start address.
    fn find_region(&mut self, size: usize) -> Option<VirtualAddress> {
        let mut current = &mut self.head;

        while let Some(ref mut region) = current.next {
            if let Ok(alloc_start) = Self::alloc_from_region(&region, size) {
                // we can allocate this region, so remove it from the list
                let next = region.next.take();
                current.next = next;
                return Some(VirtualAddress(alloc_start));
            } else {
                // try the next region
                current = current.next.as_mut().unwrap();
            }
        }

        None
    }

    /// Tries to allocate a region of the given size and alignment from the given region.
    /// Returns the start address of the allocated region if successful.
    ///
    /// # Safety
    ///
    /// Assumes the input size is a multiple of the page size.
    fn alloc_from_region(region: &ListNode, size: usize) -> Result<usize, ()> {
        let alloc_start = align_up(region.start_addr(), PAGE_SIZE);
        let alloc_end = alloc_start.checked_add(size).ok_or(())?;

        if alloc_end > region.end_addr() {
            // region too small
            return Err(());
        }

        let excess_size = region.end_addr() - alloc_end;

        // either excess_size == 0 (perfect fit), or excess_size >= sizeof(ListNode) (gives us
        // room to continue the linked list); if neither, we can't allocate this region
        if excess_size > 0 && unlikely(excess_size < mem::size_of::<ListNode>()) {
            return Err(());
        }

        Ok(alloc_start)
    }

    fn direct_map_virt_to_phys(&self, virt_addr: VirtualAddress) -> PhysicalAddress {
        PhysicalAddress(virt_addr.0 - direct_map_virt_offset())
    }
}

//--------------------------------------------------------------------------------------------------
// Private definitions
//--------------------------------------------------------------------------------------------------
/// Represents a node of the linked list allocator.
struct ListNode {
    next: Option<&'static mut ListNode>,
    size: usize,
}

//--------------------------------------------------------------------------------------------------
// Private code
//--------------------------------------------------------------------------------------------------
impl ListNode {
    /// Creates a new node with the given size.
    const fn new(size: usize) -> Self {
        Self {
            next: None,
            size,
        }
    }

    /// Returns the start address of this memory region.
    fn start_addr(&self) -> usize {
        self as *const Self as usize
    }

    /// Returns the end address of this memory region.
    fn end_addr(&self) -> usize {
        self.start_addr() + self.size
    }
}

impl PhysicalPageAllocator {
    /// Adjusts the given layout so that the resulting allocated region can also store a ListNode.
    ///
    /// Returns the adjusted size and alignment.
    fn size_align(layout: Layout) -> (usize, usize) {
        let layout = layout
            .align_to(mem::align_of::<ListNode>())
            .expect("adjusting alignment failed")
            .pad_to_align();
        let size = layout.size().max(mem::size_of::<ListNode>());
        (size, layout.align())
    }
}
