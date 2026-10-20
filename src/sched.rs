//! Cooperative round-robin scheduler. Tasks switch only when they call
//! `yield_now`, `sleep` or `exit`; the timer IRQ does not preempt them.
//! Task 0 is the boot context (the shell loop).

use crate::pic;
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::arch::global_asm;
use x86_64::instructions::interrupts;

const STACK_SIZE: usize = 32 * 1024;

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
    #[allow(dead_code)] // owns the stack memory
    stack: Vec<u8>,
    entry: Option<Box<dyn FnOnce() + Send>>,
}

struct Sched {
    tasks: Vec<Box<Task>>,
    current: u64,
    next_id: u64,
}

static mut SCHED: Option<Sched> = None;

fn sched() -> &'static mut Sched {
    unsafe { (*core::ptr::addr_of_mut!(SCHED)).as_mut().expect("scheduler not initialised") }
}

/// Registers the calling context as task 0. Requires the heap.
pub fn init(name: &'static str) {
    let main = Box::new(Task {
        id: 0,
        name,
        state: State::Running,
        rsp: 0, // filled in at the first switch away
        stack: Vec::new(),
        entry: None,
    });
    unsafe {
        *core::ptr::addr_of_mut!(SCHED) =
            Some(Sched { tasks: alloc::vec![main], current: 0, next_id: 1 });
    }
}

/// Creates a task that runs `f` and exits when it returns. Returns its id.
pub fn spawn<F: FnOnce() + Send + 'static>(name: &'static str, f: F) -> u64 {
    let s = sched();
    let id = s.next_id;
    s.next_id += 1;

    let mut stack = alloc::vec![0u8; STACK_SIZE];
    let top = (stack.as_mut_ptr() as u64 + STACK_SIZE as u64) & !15;
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
        stack,
        entry: Some(Box::new(f)),
    }));
    id
}

fn current_index(s: &Sched) -> usize {
    s.tasks.iter().position(|t| t.id == s.current).expect("current task missing")
}

/// Frees tasks that have exited. Never called on a dead task's own stack.
fn reap() {
    let s = sched();
    let cur = s.current;
    s.tasks.retain(|t| t.state != State::Dead || t.id == cur);
}

extern "C" fn task_entry() -> ! {
    interrupts::enable(); // we arrive here from inside schedule(), IF = 0
    reap();
    let s = sched();
    let idx = current_index(s);
    if let Some(f) = s.tasks[idx].entry.take() {
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
        unsafe { switch_context(old, new) };
        // Resumed (possibly much later) on this task's stack.
        reap();
    });
}

/// Gives other ready tasks a turn.
pub fn yield_now() {
    schedule();
}

/// Sleeps for `ticks` timer ticks (10 ms each), letting other tasks run.
pub fn sleep(ticks: u64) {
    let wake = pic::ticks() + ticks;
    while pic::ticks() < wake {
        let s = sched();
        let idx = current_index(s);
        s.tasks[idx].state = State::Sleeping(wake);
        schedule();
        if pic::ticks() < wake {
            x86_64::instructions::hlt(); // nothing runnable: wait for an IRQ
        }
    }
}

/// Terminates the current task.
pub fn exit() -> ! {
    let s = sched();
    let idx = current_index(s);
    s.tasks[idx].state = State::Dead;
    loop {
        schedule();
    }
}

/// Number of tasks (including the boot task) that have not exited.
pub fn live_tasks() -> usize {
    sched().tasks.iter().filter(|t| t.state != State::Dead).count()
}

/// Calls `f(id, name, state)` for every live task.
pub fn for_each(mut f: impl FnMut(u64, &str, &str)) {
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
