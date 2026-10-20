//! Minimal interactive shell. Input comes from the PS/2 keyboard and COM1;
//! output goes to the VGA console and COM1.

use crate::{paging, pic, sched, vga};
use alloc::string::String;
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
const MAX_LINE: usize = 120;

pub struct Shell {
    line: String,
}

impl Shell {
    pub fn new() -> Self {
        Shell { line: String::new() }
    }

    pub fn start(&mut self) {
        vga::clear();
        kprintln!("POST CARD SPATIAL OS");
        kprintln!("Type 'help' for commands.");
        kprint!("{}", PROMPT);
    }

    /// Handles one input character (already decoded).
    pub fn input(&mut self, c: char) {
        match c {
            '\r' | '\n' => {
                kprintln!();
                let line = core::mem::take(&mut self.line);
                run(line.trim());
                kprint!("{}", PROMPT);
            }
            '\x08' | '\x7f' => {
                if self.line.pop().is_some() {
                    vga::_print(format_args!("\x08"));
                    crate::serial::_print(format_args!("\x08 \x08"));
                }
            }
            ' '..='~' | '\t' => {
                if self.line.len() < MAX_LINE {
                    let c = if c == '\t' { ' ' } else { c };
                    self.line.push(c);
                    kprint!("{}", c);
                }
            }
            _ => {}
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
