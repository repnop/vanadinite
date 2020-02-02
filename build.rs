use std::{
    env,
    fs::{self, File},
    io::Write,
    path::PathBuf,
    process::Command,
};

fn main() {
    // Put the linker script somewhere the linker can find it
    let out = &PathBuf::from(env::var_os("OUT_DIR").unwrap());
    File::create(out.join("link.ld"))
        .unwrap()
        .write_all(include_bytes!("link.ld"))
        .unwrap();
    println!("cargo:rerun-if-changed=link.ld");
    println!("cargo:rustc-link-search={}", out.display());

    for entry in fs::read_dir("./src/asm/").unwrap() {
        let entry = entry.unwrap();
        let filename = entry.file_name().into_string().unwrap();
        let no_ext = filename
            .rfind('.')
            .map(|p| &filename[..p])
            .unwrap_or(&filename);

        File::create(out.join(&filename))
            .unwrap()
            .write_all(&fs::read(entry.path()).unwrap())
            .unwrap();
        assert!(Command::new("riscv64-unknown-elf-as")
            .current_dir(out)
            .args(&[
                &filename,
                "-o",
                &format!("{}.o", no_ext),
                "-march=rv64gc",
                "-mabi=lp64",
            ])
            .status()
            .expect("failed to run assembler")
            .success());
        assert!(Command::new("riscv64-unknown-elf-ar")
            .current_dir(out)
            .args(&["crs", &format!("lib{}.a", no_ext), &format!("{}.o", no_ext)])
            .status()
            .expect("failed to run ar")
            .success());
        println!("cargo:rerun-if-changed=./src/asm/{}", filename);
        println!("cargo:rustc-link-lib=static={}", no_ext);
    }
}
