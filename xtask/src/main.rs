// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2021 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use clap::Clap;
use std::sync::{atomic::AtomicBool, Arc};
use xshell::pushd;
use xtask::{
    build::{self, Target as BuildTarget},
    runner::{self, Target as RunTarget},
    Env, Result,
};

#[derive(Clap)]
#[clap(rename_all = "snake_case")]
enum Arguments {
    /// Clean all projects
    Clean,
    /// Build `vanadinite`, run QEMU, and wait for GDB
    Debug(Env),
    /// Start GDB and connect to a running QEMU instance
    Gdb,
    /// Build the OpenSBI firmware image and copy it to the root directory
    #[clap(name = "opensbi")]
    OpenSBI,
    /// Build `vanadinite` and run QEMU
    Run(Env),
    /// Build the RISC-V ISA simulator `spike`
    Spike,
    /// Build userspace and pack executables into tar file in the root directory
    Userspace,
    /// Build `vanadinite`
    Vanadinite(Env),
}

fn main() -> Result<()> {
    let args = Arguments::parse();
    let _working_dir = pushd(xtask::root())?;

    let _sig = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&_sig)).unwrap();

    match args {
        Arguments::Vanadinite(env) => build::build(BuildTarget::Vanadinite, &env)?,
        Arguments::Userspace => build::build(BuildTarget::Userspace, &Env::default())?,
        Arguments::OpenSBI => build::build(BuildTarget::OpenSBI, &Env::default())?,
        Arguments::Spike => build::build(BuildTarget::Spike, &Env::default())?,
        Arguments::Debug(env) => runner::run(RunTarget::Debug, &env)?,
        Arguments::Gdb => runner::run(RunTarget::Gdb, &Env::default())?,
        Arguments::Run(env) => runner::run(RunTarget::Run, &env)?,
        Arguments::Clean => xtask::clean()?,
    }

    Ok(())
}
