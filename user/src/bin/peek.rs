#![no_std]
#![no_main]

use core::sync::atomic::{AtomicU64, Ordering};

// Must be at the same address as poke's MARKER: both are the only .bss item.
#[no_mangle]
pub static MARKER: AtomicU64 = AtomicU64::new(0);

#[no_mangle]
pub extern "C" fn _start() -> ! {
    user::sleep(10); // poke has written its marker by now
    user::exit(if MARKER.load(Ordering::SeqCst) == 0 { 5 } else { 66 })
}
