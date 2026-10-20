//! Preemption control. The timer IRQ only switches tasks when no task is
//! inside a "no preempt" section, so a task can never be switched out while
//! it holds a kernel lock (which would deadlock the next task that wants it).

use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicUsize, Ordering};

static DEPTH: AtomicUsize = AtomicUsize::new(0);

/// True if the timer IRQ may switch tasks right now.
pub fn preemptible() -> bool {
    DEPTH.load(Ordering::Acquire) == 0
}

/// RAII guard: preemption is off while it lives. Nests.
pub struct NoPreempt;

impl NoPreempt {
    pub fn new() -> Self {
        DEPTH.fetch_add(1, Ordering::Acquire);
        NoPreempt
    }
}

impl Drop for NoPreempt {
    fn drop(&mut self) {
        DEPTH.fetch_sub(1, Ordering::Release);
    }
}

/// `spin::Mutex` that also disables preemption while locked.
pub struct Mutex<T> {
    inner: spin::Mutex<T>,
}

pub struct MutexGuard<'a, T> {
    guard: spin::MutexGuard<'a, T>, // dropped (unlocked) before `_np`
    _np: NoPreempt,
}

impl<T> Mutex<T> {
    pub const fn new(value: T) -> Self {
        Mutex { inner: spin::Mutex::new(value) }
    }

    pub fn lock(&self) -> MutexGuard<'_, T> {
        let np = NoPreempt::new(); // before locking, so we can't be preempted holding it
        MutexGuard { guard: self.inner.lock(), _np: np }
    }
}

impl<T> Deref for MutexGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.guard
    }
}

impl<T> DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.guard
    }
}
