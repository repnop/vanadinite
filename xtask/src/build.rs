// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use crate::{Result, VanadiniteBuildOptions};
use anyhow::Context;
use clap::{Subcommand, ValueEnum};
use std::{fs, io::Write};
use tar::{Builder, Header};
use xshell::{cmd, Shell};

#[derive(Clone, Copy, ValueEnum)]
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

#[derive(Subcommand)]
#[clap(rename_all = "snake_case")]
pub enum BuildTarget {
    /// The `vanadinite` kernel
    Vanadinite(VanadiniteBuildOptions),
    /// The OpenSBI firmware image (builds `vanadinite`)
    #[clap(name = "opensbi")]
    OpenSBI(VanadiniteBuildOptions),
    /// The `vanadium` firmware image (builds `vanadinite`)
    Vanadium(VanadiniteBuildOptions),
    /// The RISC-V ISA simulator
    Spike,
    /// Userspace applications for the `vanadinite` kernel to use on boot
    Userspace,
}

impl BuildTarget {
    fn name(&self) -> &'static str {
        match self {
            Self::Vanadinite(_) => "vanadinite",
            Self::Userspace => "userspace",
            Self::Vanadium(_) => "vanadium",
            Self::OpenSBI(_) => "OpenSBI",
            Self::Spike => "Spike",
        }
    }

    fn dependencies(&self) -> Vec<Self> {
        match self {
            BuildTarget::Vanadinite(_) => vec![BuildTarget::Userspace],
            BuildTarget::OpenSBI(args) /* | BuildTarget::Vanadium(args) */ => vec![BuildTarget::Vanadinite(args.clone())],
            _ => vec![],
        }
    }
}

impl BuildTarget {
    pub fn env<'a>(&self, shell: &'a Shell) -> Vec<xshell::PushEnv<'a>> {
        match self {
            BuildTarget::Userspace => vec![],
            BuildTarget::Vanadinite(opts) => {
                vec![shell.push_env("VANADINITE_TARGET_PLATFORM", opts.platform.to_string())]
            }
            BuildTarget::Vanadium(opts) => {
                vec![shell
                    .push_env("RUSTFLAGS", format!("-C code-model=medium -C link-arg=-Tlds/{}.lds", opts.platform))]
            }
            BuildTarget::OpenSBI(_) | BuildTarget::Spike => {
                vec![
                    shell.push_env("CROSS_COMPILE", "riscv64-unknown-elf-"),
                    shell.push_env("PLATFORM_RISCV_XLEN", "64"),
                ]
            }
        }
    }
}

pub fn build(shell: &Shell, target: BuildTarget, quiet: bool) -> Result<()> {
    tracing::info!("Running build for {}", target.name());

    check_llvm_tools(shell)?;

    if !shell.path_exists("build/") {
        tracing::debug!("Creating root build directory");
        shell.create_dir("build/").context("failed to make build directory")?;
    }

    for dependency in target.dependencies() {
        build(shell, dependency, quiet)?;
    }

    let cargo_quiet = quiet.then_some("--quiet");
    let _env = target.env(shell);

    match target {
        BuildTarget::Userspace => {
            let init_tar = std::env::current_dir()?.join("build/initfs.tar");

            shell.remove_path(&init_tar)?;

            let _dir = shell.push_dir("src/userspace");
            let mut cmd =
                cmd!(shell, "cargo build {cargo_quiet...} --release --workspace --target riscv64gc-unknown-none-elf");
            cmd.set_quiet(quiet);
            cmd.set_ignore_stdout(quiet);
            cmd.set_ignore_stderr(quiet);
            cmd.run()?;

            let out = fs::File::create(init_tar)?;
            let mut archive = Builder::new(out);

            for (bin, path) in walkdir::WalkDir::new("src/userspace/target/riscv64gc-unknown-none-elf/release/")
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

                tracing::debug!("Adding {} to archive", filename.to_str().unwrap());

                header.set_device_major(0)?;
                header.set_device_minor(0)?;
                header.set_metadata(&metadata);
                header.set_cksum();

                archive.append_data(&mut header, filename, bin)?;
            }

            archive.finish()?;
        }
        BuildTarget::Vanadinite(build_opts) => {
            let features = format!("platform.{} {}", build_opts.platform, build_opts.kernel_features);

            let opt_level = if build_opts.debug_build { "--profile=dev" } else { "--release" };
            let opt_level = &[opt_level][..];
            let (subcmd, test) = match build_opts.test {
                true => ("rustc", &["--", "--test"][..]),
                false => ("build", opt_level),
            };

            let _dir = shell.push_dir("./src/kernel");
            #[rustfmt::skip]
            let mut cmd = cmd!(shell, "
                cargo {subcmd}
                    {cargo_quiet...}
                    -p vanadinite
                    --target riscv64imac-unknown-none-elf
                    --no-default-features
                    --features {features}
                    {test...}
            ");
            cmd.set_quiet(quiet);
            cmd.set_ignore_stdout(quiet);
            cmd.set_ignore_stderr(quiet);
            cmd.run()?;
        }
        BuildTarget::Vanadium(build_opts) => {
            let features = format!("platform.{}", build_opts.platform);

            let _dir = shell.push_dir("./src/vanadium");
            #[rustfmt::skip]
            let mut cmd = cmd!(shell, "
                cargo build
                    {cargo_quiet...}
                    --release
                    --target riscv64imac-unknown-none-elf
                    --no-default-features
                    --features {features}
            ");
            cmd.set_quiet(quiet);
            cmd.set_ignore_stdout(quiet);
            cmd.set_ignore_stderr(quiet);
            cmd.run()?;

            let mut cmd = cmd!(shell, "rust-objcopy -O binary target/riscv64imac-unknown-none-elf/release/vanadium target/riscv64imac-unknown-none-elf/release/vanadium.bin --set-start 0x80000000");
            cmd.set_quiet(quiet);
            cmd.set_ignore_stdout(quiet);
            cmd.set_ignore_stderr(quiet);
            cmd.run()?;

            shell.copy_file("target/riscv64imac-unknown-none-elf/release/vanadium.bin", "../../build/vanadium.bin")?;
        }
        BuildTarget::OpenSBI(_) => {
            let mut cmd = cmd!(shell, "rust-objcopy -O binary src/kernel/target/riscv64imac-unknown-none-elf/release/vanadinite src/kernel/target/riscv64imac-unknown-none-elf/release/vanadinite.bin --set-start 0x80200000");
            cmd.set_quiet(quiet);
            cmd.set_ignore_stdout(quiet);
            cmd.set_ignore_stderr(quiet);
            cmd.run()?;

            let mut cmd = cmd!(shell, "git submodule init submodules/opensbi");
            cmd.set_quiet(quiet);
            cmd.set_ignore_stdout(quiet);
            cmd.set_ignore_stderr(quiet);
            cmd.run()?;

            let mut cmd = cmd!(shell, "git submodule update --remote submodules/opensbi");
            cmd.set_quiet(quiet);
            cmd.set_ignore_stdout(quiet);
            cmd.set_ignore_stderr(quiet);
            cmd.run()?;

            let _dir = shell.push_dir("./submodules/opensbi");

            let mut cmd = cmd!(shell, "make PLATFORM=generic LLVM=1 FW_PAYLOAD_PATH=../../src/kernel/target/riscv64imac-unknown-none-elf/release/vanadinite.bin");
            cmd.set_quiet(quiet);
            cmd.set_ignore_stdout(quiet);
            cmd.set_ignore_stderr(quiet);
            cmd.run()?;

            shell.copy_file(
                "build/platform/generic/firmware/fw_jump.bin",
                "../../build/opensbi-riscv64-generic-fw_jump.bin",
            )?;
            shell.copy_file(
                "build/platform/generic/firmware/fw_jump.elf",
                "../../build/opensbi-riscv64-generic-fw_jump.elf",
            )?;
            shell.copy_file(
                "build/platform/generic/firmware/fw_payload.bin",
                "../../build/opensbi-riscv64-generic-fw_payload.bin",
            )?;
            shell.copy_file(
                "build/platform/generic/firmware/fw_payload.elf",
                "../../build/opensbi-riscv64-generic-fw_payload.elf",
            )?;
        }
        BuildTarget::Spike => {
            let mut cmd = cmd!(shell, "git submodule init submodules/riscv-isa-sim");
            cmd.set_quiet(quiet);
            cmd.set_ignore_stdout(quiet);
            cmd.set_ignore_stderr(quiet);
            cmd.run()?;

            let mut cmd = cmd!(shell, "git submodule update --remote submodules/riscv-isa-sim");
            cmd.set_quiet(quiet);
            cmd.set_ignore_stdout(quiet);
            cmd.set_ignore_stderr(quiet);
            cmd.run()?;

            let _dir = shell.push_dir("./submodules/riscv-isa-sim");

            let mut cmd = cmd!(shell, "mkdir -p build");
            cmd.set_quiet(quiet);
            cmd.set_ignore_stdout(quiet);
            cmd.set_ignore_stderr(quiet);
            cmd.run()?;

            let _dir = shell.push_dir("./build");

            let mut cmd = cmd!(shell, "../configure");
            cmd.set_quiet(quiet);
            cmd.set_ignore_stdout(quiet);
            cmd.set_ignore_stderr(quiet);
            cmd.run()?;

            let mut cmd = cmd!(shell, "make");
            cmd.set_quiet(quiet);
            cmd.set_ignore_stdout(quiet);
            cmd.set_ignore_stderr(quiet);
            cmd.run()?;

            shell.copy_file("spike", "../../../build/spike")?;
        }
    }

    Ok(())
}

fn check_llvm_tools(shell: &Shell) -> anyhow::Result<()> {
    match cmd!(shell, "rust-objdump --help").quiet().ignore_stdout().ignore_stderr().run() {
        Err(_) => {
            print!("It does not seem like you have `cargo-binutils` installed, would you like me to add the rustup component and install `cargo-binutils` for you? [Y/n]: ");
            std::io::stdout().lock().flush().unwrap();
            let mut resp = String::new();
            std::io::stdin().read_line(&mut resp).context("Failed to read llvm-tools request response")?;

            match resp.trim() {
                "" | "y" | "yes" | "YES" => {
                    tracing::info!("Installing llvm-tools-preview component...");
                    cmd!(shell, "rustup component add llvm-tools-preview")
                        .run()
                        .context("Failed to add rustup llvm-tools-preview component")?;
                    tracing::info!("Installing `cargo-binutils`...");
                    cmd!(shell, "cargo install cargo-binutils").run().context("Failed to install `cargo-binutils`")?;

                    Ok(())
                }
                "n" | "no" | "NO" => {
                    Err(anyhow::anyhow!("Build cannot continue without `cargo-binutils` being installed"))
                }
                resp => Err(anyhow::anyhow!("Unknown response to request: {}", resp)),
            }
        }
        Ok(_) => Ok(()),
    }
}
