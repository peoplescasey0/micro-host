//! IDT with CPU exception handlers.

use crate::{gdt, pic, serial_println};
use spin::Lazy;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};

static IDT: Lazy<InterruptDescriptorTable> = Lazy::new(|| {
    let mut idt = InterruptDescriptorTable::new();
    idt[pic::TIMER_VECTOR].set_handler_fn(timer_irq);
    idt[pic::KEYBOARD_VECTOR].set_handler_fn(keyboard_irq);
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

// IRQ handlers must not print: the main code may hold the serial lock.
extern "x86-interrupt" fn timer_irq(_frame: InterruptStackFrame) {
    pic::on_timer();
    pic::eoi(pic::TIMER_VECTOR);
    crate::sched::on_tick(); // may switch tasks; EOI must already be sent
}

extern "x86-interrupt" fn keyboard_irq(_frame: InterruptStackFrame) {
    pic::on_keyboard();
    pic::eoi(pic::KEYBOARD_VECTOR);
}

fn from_user(frame: &InterruptStackFrame) -> bool {
    frame.code_segment.rpl() == x86_64::PrivilegeLevel::Ring3
}

extern "x86-interrupt" fn divide_error(frame: InterruptStackFrame) {
    if from_user(&frame) {
        crate::usermode::kill_current("divide error", frame.instruction_pointer.as_u64());
    }
    serial_println!("EXCEPTION: DIVIDE ERROR\n{:#?}", frame);
    halt();
}

extern "x86-interrupt" fn breakpoint(frame: InterruptStackFrame) {
    serial_println!("EXCEPTION: BREAKPOINT (recovered)\n{:#?}", frame);
}

extern "x86-interrupt" fn invalid_opcode(frame: InterruptStackFrame) {
    if from_user(&frame) {
        crate::usermode::kill_current("invalid opcode", frame.instruction_pointer.as_u64());
    }
    serial_println!("EXCEPTION: INVALID OPCODE\n{:#?}", frame);
    halt();
}

extern "x86-interrupt" fn general_protection(frame: InterruptStackFrame, code: u64) {
    if from_user(&frame) {
        crate::usermode::kill_current("general protection fault", frame.instruction_pointer.as_u64());
    }
    serial_println!("EXCEPTION: GENERAL PROTECTION FAULT (code {})\n{:#?}", code, frame);
    halt();
}

extern "x86-interrupt" fn page_fault(frame: InterruptStackFrame, code: PageFaultErrorCode) {
    let addr = x86_64::registers::control::Cr2::read();
    if from_user(&frame) {
        crate::kprintln!("[user] page fault at {:?} ({:?})", addr, code);
        crate::usermode::kill_current("page fault", frame.instruction_pointer.as_u64());
    }
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
