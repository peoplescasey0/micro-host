#![no_std]
#![no_main]

use core::sync::atomic::{AtomicU64, Ordering};

// Same virtual address in every process running this image (see peek).
#[no_mangle]
pub static MARKER: AtomicU64 = AtomicU64::new(0);

#[no_mangle]
pub extern "C" fn _start() -> ! {
    MARKER.store(0x1122_3344_5566_7788, Ordering::SeqCst);
    user::sleep(50);
    user::exit(if MARKER.load(Ordering::SeqCst) == 0x1122_3344_5566_7788 { 7 } else { 99 })
}
