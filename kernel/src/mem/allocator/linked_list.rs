// SPDX-License-Identifier: MIT

use core::alloc::{GlobalAlloc, Layout};
use core::mem;

use crate::info;
use crate::mem::allocator::align_up;
use crate::sync::interface::Mutex;
use crate::sync::IRQSafeNullLock;

//--------------------------------------------------------------------------------------------------
// Public definitions
//--------------------------------------------------------------------------------------------------
pub struct LinkedListAllocator {
    head: ListNode,
}

//--------------------------------------------------------------------------------------------------
// Public code
//--------------------------------------------------------------------------------------------------
impl LinkedListAllocator {
    pub const fn new() -> Self {
        Self {
            head: ListNode::new(0),
        }
    }

    /// Adds a physical memory region to the allocator.
    pub unsafe fn add_heap_region(&mut self, heap_start: usize, heap_size: usize) {
        self.add_free_region(heap_start, heap_size);
    }

    unsafe fn add_free_region(&mut self, addr: usize, size: usize) {
        assert_eq!(align_up(addr, mem::align_of::<ListNode>()), addr);
        assert!(size >= mem::size_of::<ListNode>());

        let mut node = ListNode::new(size);
        node.next = self.head.next.take();
        let node_ptr = addr as *mut ListNode;
        node_ptr.write(node);
        self.head.next = Some(&mut *node_ptr)
    }

    /// Finds a free region with the given size and alignment, removes it from the list, and returns
    /// its start address.
    pub fn allocate(&mut self, size: usize, align: usize) -> Option<usize> {
        if let Some((_, alloc_start)) = self.find_region(size, align) {
            Some(alloc_start)
        } else {
            None
        }
    }

    /// Finds a free region with the given size and alignment, removes it from the list, and returns
    /// the list node and its start address.
    fn find_region(&mut self, size: usize, align: usize) -> Option<(&'static mut ListNode, usize)> {
        let mut current = &mut self.head;

        while let Some(ref mut region) = current.next {
            if let Ok(alloc_start) = Self::alloc_from_region(&region, size, align) {
                // we can allocate this region, so remove it from the list
                let next = region.next.take();
                let ret = Some((current.next.take().unwrap(), alloc_start));
                current.next = next;
                return ret;
            } else {
                // try the next region
                current = current.next.as_mut().unwrap();
            }
        }

        None
    }

    /// Tries to allocate a region of the given size and alignment from the given region.
    /// Returns the start address of the allocated region if successful.
    fn alloc_from_region(region: &ListNode, size: usize, align: usize) -> Result<usize, ()> {
        let alloc_start = align_up(region.start_addr(), align);
        let alloc_end = alloc_start.checked_add(size).ok_or(())?;

        if alloc_end > region.end_addr() {
            // region too small
            return Err(());
        }

        let excess_size = region.end_addr() - alloc_end;

        // either excess_size == 0 (perfect fit), or excess_size >= sizeof(ListNode) (gives us
        // room to continue the linked list); if neither, we can't allocate this region
        if excess_size > 0 && excess_size < mem::size_of::<ListNode>() {
            return Err(());
        }

        Ok(alloc_start)
    }
}

unsafe impl GlobalAlloc for IRQSafeNullLock<LinkedListAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.lock(|alloc| {
            alloc.alloc(layout)
        })
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.lock(|alloc| {
            alloc.dealloc(ptr, layout)
        })
    }
}

impl LinkedListAllocator {
    pub(crate) unsafe fn alloc(&mut self, layout: Layout) -> *mut u8 {
        let (size, align) = LinkedListAllocator::size_align(layout);
        if let Some((region, alloc_start)) = self.find_region(size, align) {
            let alloc_end = alloc_start.checked_add(size).expect("overflow");
            let excess_size = region.end_addr() - alloc_end;
            if excess_size > 0 {
                self.add_free_region(alloc_end, excess_size);
            }

            alloc_start as *mut u8
        } else {
            core::ptr::null_mut()
        }
    }

    pub(crate) unsafe fn dealloc(&mut self, ptr: *mut u8, layout: Layout) {
        let (size, _) = LinkedListAllocator::size_align(layout);
        self.add_free_region(ptr as usize, size);
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

impl LinkedListAllocator {
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
