// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

pub mod build;
pub mod runner;

use build::{BuildTarget, Platform};
use clap::{AppSettings, ArgSettings, Clap};
use runner::RunOptions;
use std::sync::{atomic::AtomicBool, Arc};
use xshell::{pushd, rm_rf};

pub type Result<T> = anyhow::Result<T>;

#[derive(Clap)]
#[clap(rename_all = "snake_case", setting = AppSettings::DisableVersion, setting = AppSettings::VersionlessSubcommands)]
enum Arguments {
    /// Build the various components needed to work with `vanadinite`
    Build(BuildTarget),
    /// Clean all or specific components
    Clean(CleanTarget),
    /// Run `vanadinite`
    Run(RunOptions),
    /// Test `vanadinite`
    Test(RunOptions),
}

#[derive(Clap)]
enum CleanTarget {
    All,
    #[clap(name = "opensbi")]
    OpenSBI,
    Spike,
    Userspace,
    Vanadinite,
}

#[derive(Clap, Clone)]
pub struct VanadiniteBuildOptions {
    /// The platform to target for the `vanadinite` build
    #[clap(arg_enum, long, default_value = "virt")]
    platform: Platform,

    /// Extra kernel features to enable, space separated
    #[clap(setting = ArgSettings::AllowEmptyValues)]
    #[clap(long, default_value = "")]
    kernel_features: String,

    #[clap(skip)]
    test: bool,
}

#[derive(Clap, Clone, Copy)]
#[clap(rename_all = "snake_case")]
pub enum Simulator {
    /// The RISC-V ISA simulator Spike
    Spike,
    Qemu,
}

fn main() -> Result<()> {
    let args = Arguments::parse();

    let _sig = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&_sig)).unwrap();

    match args {
        Arguments::Build(target) => build::build(target)?,
        Arguments::Clean(target) => clean(target)?,
        Arguments::Run(target) => runner::run(target)?,
        Arguments::Test(target) => runner::test(target)?,
    }

    Ok(())
}

fn clean(target: CleanTarget) -> Result<()> {
    match target {
        CleanTarget::All => {
            clean(CleanTarget::OpenSBI)?;
            clean(CleanTarget::Spike)?;
            clean(CleanTarget::Userspace)?;
            clean(CleanTarget::Vanadinite)?;
        }
        CleanTarget::OpenSBI => {
            let _dir = pushd("./submodules/opensbi")?;
            rm_rf("./build")?;
            println!("Cleaned OpenSBI");
        }
        CleanTarget::Spike => {
            let _dir = pushd("./submodules/riscv-isa-sim")?;
            rm_rf("./build")?;
            println!("Cleaned Spike");
        }
        CleanTarget::Userspace => {
            let _dir = pushd("./userspace")?;
            rm_rf("./target")?;
            println!("Cleaned userspace");
        }
        CleanTarget::Vanadinite => {
            let _dir = pushd("./src")?;
            rm_rf("./target")?;
            println!("Cleaned vanadinite");
        }
    }

    Ok(())
}
