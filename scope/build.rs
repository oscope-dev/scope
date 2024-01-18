use anyhow::Result;
use vergen::EmitBuilder;

pub fn main() -> Result<()> {
    EmitBuilder::builder().all_build().all_git().emit()?;

    if let Ok(version) = std::env::var("VERSION") {
        println!("cargo:rustc-env=SCOPE_VERSION={}", version);
    } else {
        println!("cargo:rustc-env=SCOPE_VERSION=0.0.1");
    }
    Ok(())
}
