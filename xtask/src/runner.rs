// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{
    build::{self, BuildTarget, Platform},
    Result, SbiImpl, Simulator, VanadiniteBuildOptions,
};
use clap::{ArgSettings, Clap};
use std::path::PathBuf;
use xshell::cmd;

#[derive(Clap)]
pub struct RunOptions {
    /// Number of CPUs
    #[clap(long, default_value = "4")]
    cpus: usize,

    /// Location to write debug logging to, enables QEMU debug logging
    #[clap(long)]
    debug_log: Option<PathBuf>,

    /// Whether or not to enable debugging in the simulator
    #[clap(long)]
    debug: bool,

    /// Path to a disk image
    #[clap(long)]
    drive_file: Option<PathBuf>,

    /// Arguments passed to the kernel
    #[clap(setting = ArgSettings::AllowEmptyValues)]
    #[clap(long, default_value = "")]
    kernel_args: String,

    /// Don't build anything before running
    #[clap(long)]
    no_build: bool,

    /// RAM size in MiB
    #[clap(long, default_value = "512")]
    ram: usize,

    #[clap(flatten)]
    vanadinite_options: VanadiniteBuildOptions,

    /// Which simulator to run with
    #[clap(arg_enum, long, default_value = "qemu")]
    with: Simulator,

    /// Which SBI implementation to run with
    #[clap(arg_enum, long, default_value = "opensbi")]
    sbi: SbiImpl,
}

impl Default for RunOptions {
    fn default() -> Self {
        Self {
            cpus: 5,
            debug_log: None,
            debug: false,
            drive_file: None,
            kernel_args: String::new(),
            no_build: false,
            ram: 512,
            vanadinite_options: VanadiniteBuildOptions {
                platform: Platform::Virt,
                kernel_features: String::new(),
                test: false,
                debug_build: false,
            },
            with: Simulator::Qemu,
            sbi: SbiImpl::OpenSbi,
        }
    }
}

pub fn run(options: RunOptions) -> Result<()> {
    if !options.no_build {
        build::build(match options.sbi {
            SbiImpl::OpenSbi => BuildTarget::OpenSBI(options.vanadinite_options.clone()),
            SbiImpl::Vanadium => BuildTarget::Vanadium(options.vanadinite_options.clone()),
        })?;
    }

    let platform = options.vanadinite_options.platform.to_string();
    let cpu_count = options.cpus.to_string();
    let ram = options.ram.to_string();
    let kernel_args = options.kernel_args;

    let enable_virtio_block_device = match (options.vanadinite_options.platform, &options.drive_file) {
        (Platform::Virt, Some(path)) => vec![
            String::from("-drive"),
            format!("file={},if=none,format=raw,id=hd", path.display()),
            String::from("-device"),
            String::from("virtio-blk-device,drive=hd"),
        ],
        _ => vec![],
    };

    let kernel_path = match options.vanadinite_options.debug_build {
        true => "src/kernel/target/riscv64gc-unknown-none-elf/debug/vanadinite",
        false => "src/kernel/target/riscv64gc-unknown-none-elf/release/vanadinite",
    };
    let sbi_firmware = match (options.sbi, options.with) {
        (SbiImpl::OpenSbi, Simulator::Qemu) => "build/opensbi-riscv64-generic-fw_jump.elf",
        (SbiImpl::OpenSbi, Simulator::Spike) => "build/opensbi-riscv64-generic-fw_payload.elf",
        (SbiImpl::Vanadium, _) => "build/vanadium.bin",
    };

    #[rustfmt::skip]
    match options.with {
        Simulator::Qemu => {
            let debug = match options.debug {
                true => vec![String::from("-s"), String::from("-S")],
                false => vec![],
            };
            let debug_log = match &options.debug_log {
                Some(path) => vec![
                    String::from("-d"),
                    String::from("guest_errors,\
                                    trace:virtio*"),
                    String::from("-D"),
                    format!("{}", path.display()),
                    String::from("-monitor"), String::from("stdio"),
                ],
                None => vec![String::from("-serial"), String::from("mon:stdio"), String::from("-nographic")],
            };

            cmd!("
                qemu-system-riscv64
                    -machine {platform}
                    -cpu rv64
                    -smp {cpu_count}
                    -m {ram}M
                    -append {kernel_args}
                    -global virtio-mmio.force-legacy=false
                    {enable_virtio_block_device...}
                    -netdev user,id=net1
                    -device virtio-net-device,netdev=net1
                    -object filter-dump,id=f1,netdev=net1,file=nettraffic.dat
                    -bios {sbi_firmware}
                    -kernel {kernel_path}
                    {debug...}
                    {debug_log...}
            ").run()?;
        }
        Simulator::Spike => {
            let debug = match options.debug {
                true => vec!["-d"],
                false => vec![],
            };

            cmd!("
                ./build/spike
                    -p{cpu_count}
                    -m{ram}
                    {debug...}
                    --isa=rv64gc
                    --bootargs={kernel_args}
                    {sbi_firmware}
            ").run()?;
        }
    };

    Ok(())
}

pub fn test(mut options: RunOptions) -> Result<()> {
    options.vanadinite_options.test = true;
    options.vanadinite_options.platform = Platform::Virt;

    let build_opts = &options.vanadinite_options;
    build::build(BuildTarget::Vanadinite(build_opts.clone()))?;

    let _dir = xshell::pushd("./src");

    let platform = options.vanadinite_options.platform.to_string();
    let cpu_count = options.cpus.to_string();
    let ram = options.ram.to_string();
    let kernel_args = options.kernel_args;

    let debug_log = match &options.debug_log {
        Some(path) => vec![
            String::from("-d"),
            String::from("guest_errors,trace:riscv_trap,trace:pmpcfg_csr_write,trace:pmpaddr_csr_write,int"),
            String::from("-D"),
            format!("../{}", path.display()),
            String::from("-monitor"),
            String::from("stdio"),
        ],
        None => vec![String::from("-serial"), String::from("mon:stdio"), String::from("-nographic")],
    };

    #[rustfmt::skip]
    cmd!("
        qemu-system-riscv64
            -machine {platform}
            -cpu rv64
            -smp {cpu_count}
            -m {ram}M
            -append {kernel_args}
            -bios ../opensbi-riscv64-generic-fw_jump.bin 
            -kernel target/riscv64gc-unknown-none-elf/debug/vanadinite
            {debug_log...}
    ").run()?;

    Ok(())
}
