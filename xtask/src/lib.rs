pub mod build;
pub mod runner;

use clap::{ArgSettings, Clap};
use std::{
    env,
    fmt::{self, Display},
    path::{Path, PathBuf},
};
use xshell::{cmd, pushd};

pub type Result<T> = anyhow::Result<T>;

#[derive(Clap)]
pub struct Env {
    #[clap(arg_enum, long, env = "MACHINE", default_value = "virt")]
    machine: Machine,

    #[clap(long, env = "RAM", default_value = "512M")]
    ram: String,

    #[clap(long, env = "CPUS", default_value = "5")]
    cpus: usize,

    #[clap(setting = ArgSettings::AllowEmptyValues)]
    #[clap(long, env = "KARGS", default_value = "")]
    kernel_args: String,

    #[clap(setting = ArgSettings::AllowEmptyValues)]
    #[clap(long, env = "ADDITIONAL_FEATURES", default_value = "")]
    additional_features: String,
}

impl Default for Env {
    fn default() -> Self {
        Self {
            machine: Machine::Virt,
            ram: String::from("512M"),
            cpus: 5,
            kernel_args: String::new(),
            additional_features: String::new(),
        }
    }
}

#[derive(Clap, Clone, Copy)]
#[clap(rename_all = "snake_case")]
pub enum Machine {
    Virt,
    SifiveU,
}

impl Display for Machine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Machine::Virt => write!(f, "virt"),
            Machine::SifiveU => write!(f, "sifive_u"),
        }
    }
}

pub fn root() -> PathBuf {
    Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| env!("CARGO_MANIFEST_DIR").to_owned()))
        .ancestors()
        .nth(1)
        .unwrap()
        .to_path_buf()
}

pub fn clean() -> Result<()> {
    // Userspace
    {
        let _dir = pushd("./userspace")?;
        cmd!("cargo clean").run()?;
    }

    Ok(())
}
