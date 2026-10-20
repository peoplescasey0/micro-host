//! Round-robin scheduler. Tasks switch voluntarily (`yield_now`, `sleep`,
//! `exit`) and, once `enable_preemption` has been called, also when the timer
//! IRQ fires outside a no-preempt section (see `sync`).
//! Task 0 is the boot context (the shell loop).
//!
//! Task stacks live in a dedicated virtual region: 8 mapped pages with an
//! unmapped guard page below, so an overflow faults instead of corrupting
//! neighbouring memory.

use crate::sync::{self, NoPreempt};
use crate::{paging, pic};
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::arch::global_asm;
use core::sync::atomic::{AtomicBool, Ordering};
use x86_64::instructions::interrupts;
use x86_64::structures::paging::PageTableFlags;

const STACK_PAGES: u64 = 8; // 32 KiB
const SLOT_SIZE: u64 = 16 * 4096; // guard page + stack + slack
const STACK_REGION: u64 = 0x4800_0000; // outside the identity map, 1 GiB..2 GiB
const MAX_SLOTS: usize = 0x3800_0000 / SLOT_SIZE as usize;

static PREEMPTION: AtomicBool = AtomicBool::new(false);

global_asm!(
    ".section .text",
    ".global switch_context",
    "switch_context:",
    "   push rbp",
    "   push rbx",
    "   push r12",
    "   push r13",
    "   push r14",
    "   push r15",
    "   mov [rdi], rsp",
    "   mov rsp, rsi",
    "   pop r15",
    "   pop r14",
    "   pop r13",
    "   pop r12",
    "   pop rbx",
    "   pop rbp",
    "   ret",
);

extern "C" {
    /// Saves callee-saved registers on the current stack, stores the stack
    /// pointer in `*old_rsp`, then resumes the context saved at `new_rsp`.
    fn switch_context(old_rsp: *mut u64, new_rsp: u64);
}

#[derive(Clone, Copy, PartialEq)]
pub enum State {
    Running,
    Ready,
    Sleeping(u64), // wake-up tick
    Dead,
}

struct Task {
    id: u64,
    name: &'static str,
    state: State,
    rsp: u64,
    slot: Option<usize>, // stack slot; None for the boot task
    entry: Option<Box<dyn FnOnce() + Send>>,
}

struct Sched {
    tasks: Vec<Box<Task>>,
    current: u64,
    next_id: u64,
    free_slots: Vec<usize>,
    next_slot: usize,
}

static mut SCHED: Option<Sched> = None;

fn sched() -> &'static mut Sched {
    unsafe { (*core::ptr::addr_of_mut!(SCHED)).as_mut().expect("scheduler not initialised") }
}

fn slot_base(slot: usize) -> u64 {
    STACK_REGION + slot as u64 * SLOT_SIZE
}

/// Registers the calling context as task 0. Requires the heap.
pub fn init(name: &'static str) {
    let main = Box::new(Task {
        id: 0,
        name,
        state: State::Running,
        rsp: 0, // filled in at the first switch away
        slot: None,
        entry: None,
    });
    unsafe {
        *core::ptr::addr_of_mut!(SCHED) = Some(Sched {
            tasks: alloc::vec![main],
            current: 0,
            next_id: 1,
            free_slots: Vec::new(),
            next_slot: 0,
        });
    }
}

/// Lets the timer IRQ switch tasks.
pub fn enable_preemption() {
    PREEMPTION.store(true, Ordering::SeqCst);
}

/// Called from the timer IRQ handler, after EOI.
pub fn on_tick() {
    if PREEMPTION.load(Ordering::Relaxed) && sync::preemptible() {
        schedule();
    }
}

/// Maps a fresh stack (guard page left unmapped). Returns the slot.
fn alloc_stack(s: &mut Sched) -> usize {
    let slot = s.free_slots.pop().unwrap_or_else(|| {
        let n = s.next_slot;
        assert!(n < MAX_SLOTS, "out of task stack slots");
        s.next_slot += 1;
        n
    });
    let base = slot_base(slot);
    for i in 0..STACK_PAGES {
        paging::map_new_page(
            base + 4096 + i * 4096,
            PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE,
        )
        .expect("map task stack");
    }
    slot
}

fn free_stack(slot: usize) {
    let base = slot_base(slot);
    for i in 0..STACK_PAGES {
        paging::unmap_page(base + 4096 + i * 4096);
    }
}

/// Creates a task that runs `f` and exits when it returns. Returns its id.
pub fn spawn<F: FnOnce() + Send + 'static>(name: &'static str, f: F) -> u64 {
    let _np = NoPreempt::new();
    let s = sched();
    let id = s.next_id;
    s.next_id += 1;

    let slot = alloc_stack(s);
    let top = slot_base(slot) + 4096 + STACK_PAGES * 4096; // 16-byte aligned
    // Layout (low → high): r15 r14 r13 r12 rbx rbp, return address.
    // After `ret` into task_entry, rsp = top - 8 (≡ 8 mod 16, as after a call).
    let rsp = top - 64;
    unsafe {
        for i in 0..6 {
            ((rsp + i * 8) as *mut u64).write(0);
        }
        ((top - 16) as *mut u64).write(task_entry as usize as u64);
    }
    s.tasks.push(Box::new(Task {
        id,
        name,
        state: State::Ready,
        rsp,
        slot: Some(slot),
        entry: Some(Box::new(f)),
    }));
    id
}

fn current_index(s: &Sched) -> usize {
    s.tasks.iter().position(|t| t.id == s.current).expect("current task missing")
}

/// Frees tasks that have exited (never the running one). Interrupts or
/// preemption must be off.
fn reap() {
    let s = sched();
    let cur = s.current;
    let mut i = 0;
    while i < s.tasks.len() {
        if s.tasks[i].state == State::Dead && s.tasks[i].id != cur {
            let t = s.tasks.remove(i);
            if let Some(slot) = t.slot {
                free_stack(slot);
                s.free_slots.push(slot);
            }
        } else {
            i += 1;
        }
    }
}

extern "C" fn task_entry() -> ! {
    // We arrive here from inside schedule() with IF = 0.
    reap();
    let f = {
        let s = sched();
        let idx = current_index(s);
        s.tasks[idx].entry.take()
    };
    interrupts::enable();
    if let Some(f) = f {
        f();
    }
    exit()
}

fn schedule() {
    interrupts::without_interrupts(|| {
        let s = sched();
        let now = pic::ticks();
        let n = s.tasks.len();
        let cur = current_index(s);

        let mut next = None;
        for i in 1..=n {
            let idx = (cur + i) % n;
            let runnable = match s.tasks[idx].state {
                State::Running | State::Ready => true,
                State::Sleeping(w) => w <= now,
                State::Dead => false,
            };
            if runnable {
                next = Some(idx);
                break;
            }
        }
        let Some(next) = next else {
            s.tasks[cur].state = State::Running; // nothing else to run
            return;
        };
        if next == cur {
            s.tasks[cur].state = State::Running;
            return;
        }
        if s.tasks[cur].state == State::Running {
            s.tasks[cur].state = State::Ready;
        }
        s.tasks[next].state = State::Running;
        s.current = s.tasks[next].id;
        let old = &mut s.tasks[cur].rsp as *mut u64;
        let new = s.tasks[next].rsp;
        if let Some(slot) = s.tasks[next].slot {
            // Ring 3 → 0 transitions of the next task land on its own stack.
            crate::gdt::set_kernel_stack(slot_base(slot) + 4096 + STACK_PAGES * 4096);
        }
        unsafe { switch_context(old, new) };
        // Resumed (possibly much later) on this task's stack.
        reap();
    });
}

/// Id of the running task.
pub fn current_id() -> u64 {
    sched().current
}

/// Gives other ready tasks a turn.
pub fn yield_now() {
    schedule();
}

/// Sleeps for `ticks` timer ticks (10 ms each), letting other tasks run.
pub fn sleep(ticks: u64) {
    let wake = pic::ticks() + ticks;
    while pic::ticks() < wake {
        {
            let _np = NoPreempt::new();
            let s = sched();
            let idx = current_index(s);
            s.tasks[idx].state = State::Sleeping(wake);
        }
        schedule();
        if pic::ticks() < wake {
            x86_64::instructions::hlt(); // nothing runnable: wait for an IRQ
        }
    }
}

/// Terminates the current task.
pub fn exit() -> ! {
    {
        let _np = NoPreempt::new();
        let s = sched();
        let idx = current_index(s);
        s.tasks[idx].state = State::Dead;
    }
    loop {
        schedule();
    }
}

/// Number of tasks (including the boot task) that have not exited.
pub fn live_tasks() -> usize {
    let _np = NoPreempt::new();
    sched().tasks.iter().filter(|t| t.state != State::Dead).count()
}

/// Calls `f(id, name, state)` for every live task.
pub fn for_each(mut f: impl FnMut(u64, &str, &str)) {
    let _np = NoPreempt::new();
    for t in sched().tasks.iter() {
        let st = match t.state {
            State::Running => "running",
            State::Ready => "ready",
            State::Sleeping(_) => "sleeping",
            State::Dead => continue,
        };
        f(t.id, t.name, st);
    }
}
