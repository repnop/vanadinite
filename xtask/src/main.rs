// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

pub mod build;
pub mod runner;

use anyhow::Context;
use build::{BuildTarget, Platform};
use clap::{Parser, ValueEnum};
use runner::RunOptions;
use std::sync::{atomic::AtomicBool, Arc};
use tracing_subscriber::prelude::*;
use xshell::Shell;

pub type Result<T> = anyhow::Result<T>;

#[derive(Parser)]
#[clap(rename_all = "snake_case")]
enum Arguments {
    /// Build the various components needed to work with `vanadinite`
    Build {
        #[command(subcommand)]
        target: BuildTarget,
        #[arg(long, short, default_value = "false")]
        quiet: bool,
    },
    /// Clean all or specific components
    Clean {
        #[clap(value_enum)]
        target: CleanTarget,
    },
    /// Run `vanadinite`
    Run(RunOptions),
    /// Test `vanadinite`
    Test(RunOptions),
}

#[derive(ValueEnum, Clone, Copy)]
enum CleanTarget {
    All,
    #[clap(name = "opensbi")]
    OpenSBI,
    Spike,
    Userspace,
    Vanadinite,
    Vanadium,
}

#[derive(Parser, Clone)]
pub struct VanadiniteBuildOptions {
    /// The platform to target for the `vanadinite` build
    #[arg(value_enum, long, default_value = "virt")]
    platform: Platform,

    /// Extra kernel features to enable, space separated
    #[arg(long, default_value = "")]
    kernel_features: String,

    #[arg(skip)]
    test: bool,

    #[arg(long)]
    debug_build: bool,
}

#[derive(ValueEnum, Clone, Copy)]
#[clap(rename_all = "snake_case")]
pub enum Simulator {
    /// The RISC-V ISA simulator Spike
    Spike,
    Qemu,
}

#[derive(ValueEnum, Clone, Copy)]
#[clap(rename_all = "snake_case")]
pub enum SbiImpl {
    /// The RISC-V reference SBI implementation
    #[clap(name = "opensbi")]
    OpenSbi,
    /// In house custom SBI for `vanadinite`
    Vanadium,
}

fn main() -> Result<()> {
    let args = Arguments::parse();

    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let _sig = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&_sig)).unwrap();
    let shell = Shell::new().context("Unable to create shell instance")?;

    match args {
        Arguments::Build { target, quiet } => build::build(&shell, target, quiet)?,
        Arguments::Clean { target } => clean(&shell, target)?,
        Arguments::Run(target) => runner::run(&shell, target)?,
        Arguments::Test(target) => runner::test(&shell, target)?,
    }

    Ok(())
}

fn clean(shell: &Shell, target: CleanTarget) -> Result<()> {
    match target {
        CleanTarget::All => {
            clean(shell, CleanTarget::OpenSBI)?;
            clean(shell, CleanTarget::Spike)?;
            clean(shell, CleanTarget::Userspace)?;
            clean(shell, CleanTarget::Vanadinite)?;
            clean(shell, CleanTarget::Vanadium)?;
        }
        CleanTarget::OpenSBI => {
            let _dir = shell.push_dir("./submodules/opensbi");
            shell.remove_path("./build")?;
            println!("Cleaned OpenSBI");
        }
        CleanTarget::Spike => {
            let _dir = shell.push_dir("./submodules/riscv-isa-sim");
            shell.remove_path("./build")?;
            println!("Cleaned Spike");
        }
        CleanTarget::Userspace => {
            let _dir = shell.push_dir("./src/userspace");
            shell.remove_path("./target")?;
            println!("Cleaned userspace");
        }
        CleanTarget::Vanadinite => {
            let _dir = shell.push_dir("./src/kernel");
            shell.remove_path("./target")?;
            println!("Cleaned vanadinite");
        }
        CleanTarget::Vanadium => {
            let _dir = shell.push_dir("./src/vanadium");
            shell.remove_path("./target")?;
            println!("Cleaned vanadium");
        }
    }

    Ok(())
}
