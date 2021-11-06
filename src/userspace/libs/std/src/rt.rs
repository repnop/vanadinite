#[no_mangle]
unsafe extern "C" fn _start(argc: isize, argv: *const *const u8, a2: usize) -> ! {
    extern "C" {
        fn main(_: isize, _: *const *const u8) -> isize;
    }

    #[rustfmt::skip]
    asm!("
        .option push
        .option norelax
        lla gp, __global_pointer$
        .option pop
    ");

    A2 = a2;

    main(argc, argv);
    librust::syscalls::exit()
}

extern "C" {
    static mut ARGS: [usize; 2];
    static mut A2: usize;
}

#[lang = "start"]
fn lang_start<T>(main: fn() -> T, argc: isize, argv: *const *const u8) -> isize {
    unsafe { ARGS = [argc as usize, argv as usize] };

    let mut map = crate::env::CAP_MAP.write();
    let channel = crate::ipc::IpcChannel::new(librust::capabilities::CapabilityPtr::new(0));

    // FIXME: Wowie is this some awful code!
    while let Ok(msg) = channel.read() {
        let name = match core::str::from_utf8(msg.as_bytes()) {
            Ok(name) => name,
            Err(_) => break,
        };

        if name == "done" {
            break;
        }

        let cap = match channel.receive_capability() {
            Ok(cap) => cap,
            Err(_) => break,
        };

        map.insert(name.into(), cap);
    }

    map.insert("parent".into(), librust::capabilities::CapabilityPtr::new(0));
    drop(map);

    main();
    0
}
