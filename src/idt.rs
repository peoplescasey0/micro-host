//! IDT with CPU exception handlers.

use crate::{gdt, serial_println};
use spin::Lazy;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};

static IDT: Lazy<InterruptDescriptorTable> = Lazy::new(|| {
    let mut idt = InterruptDescriptorTable::new();
    idt.divide_error.set_handler_fn(divide_error);
    idt.breakpoint.set_handler_fn(breakpoint);
    idt.invalid_opcode.set_handler_fn(invalid_opcode);
    idt.general_protection_fault.set_handler_fn(general_protection);
    idt.page_fault.set_handler_fn(page_fault);
    unsafe {
        idt.double_fault
            .set_handler_fn(double_fault)
            .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
    }
    idt
});

pub fn init() {
    IDT.load();
}

fn halt() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}

extern "x86-interrupt" fn divide_error(frame: InterruptStackFrame) {
    serial_println!("EXCEPTION: DIVIDE ERROR\n{:#?}", frame);
    halt();
}

extern "x86-interrupt" fn breakpoint(frame: InterruptStackFrame) {
    serial_println!("EXCEPTION: BREAKPOINT (recovered)\n{:#?}", frame);
}

extern "x86-interrupt" fn invalid_opcode(frame: InterruptStackFrame) {
    serial_println!("EXCEPTION: INVALID OPCODE\n{:#?}", frame);
    halt();
}

extern "x86-interrupt" fn general_protection(frame: InterruptStackFrame, code: u64) {
    serial_println!("EXCEPTION: GENERAL PROTECTION FAULT (code {})\n{:#?}", code, frame);
    halt();
}

extern "x86-interrupt" fn page_fault(frame: InterruptStackFrame, code: PageFaultErrorCode) {
    let addr = x86_64::registers::control::Cr2::read();
    serial_println!(
        "EXCEPTION: PAGE FAULT at {:?} ({:?})\n{:#?}",
        addr,
        code,
        frame
    );
    halt();
}

extern "x86-interrupt" fn double_fault(frame: InterruptStackFrame, _code: u64) -> ! {
    serial_println!("EXCEPTION: DOUBLE FAULT\n{:#?}", frame);
    halt();
}
