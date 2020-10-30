use std::fs::read_dir;

const VIRT: &str = "CARGO_FEATURE_VIRT";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    for file in read_dir("lds")? {
        let file = file?;

        if file.file_type()?.is_dir() {
            continue;
        }

        println!("cargo:rerun-if-changed={}", file.file_name().into_string().unwrap());
    }

    match std::env::var(VIRT) {
        Ok(_) => println!(r#"cargo:rustc-cfg=feature="virt""#),
        Err(_) => {}
    }

    Ok(())
}
