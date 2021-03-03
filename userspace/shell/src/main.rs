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

        match cmd.split(' ').next().unwrap() {
            "echo" => println!("{}", cmd.split_once(' ').map(|(_, s)| s).unwrap_or_default()),
            "yeet" => {
                drop(std::syscalls::print(unsafe { core::slice::from_raw_parts(0xffffffc000000000 as *mut u8, 1024) }))
            }
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
        while let 0 = read_stdin(&mut c[..]) {}

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
