use crate::{root, Env, Result};
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

    fn runtime_env(self, env: &Env) -> Vec<xshell::Pushenv> {
        match self {
            Target::Userspace => vec![],
            Target::Vanadinite => {
                vec![pushenv("RUSTFLAGS", format!("-C link-arg=-Tvanadinite/lds/{}.lds", env.machine))]
            }
            Target::OpenSBI => vec![],
        }
    }
}

pub fn build(target: Target, env: &Env) -> Result<()> {
    let _env = target.predefined_env();
    let _env2 = target.runtime_env(env);

    let features = format!("{} {}", env.machine, env.additional_features);

    match target {
        Target::Userspace => {
            let init_tar = root().join("initfs.tar");

            rm_rf(&init_tar)?;

            let _dir = pushd("./userspace")?;
            cmd!("cargo build --release --workspace").run()?;

            println!("{}", xshell::cwd()?.display());

            let out = fs::File::create(init_tar)?;
            let mut archive = Builder::new(out);

            for (bin, path) in walkdir::WalkDir::new("./target/riscv64gc-unknown-none-elf/release/")
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
        }
        Target::Vanadinite => {
            let manifest = root().join("src/vanadinite/Cargo.toml");
            #[rustfmt::skip]
            cmd!("
                cargo build
                    -p vanadinite
                    --release
                    --target riscv64gc-unknown-none-elf
                    --manifest-path {manifest}
                    --no-default-features
                    --features {features}
            ").run()?;
        }
        Target::OpenSBI => {
            cmd!("git submodule init submodules/opensbi").run()?;
            cmd!("git submodule update --remote submodules/opensbi").run()?;
            let _dir = pushd("./submodules/opensbi")?;

            cmd!("make PLATFORM=generic").run()?;

            cp("build/platform/generic/firmware/fw_jump.bin", "../../opensbi-riscv64-generic-fw_jump.bin")?;
        }
    }

    Ok(())
}
