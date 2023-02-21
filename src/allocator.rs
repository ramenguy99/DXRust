
use core::alloc::{Allocator, Layout, AllocError};
use core::ptr::NonNull;
use core::cell::Cell;
use std::ptr::null_mut;

pub struct FixedBaseAllocator {
    base: *mut u8,
    used: Cell<u64>,
    size: u64,
}

impl FixedBaseAllocator {
    pub fn new() -> FixedBaseAllocator {
        FixedBaseAllocator {
            base: null_mut(),
            used: Cell::new(0),
            size: 0,
        }
    }
    pub fn init(&mut self, ptr: *mut u8, size: u64) {
        self.base = ptr;
        self.used.set(0);
        self.size = size;
    }
}

unsafe impl Allocator for FixedBaseAllocator {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        unsafe {
            // println!("Alloc: 0x{:16x} (0x{:16x}/0x{:16x})", layout.size(), self.used.get(), self.size);
            assert!(!self.base.is_null());

            let ptr = self.base.offset(self.used.get() as isize);
            let align_delta = ptr.align_offset(layout.align());
            let ptr = ptr.offset(align_delta as isize);

            let alloc_size = (align_delta as u64).checked_add(layout.size() as u64).unwrap();
            let new_used = self.used.get().checked_add(alloc_size).unwrap();
            if new_used > self.size {
                panic!("Out of memory");
            }
            self.used.set(new_used);

            Ok(NonNull::new(core::slice::from_raw_parts_mut(ptr, layout.size())).unwrap())
        }
    }

    unsafe fn deallocate(&self, _ptr: NonNull<u8>, _layout: Layout) {
        // println!("Free {}", layout.size());
    }
}
