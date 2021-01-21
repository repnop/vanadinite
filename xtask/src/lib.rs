use std::{
    collections::HashMap,
    env,
    path::{Path, PathBuf},
};

use xshell::{cmd, pushd};

pub mod build;
pub mod runner;

pub const ENV_LIST: &[&str] = &["MACHINE", "RAM", "CPUS", "ADDITIONAL_FEATURES", "KARGS"];
const ENV_DEFAULTS: &[(&str, &str)] =
    &[("MACHINE", "virt"), ("RAM", "512M"), ("CPUS", "5"), ("ADDITIONAL_FEATURES", ""), ("KARGS", "")];

pub struct EnvArgs(HashMap<String, String>);

impl EnvArgs {
    pub fn machine(&self) -> &str {
        self.0.get("MACHINE").unwrap()
    }

    pub fn ram(&self) -> &str {
        self.0.get("RAM").unwrap()
    }

    pub fn cpus(&self) -> &str {
        self.0.get("CPUS").unwrap()
    }

    pub fn additional_features(&self) -> Option<&str> {
        let inner = self.0.get("ADDITIONAL_FEATURES").unwrap();

        if inner.is_empty() {
            None
        } else {
            Some(inner.as_str())
        }
    }

    pub fn kernel_args(&self) -> &str {
        self.0.get("KARGS").unwrap()
    }
}

impl EnvArgs {
    pub fn new(mut from_env: HashMap<String, String>) -> Self {
        for (var, default) in ENV_DEFAULTS {
            if from_env.contains_key(*var) {
                continue;
            }
            from_env.insert(var.to_string(), default.to_string());
        }

        Self(from_env)
    }
}

pub type Result<T> = anyhow::Result<T>;

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
