// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{Result, VanadiniteBuildOptions};
use clap::Clap;
use std::fs;
use tar::{Builder, Header};
use xshell::{cmd, cp, pushd, pushenv, rm_rf};

#[derive(Clap, Clone, Copy)]
#[clap(rename_all = "snake_case")]
pub enum Platform {
    Virt,
    SifiveU,
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Platform::Virt => write!(f, "virt"),
            Platform::SifiveU => write!(f, "sifive_u"),
        }
    }
}

#[derive(Clap)]
#[clap(rename_all = "snake_case")]
pub enum BuildTarget {
    /// The `vanadinite` kernel
    Vanadinite(VanadiniteBuildOptions),
    /// The OpenSBI firmware image (builds `vanadinite`)
    #[clap(name = "opensbi")]
    OpenSBI(VanadiniteBuildOptions),
    /// The RISC-V ISA simulator
    Spike,
    /// Userspace applications for the `vanadinite` kernel to use on boot
    Userspace,
}

impl BuildTarget {
    fn dependencies(&self) -> Vec<Self> {
        match self {
            BuildTarget::Vanadinite(_) => vec![BuildTarget::Userspace],
            BuildTarget::OpenSBI(args) => vec![BuildTarget::Vanadinite(args.clone())],
            _ => vec![],
        }
    }
}

impl BuildTarget {
    pub fn env(&self) -> Vec<xshell::Pushenv> {
        match self {
            BuildTarget::Userspace => vec![],
            BuildTarget::Vanadinite(opts) => vec![pushenv(
                "RUSTFLAGS",
                format!("-C code-model=medium -C link-arg=-Tvanadinite/lds/{}.lds", opts.platform),
            )],
            BuildTarget::OpenSBI(_) | BuildTarget::Spike => {
                vec![pushenv("CROSS_COMPILE", "riscv64-unknown-elf-"), pushenv("PLATFORM_RISCV_XLEN", "64")]
            }
        }
    }
}

pub fn build(target: BuildTarget) -> Result<()> {
    for dependency in target.dependencies() {
        build(dependency)?;
    }

    let _env = target.env();

    match target {
        BuildTarget::Userspace => {
            let init_tar = std::env::current_dir()?.join("initfs.tar");

            rm_rf(&init_tar)?;

            let _dir = pushd("src/userspace")?;
            cmd!("cargo build --release --workspace").run()?;

            let out = fs::File::create(init_tar)?;
            let mut archive = Builder::new(out);

            for (bin, path) in walkdir::WalkDir::new("target/riscv64gc-unknown-none-elf/release/")
                .max_depth(1)
                .into_iter()
                .filter_entry(|e| !e.file_name().to_str().map(|s| s.starts_with('.')).unwrap_or(false))
                .filter_map(|e| e.ok())
                .map(|e| e.into_path())
                .filter(|e| e.is_file() && e.extension().is_none())
                .map(|p| (fs::read(&p), p))
            {
                let mut header = Header::new_ustar();
                let bin = std::io::Cursor::new(bin?);
                let metadata = fs::metadata(&path)?;
                let filename = path.file_name().unwrap();

                header.set_device_major(0)?;
                header.set_device_minor(0)?;
                header.set_metadata(&metadata);
                header.set_cksum();

                archive.append_data(&mut header, filename, bin)?;
            }

            archive.finish()?;

            let _dir = pushd("init/");
            cmd!("cargo build --release").run()?;
            cp("target/riscv64gc-unknown-none-elf/release/init", "../../../init")?;
        }
        BuildTarget::Vanadinite(build_opts) => {
            let features = format!("platform.{} {}", build_opts.platform, build_opts.kernel_features);

            let (subcmd, test) = match build_opts.test {
                true => ("rustc", &["--", "--test"][..]),
                false => ("build", &["--release"][..]),
            };

            let _dir = pushd("./src/kernel");
            #[rustfmt::skip]
            cmd!("
                cargo {subcmd}
                    -p vanadinite
                    --target riscv64gc-unknown-none-elf
                    --no-default-features
                    --features {features}
                    {test...}
            ").run()?;
        }
        BuildTarget::OpenSBI(_) => {
            cmd!("riscv64-unknown-elf-objcopy -O binary src/kernel/target/riscv64gc-unknown-none-elf/release/vanadinite src/kernel/target/riscv64gc-unknown-none-elf/release/vanadinite.bin --set-start 0x80200000").run()?;

            cmd!("git submodule init submodules/opensbi").run()?;
            cmd!("git submodule update --remote submodules/opensbi").run()?;
            let _dir = pushd("./submodules/opensbi")?;

            cmd!("make PLATFORM=generic FW_PIC=no FW_PAYLOAD_PATH=../../src/kernel/target/riscv64gc-unknown-none-elf/release/vanadinite.bin").run()?;

            cp("build/platform/generic/firmware/fw_jump.bin", "../../opensbi-riscv64-generic-fw_jump.bin")?;
            cp("build/platform/generic/firmware/fw_jump.elf", "../../opensbi-riscv64-generic-fw_jump.elf")?;
            cp("build/platform/generic/firmware/fw_payload.bin", "../../opensbi-riscv64-generic-fw_payload.bin")?;
            cp("build/platform/generic/firmware/fw_payload.elf", "../../opensbi-riscv64-generic-fw_payload.elf")?;
        }
        BuildTarget::Spike => {
            cmd!("git submodule init submodules/riscv-isa-sim").run()?;
            cmd!("git submodule update --remote submodules/riscv-isa-sim").run()?;
            let _dir = pushd("./submodules/riscv-isa-sim")?;

            cmd!("mkdir -p build").run()?;
            let _dir = pushd("./build")?;

            cmd!("../configure").run()?;
            cmd!("make").run()?;

            cp("spike", "../../../spike")?;
        }
    }

    Ok(())
}
