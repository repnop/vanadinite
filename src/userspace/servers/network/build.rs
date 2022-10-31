// SPDX-License-Identifier: MPL-2.0
// SPDX-FileCopyrightText: 2022 The vanadinite developers
//
// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

fn main() {
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
    for file in std::fs::read_dir("vidl/").unwrap() {
        let file = file.unwrap();
        let out_file_name = file.file_name().into_string().unwrap().replace(".vidl", ".rs");
        println!("cargo:rerun-if-changed=vidl/{}", file.file_name().into_string().unwrap());
        let source = std::fs::read_to_string(&file.path()).unwrap();
        match vidlgen::Compiler::new().compile(&source) {
            Ok(out) => std::fs::write(out_dir.join(out_file_name), out.to_string()).unwrap(),
            Err(e) => {
                eprintln!("Error parsing {}", file.path().display());
                eprintln!("{}", e.display_with(&source));
                std::process::exit(1);
            }
        }
    }
}
