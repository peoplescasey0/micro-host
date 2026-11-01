#![no_std]
#![no_main]

#[no_mangle]
pub extern "C" fn _start() -> ! {
    if user::write_raw(0x10_0000, 16) != -1 {
        user::exit(1);
    }
    user::write("bad pointer rejected; now touching kernel memory\n");
    let v = unsafe { core::ptr::read_volatile(0x10_0000 as *const u64) };
    user::exit(v as i64 | 1)
}
