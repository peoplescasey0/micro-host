//! Ring 3 support: `syscall`/`sysret` entry, user process setup and teardown.
//!
//! A user process is a scheduler task with its own page tables: a private PML4
//! that shares the kernel mappings (supervisor-only) and maps the process's
//! code page and stack at USER_BASE, in a PML4 entry no other process shares.
//! The scheduler loads the task's CR3 whenever it runs. Every process sees
//! the same virtual layout but different physical memory.
//!
//! Syscall ABI: number in rax, arguments in rdi, rsi, rdx; result in rax.
//! Only rbx, rbp, r12-r15 are preserved (rcx/r11 are clobbered by `syscall`).
//!   0 write(ptr, len)   print to the console, returns bytes written or -1
//!   1 exit(code)        never returns
//!   2 getpid()          task id
//!   3 sleep(ticks)      10 ms ticks
//!   4 yield()

use crate::sync::{Mutex, NoPreempt};
use crate::{gdt, kprint, kprintln, paging, sched};
use alloc::string::String;
use alloc::vec::Vec;
use core::arch::{asm, global_asm};
use core::sync::atomic::{AtomicUsize, Ordering};
use x86_64::registers::model_specific::{Efer, EferFlags, LStar, SFMask, Star};
use x86_64::registers::rflags::RFlags;
use x86_64::structures::paging::PageTableFlags;
use x86_64::VirtAddr;

pub const USER_BASE: u64 = 0x80_0000_0000; // PML4 entry 1 (entry 0 = kernel)
const REGION_SIZE: u64 = 1 << 20;
const CODE_OFFSET: u64 = 0;
const STACK_GUARD_OFFSET: u64 = 0x7_0000; // unmapped
const STACK_PAGES: u64 = 4;
const STACK_TOP_OFFSET: u64 = STACK_GUARD_OFFSET + 4096 + STACK_PAGES * 4096;

pub const SYS_WRITE: u64 = 0;
pub const SYS_EXIT: u64 = 1;
pub const SYS_GETPID: u64 = 2;
pub const SYS_SLEEP: u64 = 3;
pub const SYS_YIELD: u64 = 4;

/// Number of processes killed by the kernel after a fault.
pub static KILLED: AtomicUsize = AtomicUsize::new(0);

static EXITS: Mutex<Vec<(u64, i64)>> = Mutex::new(Vec::new()); // (task id, exit code)
static PROCS: Mutex<Vec<(u64, u64)>> = Mutex::new(Vec::new()); // (task id, pml4)

/// Exit code recorded by `exit` for the given task id, if it has exited.
pub fn exit_code(task: u64) -> Option<i64> {
    EXITS.lock().iter().find(|e| e.0 == task).map(|e| e.1)
}

#[no_mangle]
static mut SYSCALL_SCRATCH: u64 = 0;

global_asm!(
    ".section .text",
    ".global syscall_entry",
    "syscall_entry:",
    // IF is cleared by SFMASK, so the scratch slot cannot be raced.
    "   mov [rip + SYSCALL_SCRATCH], rsp",
    "   mov rsp, [rip + SYSCALL_KSTACK]",
    "   push qword ptr [rip + SYSCALL_SCRATCH]", // user rsp
    "   push rcx",                               // user rip
    "   push r11",                               // user rflags
    "   sub rsp, 8",                             // keep rsp 16-byte aligned
    "   mov rcx, rax",                           // 4th C argument = syscall number
    "   sti",
    "   call syscall_dispatch",
    "   cli",
    "   add rsp, 8",
    "   pop r11",
    "   pop rcx",
    "   pop rsp",
    "   sysretq",
);

// Tiny user programs, position independent, placed in .rodata and copied
// into user pages at spawn time.
global_asm!(
    ".section .rodata.user,\"a\"",
    ".global user_hello_start",
    ".global user_hello_end",
    "user_hello_start:",
    "   mov r12d, 3",
    ".Lhello_loop:",
    "   xor eax, eax",
    "   lea rdi, [rip + .Lhello_msg]",
    "   mov esi, 19",
    "   syscall",
    "   mov eax, 3",
    "   mov edi, 20",
    "   syscall",
    "   dec r12d",
    "   jnz .Lhello_loop",
    "   mov eax, 1",
    "   mov edi, 42",
    "   syscall",
    ".Lhello_msg:",
    "   .ascii \"hello from ring 3!\\n\"",
    "user_hello_end:",
    ".global user_evil_start",
    ".global user_evil_end",
    "user_evil_start:",
    "   xor eax, eax",
    "   mov edi, 0x100000",
    "   mov esi, 16",
    "   syscall",
    "   cmp rax, -1",
    "   jne .Levil_bad",
    "   xor eax, eax",
    "   lea rdi, [rip + .Levil_msg]",
    "   mov esi, 49",
    "   syscall",
    "   mov rax, qword ptr [0x100000]",
    ".Levil_bad:",
    "   mov eax, 1",
    "   mov edi, 1",
    "   syscall",
    ".Levil_msg:",
    "   .ascii \"bad pointer rejected; now touching kernel memory\\n\"",
    "user_evil_end:",
    // Writes a marker to the top of its stack, sleeps, checks it survived.
    ".global user_poke_start",
    ".global user_poke_end",
    "user_poke_start:",
    "   lea rbx, [rsp - 8]",
    "   movabs r12, 0x1122334455667788",
    "   mov [rbx], r12",
    "   mov eax, 3",
    "   mov edi, 50",
    "   syscall",
    "   cmp [rbx], r12",
    "   mov edi, 7",
    "   je .Lpoke_done",
    "   mov edi, 99",
    ".Lpoke_done:",
    "   mov eax, 1",
    "   syscall",
    "user_poke_end:",
    // Reads the same virtual address while poke sleeps: must see zero.
    ".global user_peek_start",
    ".global user_peek_end",
    "user_peek_start:",
    "   lea rbx, [rsp - 8]",
    "   mov eax, 3",
    "   mov edi, 10",
    "   syscall",
    "   mov rax, [rbx]",
    "   test rax, rax",
    "   mov edi, 5",
    "   jz .Lpeek_done",
    "   mov edi, 66",
    ".Lpeek_done:",
    "   mov eax, 1",
    "   syscall",
    "user_peek_end:",
    ".section .text",
);

extern "C" {
    fn syscall_entry();
    static user_hello_start: u8;
    static user_hello_end: u8;
    static user_evil_start: u8;
    static user_evil_end: u8;
    static user_poke_start: u8;
    static user_poke_end: u8;
    static user_peek_start: u8;
    static user_peek_end: u8;
}

fn slice(start: &'static u8, end: &'static u8) -> &'static [u8] {
    let s = start as *const u8 as usize;
    let e = end as *const u8 as usize;
    unsafe { core::slice::from_raw_parts(s as *const u8, e - s) }
}

/// Writes a marker to its stack, sleeps 50 ticks, exits 7 if it is intact.
pub fn prog_poke() -> &'static [u8] {
    unsafe { slice(&user_poke_start, &user_poke_end) }
}

/// Reads poke's marker address from another address space: exits 5 if it
/// sees zero (isolated), 66 if it sees poke's data.
pub fn prog_peek() -> &'static [u8] {
    unsafe { slice(&user_peek_start, &user_peek_end) }
}

/// Prints three times, sleeping between, then exits with code 42.
pub fn prog_hello() -> &'static [u8] {
    unsafe { slice(&user_hello_start, &user_hello_end) }
}

/// Passes a kernel pointer to `write` (must be rejected), then reads kernel
/// memory directly (must fault and get the process killed).
pub fn prog_evil() -> &'static [u8] {
    unsafe { slice(&user_evil_start, &user_evil_end) }
}

/// Enables `syscall`/`sysret`. Requires gdt::init and a working heap.
pub fn init() {
    let sel = gdt::selectors();
    unsafe {
        Efer::update(|f| *f |= EferFlags::SYSTEM_CALL_EXTENSIONS);
        Star::write(sel.user_code, sel.user_data, sel.code, sel.data).expect("STAR");
        LStar::write(VirtAddr::new(syscall_entry as usize as u64));
        SFMask::write(RFlags::INTERRUPT_FLAG | RFlags::DIRECTION_FLAG | RFlags::TRAP_FLAG);
    }
}

fn is_user_task() -> bool {
    let id = sched::current_id();
    PROCS.lock().iter().any(|p| p.0 == id)
}

/// Spawns a user process running the machine code in `code` (at most 4 KiB,
/// position independent, entered at offset 0). Returns the task id.
pub fn spawn(name: &'static str, code: &'static [u8]) -> u64 {
    assert!(code.len() <= 4096);
    sched::spawn(name, move || run(code))
}

fn run(code: &'static [u8]) -> ! {
    let base = USER_BASE;
    let pml4 = paging::new_user_space();
    PROCS.lock().push((sched::current_id(), pml4));
    sched::set_current_cr3(pml4); // from here on we run in our own address space

    let user = PageTableFlags::USER_ACCESSIBLE;
    let code_va = base + CODE_OFFSET;
    paging::map_new_page(code_va, PageTableFlags::WRITABLE | user).expect("map user code");
    unsafe { core::ptr::copy_nonoverlapping(code.as_ptr(), code_va as *mut u8, code.len()) };
    // Final permissions: read + execute, not writable.
    assert!(paging::set_page_flags(code_va, user));

    let stack_base = base + STACK_GUARD_OFFSET + 4096;
    for i in 0..STACK_PAGES {
        paging::map_new_page(
            stack_base + i * 4096,
            PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE | user,
        )
        .expect("map user stack");
    }
    let sel = gdt::selectors();
    let ss = sel.user_data.0 as u64;
    let cs = sel.user_code.0 as u64;
    let rsp = base + STACK_TOP_OFFSET;
    unsafe {
        asm!(
            "push {ss}",
            "push {rsp}",
            "push 0x202", // IF set
            "push {cs}",
            "push {rip}",
            "iretq",
            ss = in(reg) ss,
            rsp = in(reg) rsp,
            cs = in(reg) cs,
            rip = in(reg) code_va,
            options(noreturn)
        );
    }
}

/// Frees the current process's whole address space.
fn teardown() {
    let id = sched::current_id();
    let pml4 = {
        let mut p = PROCS.lock();
        match p.iter().position(|e| e.0 == id) {
            Some(i) => p.swap_remove(i).1,
            None => return,
        }
    };
    let _np = NoPreempt::new();
    sched::set_current_cr3(0); // back on the kernel's page tables
    paging::destroy_user_space(pml4);
}

/// Terminates the current process after a fault in ring 3.
pub fn kill_current(reason: &str, rip: u64) -> ! {
    kprintln!("[user] task {} killed: {} at rip {:#x}", sched::current_id(), reason, rip);
    KILLED.fetch_add(1, Ordering::SeqCst);
    teardown();
    sched::exit()
}

fn exit_current(code: i64) -> ! {
    EXITS.lock().push((sched::current_id(), code));
    teardown();
    sched::exit()
}

/// Checks that [ptr, ptr+len) lies in the user region on USER pages of the
/// caller's own address space.
fn user_range_ok(ptr: u64, len: u64) -> bool {
    if !is_user_task() {
        return false;
    }
    let Some(end) = ptr.checked_add(len) else { return false };
    if ptr < USER_BASE || end > USER_BASE + REGION_SIZE {
        return false;
    }
    let mut page = ptr & !0xfff;
    while page < end {
        match paging::page_flags(page) {
            Some(f) if f.contains(PageTableFlags::USER_ACCESSIBLE) => {}
            _ => return false,
        }
        page += 4096;
    }
    true
}

#[no_mangle]
extern "C" fn syscall_dispatch(a1: u64, a2: u64, _a3: u64, nr: u64) -> i64 {
    match nr {
        SYS_WRITE => {
            if a2 > 4096 || !user_range_ok(a1, a2) {
                return -1;
            }
            let bytes = unsafe { core::slice::from_raw_parts(a1 as *const u8, a2 as usize) };
            let text: String = bytes
                .iter()
                .map(|&b| if b == b'\n' || (0x20..0x7f).contains(&b) { b as char } else { '?' })
                .collect();
            kprint!("{}", text);
            a2 as i64
        }
        SYS_EXIT => exit_current(a1 as i64),
        SYS_GETPID => sched::current_id() as i64,
        SYS_SLEEP => {
            sched::sleep(a1.min(6000));
            0
        }
        SYS_YIELD => {
            sched::yield_now();
            0
        }
        _ => -1,
    }
}
