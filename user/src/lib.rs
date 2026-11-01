//! Minimal runtime for user programs: raw syscalls and a panic handler.
#![no_std]

use core::arch::asm;

#[inline(always)]
unsafe fn syscall3(nr: u64, a: u64, b: u64, c: u64) -> i64 {
    let ret: i64;
    asm!(
        "syscall",
        inlateout("rax") nr as i64 => ret,
        in("rdi") a, in("rsi") b, in("rdx") c,
        lateout("rcx") _, lateout("r11") _,
        options(nostack)
    );
    ret
}

/// Writes to the console. Returns bytes written or -1.
pub fn write(s: &str) -> i64 {
    unsafe { syscall3(0, s.as_ptr() as u64, s.len() as u64, 0) }
}

pub fn write_raw(ptr: u64, len: u64) -> i64 {
    unsafe { syscall3(0, ptr, len, 0) }
}

pub fn exit(code: i64) -> ! {
    unsafe { syscall3(1, code as u64, 0, 0) };
    loop {}
}

pub fn getpid() -> u64 {
    unsafe { syscall3(2, 0, 0, 0) as u64 }
}

/// Sleeps for `ticks` 10 ms ticks.
pub fn sleep(ticks: u64) {
    unsafe { syscall3(3, ticks, 0, 0) };
}

pub fn yield_now() {
    unsafe { syscall3(4, 0, 0, 0) };
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    write("user panic\n");
    exit(101)
}
