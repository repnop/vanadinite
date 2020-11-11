pub mod sifive {
    pub mod fu540_c000 {
        pub mod clint;
        pub mod uart;
    }
}

pub mod misc {
    pub mod uart16550;
}

pub trait CompatibleWith {
    fn list() -> &'static [&'static str];
}
