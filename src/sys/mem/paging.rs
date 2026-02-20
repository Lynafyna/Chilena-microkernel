//! Paging — Manajemen page table x86_64
//!
//! Menyediakan fungsi untuk map/unmap halaman virtual ke frame fisik.

use super::with_frame_allocator;
use x86_64::registers::control::Cr3;
use x86_64::structures::paging::{
    FrameAllocator, FrameDeallocator,
    Mapper, OffsetPageTable, Page, PageTable,
    PageTableFlags, PhysFrame, Size4KiB,
    mapper::CleanUp,
};
use x86_64::VirtAddr;

/// Ambil pointer ke active page table dari register CR3
pub unsafe fn active_page_table() -> &'static mut PageTable {
    let (frame, _) = Cr3::read();
    let virt = super::phys_to_virt(frame.start_address());
    &mut *virt.as_mut_ptr()
}

/// Buat page table baru dari frame fisik yang sudah dialokasi
pub unsafe fn create_page_table_from_frame(frame: PhysFrame) -> &'static mut PageTable {
    let virt = super::phys_to_virt(frame.start_address());
    &mut *virt.as_mut_ptr()
}

/// Flag untuk halaman user-accessible
const USER_FLAGS: PageTableFlags = PageTableFlags::from_bits_truncate(
    PageTableFlags::PRESENT.bits()
    | PageTableFlags::WRITABLE.bits()
    | PageTableFlags::USER_ACCESSIBLE.bits()
);

/// Alokasi dan map satu atau lebih halaman berurutan mulai dari `addr`
pub fn map_page(mapper: &mut OffsetPageTable, addr: u64, count: usize) -> Result<(), ()> {
    let count = count.saturating_sub(1) as u64;
    let start = Page::containing_address(VirtAddr::new(addr));
    let end   = Page::containing_address(VirtAddr::new(addr + count));
    let range = Page::range_inclusive(start, end);

    with_frame_allocator(|fa| {
        for page in range {
            let frame = fa.allocate_frame().ok_or(())?;
            let result = unsafe { mapper.map_to(page, frame, USER_FLAGS, fa) };
            match result {
                Ok(flush) => flush.flush(),
                Err(_) => return Err(()),
            }
        }
        Ok(())
    })
}

/// Unmap dan bebaskan halaman-halaman dalam range
pub fn unmap_page(mapper: &mut OffsetPageTable, addr: u64, size: usize) {
    let size = size.saturating_sub(1) as u64;
    let start = Page::containing_address(VirtAddr::new(addr));
    let end   = Page::containing_address(VirtAddr::new(addr + size));

    for page in Page::<Size4KiB>::range_inclusive(start, end) {
        if let Ok((frame, flush)) = mapper.unmap(page) {
            flush.flush();
            unsafe {
                with_frame_allocator(|fa| {
                    mapper.clean_up(fa);
                    fa.deallocate_frame(frame);
                });
            }
        }
    }
}
