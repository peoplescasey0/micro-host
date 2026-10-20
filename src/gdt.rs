//! Kernel GDT + TSS. The TSS provides a dedicated stack for double faults.

use spin::Lazy;
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;
const DF_STACK_SIZE: usize = 4096 * 5;

#[repr(C, align(4096))]
struct Stack {
    guard: [u8; 4096], // left unmapped by paging::remap_kernel
    data: [u8; DF_STACK_SIZE],
}
static mut DF_STACK: Stack = Stack { guard: [0; 4096], data: [0; DF_STACK_SIZE] };

/// Address of the (page-sized) guard below the double-fault stack.
pub fn df_guard_page() -> u64 {
    core::ptr::addr_of!(DF_STACK) as u64
}

static TSS: Lazy<TaskStateSegment> = Lazy::new(|| {
    let mut tss = TaskStateSegment::new();
    tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
        let start = VirtAddr::from_ptr(core::ptr::addr_of!(DF_STACK));
        start + (4096 + DF_STACK_SIZE) as u64
    };
    tss
});

struct Selectors {
    code: SegmentSelector,
    data: SegmentSelector,
    tss: SegmentSelector,
}

static GDT: Lazy<(GlobalDescriptorTable, Selectors)> = Lazy::new(|| {
    let mut gdt = GlobalDescriptorTable::new();
    let code = gdt.append(Descriptor::kernel_code_segment());
    let data = gdt.append(Descriptor::kernel_data_segment());
    let tss = gdt.append(Descriptor::tss_segment(&TSS));
    (gdt, Selectors { code, data, tss })
});

pub fn init() {
    use x86_64::instructions::segmentation::{Segment, CS, DS, ES, SS};
    use x86_64::instructions::tables::load_tss;

    GDT.0.load();
    unsafe {
        CS::set_reg(GDT.1.code);
        DS::set_reg(GDT.1.data);
        ES::set_reg(GDT.1.data);
        SS::set_reg(GDT.1.data);
        load_tss(GDT.1.tss);
    }
}
