use anyhow::Result;

pub fn main() -> Result<()> {
    if let Ok(version) = std::env::var("VERSION") {
        println!("cargo:rustc-env=SCOPE_VERSION={}", version);
    } else {
        println!("cargo:rustc-env=SCOPE_VERSION=0.0.1");
    }
    Ok(())
}
