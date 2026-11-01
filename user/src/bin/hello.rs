#![no_std]
#![no_main]

use core::sync::atomic::{AtomicU32, Ordering};

// Initialised data: lives in .data, exercises the loader's RW segment.
static ROUNDS: AtomicU32 = AtomicU32::new(3);
// Zero-initialised: lives in .bss.
static DONE: AtomicU32 = AtomicU32::new(0);

#[no_mangle]
pub extern "C" fn _start() -> ! {
    while ROUNDS.load(Ordering::Relaxed) > 0 {
        user::write("hello from an ELF program in ring 3!\n");
        ROUNDS.fetch_sub(1, Ordering::Relaxed);
        DONE.fetch_add(1, Ordering::Relaxed);
        user::sleep(20);
    }
    // 3 rounds ran, bss counter started at zero -> 42.
    user::exit(39 + DONE.load(Ordering::Relaxed) as i64)
}
