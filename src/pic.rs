//! 8259 PIC (remapped to vectors 32..48), PIT timer (100 Hz) and keyboard IRQ state.

use core::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use x86_64::instructions::port::Port;

pub const PIC1_OFFSET: u8 = 32;
pub const PIC2_OFFSET: u8 = PIC1_OFFSET + 8;
pub const TIMER_VECTOR: u8 = PIC1_OFFSET; // IRQ0
pub const KEYBOARD_VECTOR: u8 = PIC1_OFFSET + 1; // IRQ1

pub const TIMER_HZ: u64 = 100;
const PIT_BASE_HZ: u64 = 1_193_182;

const PIC1_CMD: u16 = 0x20;
const PIC1_DATA: u16 = 0x21;
const PIC2_CMD: u16 = 0xA0;
const PIC2_DATA: u16 = 0xA1;
const EOI: u8 = 0x20;

static TICKS: AtomicU64 = AtomicU64::new(0);
static KEY_COUNT: AtomicU64 = AtomicU64::new(0);
static LAST_SCANCODE: AtomicU8 = AtomicU8::new(0);

unsafe fn outb(port: u16, val: u8) {
    Port::<u8>::new(port).write(val);
}

unsafe fn io_wait() {
    outb(0x80, 0); // unused port; gives old PICs time to settle
}

/// Remaps the PICs and unmasks only IRQ0 (timer) and IRQ1 (keyboard).
/// Interrupts stay disabled; call `x86_64::instructions::interrupts::enable()` after.
pub fn init() {
    unsafe {
        // ICW1: begin init, expect ICW4
        outb(PIC1_CMD, 0x11);
        io_wait();
        outb(PIC2_CMD, 0x11);
        io_wait();
        // ICW2: vector offsets
        outb(PIC1_DATA, PIC1_OFFSET);
        io_wait();
        outb(PIC2_DATA, PIC2_OFFSET);
        io_wait();
        // ICW3: master has slave on IRQ2; slave cascade identity 2
        outb(PIC1_DATA, 0b0000_0100);
        io_wait();
        outb(PIC2_DATA, 2);
        io_wait();
        // ICW4: 8086 mode
        outb(PIC1_DATA, 0x01);
        io_wait();
        outb(PIC2_DATA, 0x01);
        io_wait();
        // Masks: 1 = disabled
        outb(PIC1_DATA, 0b1111_1100);
        outb(PIC2_DATA, 0xFF);

        // PIT channel 0, lobyte/hibyte, mode 3 (square wave)
        let divisor = (PIT_BASE_HZ / TIMER_HZ) as u16;
        outb(0x43, 0x36);
        outb(0x40, (divisor & 0xFF) as u8);
        outb(0x40, (divisor >> 8) as u8);
    }
}

/// Signal end-of-interrupt for an IRQ vector (32..48).
pub fn eoi(vector: u8) {
    unsafe {
        if vector >= PIC2_OFFSET {
            outb(PIC2_CMD, EOI);
        }
        outb(PIC1_CMD, EOI);
    }
}

pub fn on_timer() {
    TICKS.fetch_add(1, Ordering::Relaxed);
}

pub fn on_keyboard() {
    let sc = unsafe { Port::<u8>::new(0x60).read() };
    crate::keyboard::push(sc);
    LAST_SCANCODE.store(sc, Ordering::Relaxed);
    KEY_COUNT.fetch_add(1, Ordering::Relaxed);
}

pub fn ticks() -> u64 {
    TICKS.load(Ordering::Relaxed)
}

