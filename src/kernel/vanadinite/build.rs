// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let init = std::path::PathBuf::from(std::env::var("CARGO_BIN_FILE_INIT").unwrap());
    let init_dumped = init.parent().unwrap().with_file_name("init.bin");

    std::process::Command::new("riscv64-unknown-elf-objcopy")
        .args(&["-O", "binary", init.to_str().unwrap(), init_dumped.to_str().unwrap(), "--set-start", "0xF00D0000"])
        .spawn()?;

    println!("cargo:rustc-env=CARGO_BIN_FILE_INIT={}", init_dumped.display());
    println!("cargo:rustc-link-arg=-Tvanadinite/lds/{}.lds", std::env::var("VANADINITE_TARGET_PLATFORM").unwrap());
    Ok(())
}
