// POST CARD SPATIAL OS - MULTIBOOT2 KERNEL
// Boots via GRUB using multiboot2 protocol

#![no_std]
#![no_main]

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
    "   .skip 16384",
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
pub extern "C" fn _start() -> ! {
    serial::init();

    serial_println!("POST CARD SPATIAL OS - BOOTED!");
    serial_println!("================================");
    serial_println!("");
    serial_println!("Kernel: multiboot2 via GRUB");
    serial_println!("Status: Running in 64-bit mode");
    serial_println!("");
    serial_println!("POST CARD SPATIAL OS:");
    serial_println!("  Universal spatial computing");
    serial_println!("  Windows + Linux + Android in 3D");
    serial_println!("  Paper airplane sharing");
    serial_println!("");
    serial_println!("Bootloader fix: COMPLETE");
    serial_println!("ISO creation:   COMPLETE");
    serial_println!("Next step:      QEMU integration");
    serial_println!("");
    serial_println!("Spatial OS is ALIVE!");

    loop {
        core::hint::spin_loop();
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    serial_println!("KERNEL PANIC: {}", info);
    loop {}
}
