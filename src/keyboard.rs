//! PS/2 keyboard: lock-free scancode queue (filled by the IRQ handler) and a
//! scancode set 1 decoder for a US layout.

use core::sync::atomic::{AtomicUsize, Ordering};
use core::sync::atomic::AtomicU8;

const QUEUE_SIZE: usize = 128;
static QUEUE: [AtomicU8; QUEUE_SIZE] = [const { AtomicU8::new(0) }; QUEUE_SIZE];
static HEAD: AtomicUsize = AtomicUsize::new(0); // written by the consumer
static TAIL: AtomicUsize = AtomicUsize::new(0); // written by the IRQ handler

/// Called from the keyboard IRQ. Drops the scancode if the queue is full.
pub fn push(sc: u8) {
    let tail = TAIL.load(Ordering::Relaxed);
    let next = (tail + 1) % QUEUE_SIZE;
    if next == HEAD.load(Ordering::Acquire) {
        return;
    }
    QUEUE[tail].store(sc, Ordering::Relaxed);
    TAIL.store(next, Ordering::Release);
}

/// Pops the oldest queued scancode (single consumer).
pub fn pop() -> Option<u8> {
    let head = HEAD.load(Ordering::Relaxed);
    if head == TAIL.load(Ordering::Acquire) {
        return None;
    }
    let sc = QUEUE[head].load(Ordering::Relaxed);
    HEAD.store((head + 1) % QUEUE_SIZE, Ordering::Release);
    Some(sc)
}

const NORMAL: &[u8; 58] = b"\x00\x001234567890-=\x08\tqwertyuiop[]\n\0asdfghjkl;'`\0\\zxcvbnm,./\0*\0 ";
const SHIFTED: &[u8; 58] = b"\0\0!@#$%^&*()_+\x08\tQWERTYUIOP{}\n\0ASDFGHJKL:\"~\0|ZXCVBNM<>?\0*\0 ";

/// Scancode set 1 decoder. Handles shift, caps lock; ignores extended keys.
pub struct Decoder {
    shift: bool,
    caps: bool,
    extended: bool,
}

impl Decoder {
    pub const fn new() -> Self {
        Decoder { shift: false, caps: false, extended: false }
    }

    /// Feeds one raw scancode; returns a character for key presses that
    /// produce one ('\n' for Enter, '\x08' for Backspace, '\t' for Tab).
    pub fn feed(&mut self, sc: u8) -> Option<char> {
        if sc == 0xE0 {
            self.extended = true;
            return None;
        }
        if self.extended {
            self.extended = false; // arrows, delete, etc. not handled yet
            return None;
        }
        let release = sc & 0x80 != 0;
        let code = sc & 0x7F;
        match code {
            0x2A | 0x36 => {
                self.shift = !release;
                return None;
            }
            0x3A => {
                if !release {
                    self.caps = !self.caps;
                }
                return None;
            }
            _ => {}
        }
        if release || code as usize >= NORMAL.len() {
            return None;
        }
        let base = NORMAL[code as usize];
        let table = if base.is_ascii_lowercase() { self.shift ^ self.caps } else { self.shift };
        let c = if table { SHIFTED[code as usize] } else { base };
        if c == 0 { None } else { Some(c as char) }
    }
}
