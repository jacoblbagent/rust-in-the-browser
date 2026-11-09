// Capture real toolchain facts at compile time so the page can report what
// actually produced the .wasm instead of a hardcoded string.
use std::process::Command;

fn main() {
    let rustc = Command::new("rustc")
        .arg("--version")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    println!("cargo:rustc-env=RUSTC_VERSION={}", rustc);
    println!("cargo:rustc-env=BUILD_EPOCH={}", now_epoch());
    println!("cargo:rerun-if-changed=build.rs");
}

// Seconds since the epoch, without pulling a date crate into the build.
fn now_epoch() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
