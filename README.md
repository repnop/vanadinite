# `vanadinite`

A RISC-V (RV64GC) microkernel written in Rust

## License

The source code in this project is licensed under the Mozilla Public License 2.0

## Building
### Vanadinite and Userspace
The Rust `riscv64gc-unknown-none-elf` toolchain must be installed, then run
`cargo xtask vanadinite` to build the kernel ELF, or `cargo xtask userspace` to
build the userspace executables and package them in a tar file in the root
directory. **Note:** building the kernel will automatically build and package
the userspace binaries.

### OpenSBI
Building the OpenSBI firmware image requires you to have the
`riscv64-unknown-elf-` binutils package installed. For Arch users, you can
install the following packages: `riscv64-unknown-elf-gdb`,
`riscv64-unknown-elf-binutils`, `riscv64-unknown-elf-newlib`. If the tools are
installed, run `cargo xtask opensbi` to build the SBI firmare image and place it
in the root directory for use by QEMU.

## Running
### Requirements
You will need to have the `qemu-system-riscv64` QEMU executable installed and in
your path.

### Running QEMU
To run QEMU with the kernel, the OpenSBI image must be built, as well as the
kernel ELF which will be built before executing QEMU.

To run, execute `cargo xtask run`. By default this will run `vanadinite` on the
QEMU `virt` machine, if you would like to run for a different platform (e.g.
`sifive_u`) or change the machine properties, see `cargo xtask run --help` for a
full list of available options.

Default settings are:

            Platform: virt
              # CPUs: 5
                 RAM: 512M
    Kernel Arguments: ""

To exit QEMU press: `Ctrl+A` + `x`

## Screenshots!

![Running the shell](assets/running_shell.png)