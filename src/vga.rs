//! VGA text mode (80x25) console at 0xB8000.

use core::fmt;
use crate::sync::Mutex;
use x86_64::instructions::port::Port;

const WIDTH: usize = 80;
const HEIGHT: usize = 25;
const BUFFER: *mut u16 = 0xB8000 as *mut u16;
const ATTR: u16 = 0x0F00; // white on black

pub struct Writer {
    row: usize,
    col: usize,
}

static WRITER: Mutex<Writer> = Mutex::new(Writer { row: 0, col: 0 });

fn put(row: usize, col: usize, val: u16) {
    unsafe { BUFFER.add(row * WIDTH + col).write_volatile(val) }
}

fn get(row: usize, col: usize) -> u16 {
    unsafe { BUFFER.add(row * WIDTH + col).read_volatile() }
}

impl Writer {
    fn blank_row(&mut self, row: usize) {
        for c in 0..WIDTH {
            put(row, c, ATTR | b' ' as u16);
        }
    }

    fn newline(&mut self) {
        self.col = 0;
        if self.row + 1 < HEIGHT {
            self.row += 1;
            return;
        }
        for r in 1..HEIGHT {
            for c in 0..WIDTH {
                put(r - 1, c, get(r, c));
            }
        }
        self.blank_row(HEIGHT - 1);
    }

    fn write_byte(&mut self, b: u8) {
        match b {
            b'\n' => self.newline(),
            b'\r' => self.col = 0,
            0x08 => {
                if self.col > 0 {
                    self.col -= 1;
                    put(self.row, self.col, ATTR | b' ' as u16);
                }
            }
            b'\t' => {
                for _ in 0..4 {
                    self.write_byte(b' ');
                }
            }
            0x20..=0x7E => {
                if self.col >= WIDTH {
                    self.newline();
                }
                put(self.row, self.col, ATTR | b as u16);
                self.col += 1;
            }
            _ => {}
        }
    }

    fn update_cursor(&self) {
        let pos = (self.row * WIDTH + self.col.min(WIDTH - 1)) as u16;
        unsafe {
            let mut idx = Port::<u8>::new(0x3D4);
            let mut data = Port::<u8>::new(0x3D5);
            idx.write(0x0F);
            data.write((pos & 0xFF) as u8);
            idx.write(0x0E);
            data.write((pos >> 8) as u8);
        }
    }
}

impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for b in s.bytes() {
            self.write_byte(b);
        }
        self.update_cursor();
        Ok(())
    }
}

pub fn clear() {
    let mut w = WRITER.lock();
    for r in 0..HEIGHT {
        w.blank_row(r);
    }
    w.row = 0;
    w.col = 0;
    w.update_cursor();
}

pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    // Interrupts must not preempt a holder of this lock; the IRQ handlers
    // never print, so a plain lock is enough.
    let _ = WRITER.lock().write_fmt(args);
}
