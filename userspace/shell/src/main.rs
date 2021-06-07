// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

#![feature(allocator_api, asm)]

extern crate alloc;

use core::num::NonZeroUsize;
use std::librust::syscalls::{channel::ChannelId, *};
use std::librust::{
    message::{Message, MessageKind, Sender},
    syscalls::{
        allocation::{alloc_virtual_memory, AllocationOptions, MemoryPermissions},
        channel,
    },
    task::Tid,
};

fn main() {
    let mut history: VecDeque<String> = VecDeque::new();
    let mut history_index = None;
    let mut curr_history: Option<&str> = None;
    let mut channels = Vec::new();

    loop {
        print!("vanadinite> ");

        if let Some(cmd) = &curr_history {
            print!("{}", cmd);
        }

        let input = match read_input(curr_history.as_deref()) {
            Some(input) => input,
            None => continue,
        };

        let cmd_str = match input {
            Input::Command(cmd_str) => cmd_str,
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
                    curr_history = Some(&history[index]);
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
                            curr_history = Some(&history[n - 1]);
                        }
                    }
                }

                clear_line();
                continue;
            }
        };

        let cmd = match &*cmd_str {
            "" => continue,
            cmd => cmd,
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
                let mut b = Box::new(5u32);
                *b = 6;
                println!("    *b = {}", b);

                println!("Testing Vec...");
                let mut v = Vec::new();

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
            "test_large_page_alloc" => unsafe {
                alloc::alloc::alloc(alloc::alloc::Layout::from_size_align(32768, 8).unwrap());
            },
            "tp" => {
                let tp: usize;
                unsafe { asm!("mv {}, tp", out(reg) tp) };

                println!("tp={:#p}", tp as *mut u8);
            }
            "tid" => {
                println!("Our TID is {}", current_tid().value())
            }
            "where_main" => println!("main is at: {:#p}", main as *mut u8),
            "open_channel" => match args.trim().parse::<usize>() {
                Ok(0) | Err(_) => {
                    println!("Need valid TID :(")
                }
                Ok(tidn) => {
                    let tid = Tid::new(NonZeroUsize::new(tidn).unwrap());
                    let ret = send_message(
                        tid,
                        Message {
                            sender: Sender::dummy(),
                            kind: MessageKind::Request(None),
                            fid: 1,
                            arguments: [0; 8],
                        },
                    );

                    match ret {
                        Ok(_) => println!("Message sent to TID {}!", tidn),
                        Err(e) => println!("Couldn't send message: {:?}", e),
                    }

                    println!("Waiting for response with channel ID");
                    let channel_id = loop {
                        match receive_message() {
                            Ok(Some(msg)) if msg.sender == Sender::task(tid) => {
                                break ChannelId::new(msg.arguments[0]);
                            }
                            _ => {}
                        }
                    };

                    match channel::read_message(channel_id) {
                        Ok(Some(msg)) => {
                            let slice = unsafe { core::slice::from_raw_parts(msg.ptr, msg.len) };
                            println!("A message already! Contents: {:?}", slice);

                            channel::retire_message(channel_id, msg.id).unwrap();

                            channels.push(channel_id);

                            println!("Channel ID with {:?}: {:?}", tid, channel_id);
                        }
                        Ok(None) => {
                            channels.push(channel_id);
                            println!("Channel ID with {:?}: {:?}", tid, channel_id);
                        }
                        Err(_) => println!("Didn't get back a valid channel ID :("),
                    }
                }
            },
            "read_channel" => match args.trim().parse::<usize>() {
                Err(_) => {
                    println!("Need valid channel ID :(")
                }
                Ok(cid) => {
                    let channel_id = ChannelId::new(cid);

                    match channels.iter().find(|c| **c == channel_id) {
                        None => println!("Not a channel I know about :("),
                        Some(_) => match channel::read_message(channel_id) {
                            Ok(Some(msg)) => {
                                let slice = unsafe { core::slice::from_raw_parts(msg.ptr, msg.len) };

                                match core::str::from_utf8(slice) {
                                    Err(_) => println!("A message! Contents: {:?}", slice),
                                    Ok(s) => println!("A message! It says: {}", s),
                                }

                                channel::retire_message(channel_id, msg.id).unwrap();
                            }
                            Ok(None) => println!("No message waiting on channel"),
                            Err(_) => println!("Didn't get back a valid channel ID :("),
                        },
                    }
                }
            },
            _ => println!("unknown command :("),
        }

        if history.front() != Some(&cmd_str) {
            history.push_front(cmd_str);
        }
        history_index = None;
        curr_history = None;
    }
}

enum Input {
    Command(String),
    Control(ControlSequence),
}

enum ControlSequence {
    ArrowUp,
    ArrowDown,
}

fn read_input(current_cmd: Option<&str>) -> Option<Input> {
    let mut buf = match current_cmd {
        Some(cmd) => cmd.to_string(),
        None => String::with_capacity(256),
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

        buf.push(c[0] as char);
        read += 1;
    }

    println!();

    Some(Input::Command(buf))
}

fn clear_line() {
    print!("\x1B[2K\x1B[1G");
}
