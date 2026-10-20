//! Kernel/user GDT + TSS. The TSS provides a dedicated stack for double
//! faults and the ring-0 stack used when an interrupt arrives in user mode.
//!
//! Segment order is dictated by `syscall`/`sysret`:
//! 0x08 kernel code, 0x10 kernel data, 0x18 user data, 0x20 user code.

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

static mut TSS: TaskStateSegment = TaskStateSegment::new();

/// Kernel stack top used by the `syscall` entry stub (see usermode.rs).
#[no_mangle]
pub static mut SYSCALL_KSTACK: u64 = 0;

/// Sets the stack the CPU switches to on ring 3 → ring 0 transitions
/// (interrupts and `syscall`). Called by the scheduler on every task switch.
pub fn set_kernel_stack(top: u64) {
    unsafe {
        (*core::ptr::addr_of_mut!(TSS)).privilege_stack_table[0] = VirtAddr::new(top);
        core::ptr::write_volatile(core::ptr::addr_of_mut!(SYSCALL_KSTACK), top);
    }
}

pub struct Selectors {
    pub code: SegmentSelector,
    pub data: SegmentSelector,
    pub user_data: SegmentSelector,
    pub user_code: SegmentSelector,
    tss: SegmentSelector,
}

static GDT: Lazy<(GlobalDescriptorTable, Selectors)> = Lazy::new(|| {
    let mut gdt = GlobalDescriptorTable::new();
    let code = gdt.append(Descriptor::kernel_code_segment());
    let data = gdt.append(Descriptor::kernel_data_segment());
    let user_data = gdt.append(Descriptor::user_data_segment());
    let user_code = gdt.append(Descriptor::user_code_segment());
    let tss = gdt.append(Descriptor::tss_segment(unsafe { &*core::ptr::addr_of!(TSS) }));
    (gdt, Selectors { code, data, user_data, user_code, tss })
});

pub fn selectors() -> &'static Selectors {
    &GDT.1
}

pub fn init() {
    use x86_64::instructions::segmentation::{Segment, CS, DS, ES, SS};
    use x86_64::instructions::tables::load_tss;

    unsafe {
        let tss = &mut *core::ptr::addr_of_mut!(TSS);
        tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] =
            VirtAddr::from_ptr(core::ptr::addr_of!(DF_STACK)) + (4096 + DF_STACK_SIZE) as u64;
    }
    GDT.0.load();
    unsafe {
        CS::set_reg(GDT.1.code);
        DS::set_reg(GDT.1.data);
        ES::set_reg(GDT.1.data);
        SS::set_reg(GDT.1.data);
        load_tss(GDT.1.tss);
    }
}
