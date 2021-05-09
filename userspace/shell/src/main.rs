use core::num::NonZeroUsize;
use std::librust::{
    message::{Message, MessageKind, Sender},
    task::Tid,
};

// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

fn main() {
    let mut buf = [0; 256];
    loop {
        print!("vanadinite> ");
        let cmd = match read_line(&mut buf[..]) {
            Some(s) => s,
            None => {
                println!("Unrecognized input :(");
                continue;
            }
        };

        let mut args = cmd.split(' ');
        let cmd = args.next().unwrap();
        let arg_str = args.next().unwrap_or_default();

        match cmd {
            "echo" => println!("{}", arg_str),
            "yeet" => {
                println!("Asking the kernel to print some of its memory!");
                let kresult =
                    std::syscalls::print(unsafe { core::slice::from_raw_parts(0xffffffc000000000 as *mut u8, 1024) });
                println!("Kernel responded with: {:?}", kresult);
            }
            "send" => match arg_str.trim().parse::<usize>() {
                Ok(0) | Err(_) => {
                    println!("Need valid TID :(")
                }
                Ok(tidn) => {
                    let tid = Tid::new(NonZeroUsize::new(tidn).unwrap());
                    let ret = send_message(
                        tid,
                        Message {
                            sender: Sender::dummy(),
                            kind: MessageKind::Request(Some(NonZeroUsize::new(1).unwrap())),
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
            "" => {}
            _ => println!("Unrecognized command :("),
        }
    }
}

fn read_line(buf: &mut [u8]) -> Option<&str> {
    let max_len = buf.len();
    let mut read = 0;

    while read < max_len {
        let mut c = [0u8];
        while let Ok(0) = read_stdin(&mut c[..]) {}

        match c[0] {
            b'\r' => break,
            0x7F if read > 0 => {
                print!("\x1B[1D \x1B[1D");
                read -= 1;
                continue;
            }
            0x7F => continue,
            _ => print!("{}", c[0] as char),
        }

        buf[read] = c[0];
        read += 1;
    }

    println!();

    core::str::from_utf8(&buf[..read]).ok()
}

#[used]
#[link_section = ".capabilities"]
static CAPABILITIES: [std::Capability; 2] = [std::Capability::Driver, std::Capability::Server];
