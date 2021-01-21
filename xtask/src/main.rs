use xshell::pushd;
use xtask::{
    build::{self, Target as BuildTarget},
    runner::{self, Target as RunTarget},
    EnvArgs, Result, ENV_LIST,
};

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let command = args.next().unwrap_or_default();
    let subcommand = args.next().unwrap_or_default();

    let env_parts = std::env::vars()
        // TODO: Is this really needed?
        .filter(|(key, _)| ENV_LIST.contains(&key.as_str()))
        .collect();

    let env = EnvArgs::new(env_parts);

    let _working_dir = pushd(xtask::root())?;

    match command.as_str() {
        "v" | "vanadinite" => build::build(BuildTarget::Vanadinite, &env)?,
        "u" | "userspace" => build::build(BuildTarget::Userspace, &env)?,
        "opensbi" => build::build(BuildTarget::OpenSBI, &env)?,
        "debug" => runner::run(RunTarget::Debug, &env, subcommand)?,
        "gdb" => runner::run(RunTarget::Gdb, &env, subcommand)?,
        "run" => runner::run(RunTarget::Run, &env, subcommand)?,
        "c" | "clean" => xtask::clean()?,

        _ => anyhow::bail!("Unknown command provided!"),
    }

    Ok(())
}
