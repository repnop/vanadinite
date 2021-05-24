// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#![feature(allocator_api, asm)]

extern crate alloc;

use alloc::alloc::Allocator;
use core::num::NonZeroUsize;
use std::librust::syscalls::*;
use std::librust::{
    message::{Message, MessageKind, Sender},
    syscalls::allocation::{alloc_virtual_memory, AllocationOptions, MemoryPermissions},
    task::Tid,
};

fn main() {
    let mut history: Vec<Vec<u8>> = Vec::new_in(std::heap::TaskLocal::new());
    let mut history_index = None;
    let mut curr_history: Option<Vec<u8>> = None;

    loop {
        print!("vanadinite> ");

        if let Some(cmd) = &curr_history {
            print!("{}", core::str::from_utf8(cmd).unwrap());
        }

        let input = match read_input(curr_history.as_ref()) {
            Some(input) => input,
            None => continue,
        };

        let cmd_bytes = match input {
            Input::Command(cmd_bytes) => cmd_bytes,
            Input::Control(ControlSequence::ArrowUp) => {
                let index = match &mut history_index {
                    Some(i) if history.len() > *i + 1 => {
                        *i += 1;
                        *i
                    }
                    Some(i) => *i,
                    None => {
                        history_index = Some(0);
                        0
                    }
                };

                if index < history.len() {
                    curr_history = Some(history[index].clone());
                }

                clear_line();
                continue;
            }
            Input::Control(ControlSequence::ArrowDown) => {
                if let Some(i) = &mut history_index {
                    match *i {
                        0 => {
                            history_index = None;
                            curr_history = None;
                        }
                        n => {
                            *i -= 1;
                            curr_history = Some(history[n - 1].clone());
                        }
                    }
                }

                clear_line();
                continue;
            }
        };

        let cmd = match core::str::from_utf8(&cmd_bytes).ok() {
            Some("") => continue,
            Some(cmd) => cmd,
            None => {
                println!("unknown command :(");
                continue;
            }
        };

        let (cmd, args) = cmd.split_once(' ').unwrap_or((cmd, ""));

        match cmd {
            "echo" => println!("{}", args),
            "yeet" => {
                println!("Asking the kernel to print some of its memory!");
                let kresult = print(unsafe { core::slice::from_raw_parts(0xffffffc000000000 as *mut u8, 1024) });
                println!("Kernel responded with: {:?}", kresult);
            }
            "send" => match args.trim().parse::<usize>() {
                Ok(0) | Err(_) => {
                    println!("Need valid TID :(")
                }
                Ok(tidn) => {
                    let tid = Tid::new(NonZeroUsize::new(tidn).unwrap());
                    let ret = send_message(
                        tid,
                        Message {
                            sender: Sender::dummy(),
                            kind: MessageKind::Notification(0),
                            fid: 0,
                            arguments: [0; 8],
                        },
                    );

                    match ret {
                        Ok(_) => println!("Message sent to TID {}!", tidn),
                        Err(e) => println!("Couldn't send message: {:?}", e),
                    }
                }
            },
            "read" => match receive_message() {
                Ok(Some(msg)) => println!("We had a message! {:?}", msg),
                Ok(None) => println!("No messages :("),
                Err(e) => println!("Error receiving message: {:?}", e),
            },
            "test_alloc_mem" => match alloc_virtual_memory(
                4096,
                AllocationOptions::None,
                MemoryPermissions::Read | MemoryPermissions::Write,
            ) {
                Ok(ptr) => {
                    println!("Kernel returned us address: {:#p}", ptr);
                    println!("Testing read/write...");

                    for i in 0..4096 {
                        unsafe { *ptr.add(i) = ((i as u8) % (126 - 32)) + 32 };
                    }

                    for i in 0..(4096 / 256) {
                        for c in 0..256 {
                            unsafe { print!("{}", *ptr.add(i * 256 + c) as char) };
                        }
                        println!();
                    }
                }
                Err(e) => println!("Kernel returned error: {:?}", e),
            },
            "test_std_allocator" => {
                println!("Testing Box...");
                let mut b = Box::new_in(5u32, std::heap::TaskLocal::new());
                *b = 6;
                println!("    *b = {}", b);

                println!("Testing Vec...");
                let mut v = Vec::new_in(std::heap::TaskLocal::new());

                for i in 0..100usize {
                    v.push(i);
                }
                println!("    v.len() = {}", v.len());
            }
            "test_guard_page" => unsafe {
                let sp: *mut u8;
                asm!("mv {}, sp", out(reg) sp);

                *(sp.add(4096)) = 0;
            },
            "test_large_page_alloc" => {
                std::heap::TaskLocal::new().allocate(alloc::alloc::Layout::from_size_align(32768, 8).unwrap()).unwrap();
            }
            "tp" => {
                let tp: usize;
                unsafe { asm!("mv {}, tp", out(reg) tp) };

                println!("tp={:#p}", tp as *mut u8);
            }
            "tid" => {
                println!("Our TID is {}", current_tid().value())
            }
            _ => println!("unknown command :("),
        }

        if history.first() != Some(&cmd_bytes) {
            history.insert(0, cmd_bytes);
        }
        history_index = None;
        curr_history = None;
    }
}

enum Input {
    Command(Vec<u8>),
    Control(ControlSequence),
}

enum ControlSequence {
    ArrowUp,
    ArrowDown,
}

fn read_input(current_cmd: Option<&Vec<u8>>) -> Option<Input> {
    let mut buf = match current_cmd {
        Some(cmd) => cmd.clone(),
        None => Vec::with_capacity_in(256, std::heap::TaskLocal::new()),
    };

    let max_len = 256;
    let mut read = 0;

    while read < max_len {
        let mut c = [0u8];
        while let Ok(0) = read_stdin(&mut c[..]) {}

        if c[0] == b'\x1B' {
            let mut ctrl_seq = [b'\x1B', 0, 0];
            for byte in &mut ctrl_seq[1..] {
                while let Ok(0) = read_stdin(&mut c[..]) {}
                *byte = c[0];
            }

            return match &ctrl_seq {
                b"\x1B[A" => Some(Input::Control(ControlSequence::ArrowUp)),
                b"\x1B[B" => Some(Input::Control(ControlSequence::ArrowDown)),
                _ => None,
            };
        }

        match c[0] {
            b'\r' => break,
            0x7F if !buf.is_empty() => {
                print!("\x1B[1D \x1B[1D");
                read -= 1;
                buf.pop();
                continue;
            }
            0x7F => continue,
            _ => print!("{}", c[0] as char),
        }

        buf.push(c[0]);
        read += 1;
    }

    println!();

    Some(Input::Command(buf))
}

fn clear_line() {
    print!("\x1B[2K\x1B[1G");
}
