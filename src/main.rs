// POST CARD SPATIAL OS - MULTIBOOT2 KERNEL
// Boots via GRUB using multiboot2 protocol

#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

extern crate alloc;

mod gdt;
mod heap;
mod idt;
mod memory;
mod paging;
mod pic;

use core::panic::PanicInfo;
use core::arch::global_asm;

// The multiboot2 header MUST be in the first 32KB of the file.
// We place it in .text.boot which the linker puts at the very start.
// The header is followed immediately by the 64-bit entry stub.
global_asm!(
    // Put everything in .text.boot - the linker script places this FIRST
    ".section .text.boot, \"ax\"",

    // ── Multiboot2 header ──────────────────────────────────────────
    ".align 8",
    "multiboot2_start:",
    "   .long 0xe85250d6",          // magic
    "   .long 0",                   // architecture: 0 = i386 protected mode
    "   .long multiboot2_end - multiboot2_start", // header length
    // checksum: magic + arch + length + checksum must equal 0 mod 2^32
    "   .long -(0xe85250d6 + 0 + (multiboot2_end - multiboot2_start))",
    // end tag
    "   .short 0",                  // type
    "   .short 0",                  // flags
    "   .long 8",                   // size
    "multiboot2_end:",

    // ── Entry stub ────────────────────────────────────────────────
    // GRUB hands off in 32-bit protected mode; we need to switch to 64-bit.
    ".code32",
    ".global entry32",
    "entry32:",
    "   cli",
    // Build an identity map for the first GiB using 2 MiB pages.
    "   mov eax, offset boot_pdpt",
    "   or eax, 0x3",
    "   mov dword ptr [boot_pml4], eax",
    "   mov eax, offset boot_pd",
    "   or eax, 0x3",
    "   mov dword ptr [boot_pdpt], eax",
    "   xor ecx, ecx",
    "2:",
    "   mov eax, ecx",
    "   shl eax, 21",
    "   or eax, 0x83",
    "   mov dword ptr [boot_pd + ecx * 8], eax",
    "   mov dword ptr [boot_pd + ecx * 8 + 4], 0",
    "   inc ecx",
    "   cmp ecx, 512",
    "   jne 2b",
    // Enable PAE, long mode, and paging before loading the 64-bit GDT.
    "   mov eax, cr4",
    "   or eax, 0x20",
    "   mov cr4, eax",
    "   mov eax, offset boot_pml4",
    "   mov cr3, eax",
    "   mov ecx, 0xC0000080",
    "   rdmsr",
    "   or eax, 0x100",
    "   wrmsr",
    "   lgdt [boot_gdt_ptr]",
    "   mov eax, cr0",
    "   or eax, 0x80000000",
    "   mov cr0, eax",
    "   .byte 0xea",
    "   .long long_mode_start",
    "   .word 0x08",
    ".code64",
    "long_mode_start:",
    "   mov ax, 0x10",
    "   mov ds, ax",
    "   mov es, ax",
    "   mov ss, ax",
    "   lea rsp, [rip + boot_stack_top]",
    "   and rsp, -16",
    "   xor rbp, rbp",
    // EBX still holds the multiboot2 info pointer from GRUB; pass it as arg 1.
    "   mov edi, ebx",
    "   call _start",
    "1: hlt",
    "   jmp 1b",

    ".section .rodata",
    ".align 8",
    "boot_gdt:",
    "   .quad 0",
    "   .quad 0x00af9a000000ffff",
    "   .quad 0x00cf92000000ffff",
    "boot_gdt_end:",
    "boot_gdt_ptr:",
    "   .word boot_gdt_end - boot_gdt - 1",
    "   .long boot_gdt",

    ".section .bss",
    ".align 4096",
    "boot_pml4:",
    "   .skip 4096",
    "boot_pdpt:",
    "   .skip 4096",
    "boot_pd:",
    "   .skip 4096",
    ".align 16",
    "boot_stack:",
    "   .skip 65536",
    "boot_stack_top:",
);
// Minimal serial implementation
mod serial {
    use uart_16550::SerialPort;
    use spin::Mutex;
    use core::fmt;

    pub static SERIAL1: Mutex<SerialPort> = Mutex::new(unsafe { SerialPort::new(0x3F8) });

    pub fn init() {
        SERIAL1.lock().init();
    }

    #[doc(hidden)]
    pub fn _print(args: fmt::Arguments) {
        use core::fmt::Write;
        SERIAL1.lock().write_fmt(args).expect("Printing to serial failed");
    }

    #[macro_export]
    macro_rules! serial_print {
        ($($arg:tt)*) => {
            $crate::serial::_print(format_args!($($arg)*))
        };
    }

    #[macro_export]
    macro_rules! serial_println {
        () => ($crate::serial_print!("\n"));
        ($fmt:expr) => ($crate::serial_print!(concat!($fmt, "\n")));
        ($fmt:expr, $($arg:tt)*) => ($crate::serial_print!(concat!($fmt, "\n"), $($arg)*));
    }
}

// Entry point - called from the 32-bit stub above
#[no_mangle]
pub extern "C" fn _start(mb_info: usize) -> ! {
    serial::init();

    serial_println!("POST CARD SPATIAL OS - BOOTED!");
    serial_println!("================================");
    serial_println!("Kernel: multiboot2 via GRUB, 64-bit long mode");
    serial_println!("");

    gdt::init();
    serial_println!("[ok] GDT + TSS loaded");
    idt::init();
    serial_println!("[ok] IDT loaded");

    // Prove exceptions work: breakpoint must be handled and execution resume.
    x86_64::instructions::interrupts::int3();
    serial_println!("[ok] breakpoint exception handled, execution resumed");

    serial_println!("[..] multiboot2 info at {:#x}", mb_info);
    serial_println!("[..] kernel image ends at {:#x}", memory::kernel_end_addr());
    let total = unsafe { memory::report(mb_info) };
    serial_println!("[ok] {} MiB usable RAM", total / (1024 * 1024));

    match unsafe { heap::init(mb_info) } {
        Some((start, size)) => {
            serial_println!("[ok] heap: {:#x}, {} KiB", start, size / 1024);
            heap_selftest();
            let n = unsafe { paging::init_frames(mb_info, start + size) };
            serial_println!("[ok] frame allocator: {} frames ({} MiB) above heap", n, n * 4 / 1024);
            paging_selftest();
        }
        None => serial_println!("[!!] no suitable region for heap"),
    }

    pic::init();
    serial_println!("[ok] PIC remapped to vectors 32-47, PIT at {} Hz", pic::TIMER_HZ);
    x86_64::instructions::interrupts::enable();
    while pic::ticks() < pic::TIMER_HZ {
        x86_64::instructions::hlt();
    }
    serial_println!("[ok] timer IRQ: {} ticks received (~1 second)", pic::ticks());

    serial_println!("");
    serial_println!("Spatial OS is ALIVE! (press keys in the QEMU window to see scancodes)");

    let mut seen_keys = 0;
    loop {
        x86_64::instructions::hlt();
        let n = pic::key_count();
        if n != seen_keys {
            seen_keys = n;
            serial_println!("key scancode: {:#04x}", pic::last_scancode());
        }
    }
}

fn paging_selftest() {
    use x86_64::structures::paging::PageTableFlags as F;
    const A: u64 = 0x4000_0000; // 1 GiB: outside the boot identity map
    const B: u64 = 0x4000_1000;
    let rw = F::WRITABLE;

    let before = paging::frames_in_use();
    assert!(paging::translate(A).is_none(), "A already mapped");
    let pa = paging::map_new_page(A, rw).expect("map A");
    let pb = paging::map_new_page(B, rw).expect("map B");
    assert_eq!(paging::translate(A), Some(pa));
    assert_eq!(paging::translate(B), Some(pb));
    unsafe {
        (A as *mut u64).write_volatile(0xDEAD_BEEF_CAFE_F00D);
        (B as *mut u64).write_volatile(0x1122_3344_5566_7788);
        assert_eq!((A as *const u64).read_volatile(), 0xDEAD_BEEF_CAFE_F00D);
        assert_eq!((B as *const u64).read_volatile(), 0x1122_3344_5566_7788);
        // Same data visible through the identity mapping of the frame.
        assert_eq!((pa.as_u64() as *const u64).read_volatile(), 0xDEAD_BEEF_CAFE_F00D);
    }
    serial_println!("[ok] paging: {:#x} -> {:#x}, {:#x} -> {:#x}, read/write verified",
        A, pa.as_u64(), B, pb.as_u64());

    assert!(paging::unmap_page(A));
    assert!(paging::unmap_page(B));
    assert!(paging::translate(A).is_none());
    let pc = paging::map_new_page(A, rw).expect("remap A");
    assert!(pc == pb || pc == pa, "freed frame not reused");
    unsafe { assert_eq!((A as *const u64).read_volatile(), 0, "frame not zeroed") };
    assert!(paging::unmap_page(A));
    serial_println!("[ok] unmap + frame reuse + zeroing verified ({} frames in use, {} for page tables)",
        paging::frames_in_use(), paging::frames_in_use() - before);
}

fn heap_selftest() {
    use alloc::{boxed::Box, string::String, vec::Vec};

    let boxed = Box::new(0xC0FFEEu32);
    let mut v: Vec<u64> = Vec::new();
    for i in 0..1000 {
        v.push(i * i);
    }
    let sum: u64 = v.iter().sum();
    let mut s = String::from("heap");
    s.push_str(" works");

    assert_eq!(*boxed, 0xC0FFEE);
    assert_eq!(sum, 332_833_500);
    serial_println!("[ok] heap selftest: Box, Vec(1000, sum={}), String(\"{}\")", sum, s);
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    serial_println!("KERNEL PANIC: {}", info);
    loop {}
}
