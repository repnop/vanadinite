// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.

use std::fs::read_dir;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    for file in read_dir("lds")? {
        let file = file?;

        if file.file_type()?.is_dir() {
            continue;
        }

        println!("cargo:rerun-if-changed={}", file.file_name().into_string().unwrap());
    }

    Ok(())
}
