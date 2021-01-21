use crate::{root, EnvArgs, Result};
use std::fs;
use tar::{Builder, Header};
use xshell::{cmd, cp, pushd, pushenv, rm_rf};

#[derive(Clone, Copy)]
pub enum Target {
    Userspace,
    Vanadinite,
    OpenSBI,
}

impl Target {
    fn predefined_env(self) -> Vec<xshell::Pushenv> {
        match self {
            Target::Userspace => vec![],
            Target::Vanadinite => vec![pushenv("RUSTFLAGS", "-C code-model=medium")],
            Target::OpenSBI => {
                vec![pushenv("CROSS_COMPILE", "riscv64-unknown-elf-"), pushenv("PLATFORM_RISCV_XLEN", "64")]
            }
        }
    }

    fn runtime_env(self, env: &EnvArgs) -> Vec<xshell::Pushenv> {
        match self {
            Target::Userspace => vec![],
            Target::Vanadinite => {
                vec![pushenv("RUSTFLAGS", format!("-C link-arg=-Tvanadinite/lds/{}.lds", env.machine()))]
            }
            Target::OpenSBI => vec![],
        }
    }
}

pub fn build(target: Target, env: &EnvArgs) -> Result<()> {
    let _env = target.predefined_env();
    let _env2 = target.runtime_env(env);

    let mut features = vec![env.machine()];
    if let Some(additional) = env.additional_features() {
        features.push(additional);
    }

    match target {
        Target::Userspace => {
            let init_tar = root().join("initfs.tar");

            rm_rf(&init_tar)?;

            let _dir = pushd("./userspace")?;
            cmd!("cargo build --release --workspace").run()?;

            println!("{}", xshell::cwd()?.display());

            let out = fs::File::create(init_tar)?;
            let mut archive = Builder::new(out);
            let mut header = Header::new_ustar();

            for (bin, path) in walkdir::WalkDir::new("./target/riscv64gc-unknown-none-elf/release/")
                .max_depth(1)
                .into_iter()
                .filter_entry(|e| !e.file_name().to_str().map(|s| s.starts_with('.')).unwrap_or(false))
                .filter_map(|e| e.ok())
                .map(|e| e.into_path())
                .filter(|e| e.is_file() && e.extension().is_none())
                .map(|p| (fs::read(&p), p))
            {
                let bin = std::io::Cursor::new(bin?);
                let path = path.file_name().unwrap();

                archive.append_data(&mut header, path, bin)?;
            }

            archive.finish()?;
        }
        Target::Vanadinite => {
            let manifest = root().join("src/vanadinite/Cargo.toml");
            cmd!(
                "cargo build -p vanadinite --release --target riscv64gc-unknown-none-elf 
                --manifest-path {manifest} --no-default-features --features {features...}
                "
            )
            .run()?;
        }
        Target::OpenSBI => {
            cmd!("git submodule init submodules/opensbi").run()?;
            cmd!("git submodule update --remote submodules/opensbi").run()?;
            let _dir = pushd("./submodules/opensbi")?;

            cmd!("make PLATFORM=generic").run()?;

            cp("build/platform/generic/firmware/fw_dynamic.bin", "../../opensbi-riscv64-generic-fw_dynamic.bin")?;
        }
    }

    Ok(())
}
