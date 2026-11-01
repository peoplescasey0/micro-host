//! Minimal interactive shell. Input comes from the PS/2 keyboard and COM1;
//! output goes to the VGA console and COM1.

use crate::keyboard::Key;
use crate::{paging, pic, sched, vga};
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt;

/// Prints to both the VGA console and the serial port.
pub fn _print(args: fmt::Arguments) {
    vga::_print(args);
    crate::serial::_print(args);
}

#[macro_export]
macro_rules! kprint {
    ($($arg:tt)*) => ($crate::shell::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! kprintln {
    () => ($crate::kprint!("\n"));
    ($($arg:tt)*) => ($crate::kprint!("{}\n", format_args!($($arg)*)));
}

const PROMPT: &str = "spatial> ";
const MAX_LINE: usize = 70; // prompt + line must fit in one 80-column row
const MAX_HISTORY: usize = 32;

pub struct Shell {
    line: String, // ASCII only, so byte index == column
    cursor: usize,
    history: Vec<String>,
    browsing: Option<usize>, // index into history while Up/Down is in use
    stash: String,           // the line being typed before browsing started
}

impl Shell {
    pub fn new() -> Self {
        Shell {
            line: String::new(),
            cursor: 0,
            history: Vec::new(),
            browsing: None,
            stash: String::new(),
        }
    }

    pub fn start(&mut self) {
        vga::clear();
        kprintln!("POST CARD SPATIAL OS");
        kprintln!("Type 'help' for commands.");
        kprint!("{}", PROMPT);
    }

    /// Redraws the whole input line and puts the cursor back in place.
    fn redraw(&self) {
        let back = self.line.len() - self.cursor;
        vga::_print(format_args!("\r{}{}", PROMPT, self.line));
        vga::clear_eol();
        vga::cursor_left(back);
        crate::serial::_print(format_args!("\r{}{}\x1b[K", PROMPT, self.line));
        if back > 0 {
            crate::serial::_print(format_args!("\x1b[{}D", back));
        }
    }

    fn set_line(&mut self, s: String) {
        self.cursor = s.len();
        self.line = s;
        self.redraw();
    }

    fn submit(&mut self) {
        kprintln!();
        let line = core::mem::take(&mut self.line);
        self.cursor = 0;
        self.browsing = None;
        self.stash.clear();
        let cmd = line.trim();
        if !cmd.is_empty() && self.history.last().map(|l| l.as_str()) != Some(cmd) {
            if self.history.len() == MAX_HISTORY {
                self.history.remove(0);
            }
            self.history.push(String::from(cmd));
        }
        run(cmd);
        kprint!("{}", PROMPT);
    }

    /// Handles one decoded key press.
    pub fn input(&mut self, key: Key) {
        match key {
            Key::Char('\n') | Key::Char('\r') => self.submit(),
            Key::Char('\x08') | Key::Char('\x7f') => {
                if self.cursor > 0 {
                    self.browsing = None;
                    self.cursor -= 1;
                    self.line.remove(self.cursor);
                    self.redraw();
                }
            }
            Key::Delete => {
                if self.cursor < self.line.len() {
                    self.browsing = None;
                    self.line.remove(self.cursor);
                    self.redraw();
                }
            }
            Key::Char(c @ (' '..='~' | '\t')) => {
                if self.line.len() < MAX_LINE {
                    self.browsing = None;
                    let c = if c == '\t' { ' ' } else { c };
                    self.line.insert(self.cursor, c);
                    self.cursor += 1;
                    if self.cursor == self.line.len() {
                        kprint!("{}", c); // plain append, no redraw needed
                    } else {
                        self.redraw();
                    }
                }
            }
            Key::Left => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    self.redraw();
                }
            }
            Key::Right => {
                if self.cursor < self.line.len() {
                    self.cursor += 1;
                    self.redraw();
                }
            }
            Key::Home => {
                self.cursor = 0;
                self.redraw();
            }
            Key::End => {
                self.cursor = self.line.len();
                self.redraw();
            }
            Key::Up => {
                let next = match self.browsing {
                    None if !self.history.is_empty() => {
                        self.stash = self.line.clone();
                        Some(self.history.len() - 1)
                    }
                    Some(i) if i > 0 => Some(i - 1),
                    _ => None,
                };
                if let Some(i) = next {
                    self.browsing = Some(i);
                    let s = self.history[i].clone();
                    self.set_line(s);
                }
            }
            Key::Down => {
                if let Some(i) = self.browsing {
                    if i + 1 < self.history.len() {
                        self.browsing = Some(i + 1);
                        let s = self.history[i + 1].clone();
                        self.set_line(s);
                    } else {
                        self.browsing = None;
                        let s = core::mem::take(&mut self.stash);
                        self.set_line(s);
                    }
                }
            }
            Key::Char(_) => {}
        }
    }
}

fn run(line: &str) {
    let mut parts = line.splitn(2, ' ');
    let cmd = parts.next().unwrap_or("");
    let rest = parts.next().unwrap_or("").trim();
    match cmd {
        "" => {}
        "help" => {
            kprintln!("help            this list");
            kprintln!("echo <text>     print text");
            kprintln!("clear           clear the screen");
            kprintln!("uptime          time since timer start");
            kprintln!("mem             frame allocator usage");
            kprintln!("ps              list tasks");
            kprintln!("spawn           start a demo task (3 ticks, 1 s apart)");
            kprintln!("user            run a ring 3 demo program");
            kprintln!("crash           run a ring 3 program that misbehaves (gets killed)");
            kprintln!("reboot          reset the machine");
            kprintln!("halt            stop the CPU");
        }
        "echo" => kprintln!("{}", rest),
        "clear" => vga::clear(),
        "uptime" => {
            let t = pic::ticks();
            kprintln!("{}.{:02} s ({} ticks)", t / pic::TIMER_HZ, t % pic::TIMER_HZ, t);
        }
        "mem" => {
            let used = paging::frames_in_use();
            let total = paging::frames_total();
            kprintln!("frames: {} used / {} total ({} KiB used)", used, total, used * 4);
        }
        "ps" => sched::for_each(|id, name, state| kprintln!("{:>3}  {:<10} {}", id, name, state)),
        "spawn" => {
            let id = sched::spawn("demo", || {
                for i in 1..=3 {
                    sched::sleep(100);
                    kprintln!("[demo] tick {}", i);
                }
            });
            kprintln!("spawned task {}", id);
        }
        "user" => kprintln!("spawned user task {}", crate::usermode::spawn("hello", crate::usermode::prog_hello())),
        "crash" => kprintln!("spawned user task {}", crate::usermode::spawn("evil", crate::usermode::prog_evil())),
        "reboot" => unsafe {
            // Pulse the CPU reset line through the keyboard controller.
            x86_64::instructions::port::Port::<u8>::new(0x64).write(0xFE);
            loop {
                x86_64::instructions::hlt();
            }
        },
        "halt" => {
            kprintln!("halted.");
            x86_64::instructions::interrupts::disable();
            loop {
                x86_64::instructions::hlt();
            }
        }
        other => kprintln!("unknown command: {} (try 'help')", other),
    }
}
