use clap::Clap;
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
    /// Build userspace and pack executables into tar file in the root directory
    Userspace,
    /// Build `vanadinite`
    Vanadinite(Env),
}

fn main() -> Result<()> {
    let args = Arguments::parse();
    let _working_dir = pushd(xtask::root())?;

    match args {
        Arguments::Vanadinite(env) => build::build(BuildTarget::Vanadinite, &env)?,
        Arguments::Userspace => build::build(BuildTarget::Userspace, &Env::default())?,
        Arguments::OpenSBI => build::build(BuildTarget::OpenSBI, &Env::default())?,
        Arguments::Debug(env) => runner::run(RunTarget::Debug, &env)?,
        Arguments::Gdb => runner::run(RunTarget::Gdb, &Env::default())?,
        Arguments::Run(env) => runner::run(RunTarget::Run, &env)?,
        Arguments::Clean => xtask::clean()?,
    }

    Ok(())
}
