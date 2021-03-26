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

    let enable_virtio_block_device = match (env.machine, &env.drive_file) {
        (Machine::Virt, Some(path)) => vec![
            String::from("-global"),
            String::from("virtio-mmio.force-legacy=false"),
            String::from("-drive"),
            format!("file={},if=none,format=raw,id=hd", path.display()),
            String::from("-device"),
            String::from("virtio-blk-device,drive=hd"),
        ],
        _ => vec![],
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
                    -bios opensbi-riscv64-generic-fw_jump.bin 
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
            let debug_log = match &env.debug_log {
                Some(path) => vec![String::from("-d"), String::from("guest_errors,trace:riscv_trap,trace:pmpcfg_csr_write,trace:pmpaddr_csr_write,int"), String::from("-D"), format!("{}", path.display())],
                None => vec![String::new()],
            };

            cmd!("
                qemu-system-riscv64
                    -machine {machine}
                    -cpu rv64
                    -smp {cpu_count}
                    -m {ram}
                    -append {kernel_args}
                    {enable_virtio_block_device...}
                    -bios opensbi-riscv64-generic-fw_jump.bin 
                    -kernel src/target/riscv64gc-unknown-none-elf/release/vanadinite
                    -serial mon:stdio
                    -nographic
                    {debug_log...}
            ").run()?;
        }
    };

    Ok(())
}
