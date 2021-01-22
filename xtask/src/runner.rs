use crate::{
    build::{self, Target as BuildTarget},
    Env, Machine, Result,
};
use xshell::cmd;

#[derive(Clone, Copy)]
pub enum Target {
    Debug,
    Gdb,
    Run,
}

impl Target {
    const fn dependencies(self) -> &'static [BuildTarget] {
        match self {
            Target::Debug | Target::Run => &[BuildTarget::Userspace, BuildTarget::Vanadinite],
            Target::Gdb => &[],
        }
    }
}

pub fn run(target: Target, env: &Env) -> Result<()> {
    for dep in target.dependencies() {
        build::build(*dep, &env)?;
    }

    let machine = env.machine.to_string();
    let cpu_count = env.cpus.to_string();
    let ram = &env.ram;
    let kernel_args = &env.kernel_args;

    let enable_virtio_block_device = match env.machine {
        Machine::Virt => &[
            "-global",
            "virtio-mmio.force-legacy=false",
            "-drive",
            "file=testing_files/test_fat.fs,if=none,format=raw,id=hd",
            "-device",
            "virtio-blk-device,drive=hd",
        ],
        _ => &[][..],
    };

    #[rustfmt::skip]
    match target {
        Target::Debug =>{
            cmd!("
                qemu-system-riscv64 
                    -machine {machine}
                    -cpu rv64
                    -smp {cpu_count}
                    -m {ram}
                    -append {kernel_args}
                    {enable_virtio_block_device...}
                    -bios opensbi-riscv64-generic-fw_dynamic.bin 
                    -kernel src/target/riscv64gc-unknown-none-elf/release/vanadinite
                    -monitor stdio
                    -gdb tcp::1234
                    -S 
                    -d guest_errors,trace:riscv_trap,trace:pmpcfg_csr_write,trace:pmpaddr_csr_write,int
                    -D qemu.log
            ").run()?;
        }
        Target::Gdb => {
            cmd!("
                riscv64-unknown-elf-gdb 
                    'src/target/riscv64gc-unknown-none-elf/release/vanadinite' 
                    '--eval-command' 'target remote :1234'
            ").run()?;
        }
        Target::Run => {
            cmd!("
                qemu-system-riscv64
                    -machine {machine}
                    -cpu rv64
                    -smp {cpu_count}
                    -m {ram}
                    -append {kernel_args}
                    {enable_virtio_block_device...}
                    -bios opensbi-riscv64-generic-fw_dynamic.bin 
                    -kernel src/target/riscv64gc-unknown-none-elf/release/vanadinite
                    -serial mon:stdio
                    -nographic
            ").run()?;
        }
    };

    Ok(())
}
