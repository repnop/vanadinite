# vanadinite

A toy RISC-V OS made with Rust.

## License

The source code in this project is licensed under the Mozilla Public License 2.0

## Building

To build the project you must have the `riscv64gc-unknown-none-elf` rustc target
installed.

## Running

`cargo-make` is required to run the kernel via QEMU.

To run, you must have `qemu-system-riscv64` installed and in your path, then
just do `cargo make run`! By default this will run `vanadinite` on the QEMU
`virt` machine, if you would like to run it via the `sifive_u` SiFive Freedom
Unleashed 540 machine, you can do so with `cargo make run --env
MACHINE=sifive_u`.

To exit QEMU press: `Ctrl+A` + `x`