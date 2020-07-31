use crate::{mem, virt};
use alloc::{string::String, vec::Vec};

pub struct Repl {
    history: Vec<String>,
}

impl Repl {
    pub fn new() -> Self {
        Repl {
            history: Vec::with_capacity(10),
        }
    }

    pub fn run(&mut self) {
        let mut cmd = String::with_capacity(10);
        let mut history_index = 0;
        'outer: loop {
            cmd.clear();
            print!("~> ");

            let mut lock = virt::uart::UART0.lock();
            mem::heap::DO_TRACE.store(false, core::sync::atomic::Ordering::SeqCst);
            loop {
                match lock.read() {
                    b'\r' => break,
                    0x7F => {
                        if !cmd.is_empty() {
                            cmd.pop();
                            self.clear_prompt(&mut lock);
                        }

                        if history_index != self.history.len() {
                            history_index = self.history.len();
                        }
                    }
                    0x04 => {
                        self.clear_prompt(&mut lock);
                        lock.write_str("exit\n\r");
                        break 'outer;
                    }
                    0x1B => {
                        if lock.try_read() == Some(b'[') {
                            match lock.try_read() {
                                Some(b'A') => {
                                    if self.history.is_empty() {
                                        self.clear_prompt(&mut lock);
                                        continue;
                                    }

                                    if let Some(cmd2) = self.history.get(history_index - 1) {
                                        self.clear_prompt(&mut lock);
                                        lock.write_str(&cmd);
                                        cmd.clear();
                                        cmd += &*cmd2;

                                        if history_index != 0 {
                                            history_index -= 1;
                                        }
                                    }

                                    continue;
                                }
                                Some(b'B') => {
                                    if let Some(cmd2) = self.history.get(history_index) {
                                        self.clear_prompt(&mut lock);
                                        lock.write_str(&cmd);
                                        cmd.clear();
                                        cmd += &*cmd2;

                                        if history_index < self.history.len() {
                                            history_index += 1;
                                        }
                                    } else if history_index == self.history.len()
                                        && !self.history.is_empty()
                                    {
                                        self.clear_prompt(&mut lock);
                                    } else {
                                        history_index += 1;
                                    }

                                    continue;
                                }
                                Some(_) | None => {}
                            }
                        }
                    }
                    c => {
                        lock.write(c);
                        cmd.push(c as char);
                    }
                }
            }
            mem::heap::DO_TRACE.store(true, core::sync::atomic::Ordering::SeqCst);

            lock.write_str("\n\r");
            drop(lock);

            if self.history.last() != Some(&cmd) && !cmd.is_empty() {
                self.history.push(cmd.clone());
                history_index = self.history.len();
            }

            match &*cmd {
                "exit" => break,
                "trap" => unsafe {
                    asm!("ecall");
                },
                "clear" => print!("\x1B[2J\x1B[1;1H"),
                _ => continue,
            }
        }
    }

    pub fn clear_prompt(&self, lock: &mut spin::MutexGuard<virt::uart::Uart16550>) {
        lock.write_str("\x1B[2K\x1B[1G~> ");
    }
}
