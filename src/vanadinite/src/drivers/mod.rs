pub mod sifive_uart;
pub mod uart16550;

pub trait CompatibleWith {
    fn list() -> &'static [&'static str];
}
