#![allow(clippy::disallowed_methods)]

use std::path::PathBuf;
use std::process::Command;

/// Returns the macOS Swift runtime directory from the active Xcode toolchain,
/// e.g. `.../XcodeDefault.xctoolchain/usr/lib/swift-5.5/macosx`.
fn swift_runtime_path() -> Option<String> {
    let swiftc = Command::new("xcrun")
        .args(["--find", "swiftc"])
        .output()
        .ok()?;
    let swiftc_path = String::from_utf8(swiftc.stdout).ok()?;
    let swiftc_bin: PathBuf = swiftc_path.trim().into();
    // swiftc → usr/bin/swiftc; go up to usr, then into lib/swift-5.5/macosx
    let usr = swiftc_bin.parent()?.parent()?;
    let candidate = usr.join("lib/swift-5.5/macosx");
    if candidate.exists() {
        Some(candidate.to_string_lossy().into_owned())
    } else {
        None
    }
}

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default() != "macos" {
        return;
    }

    // Embed rpaths so dyld can find libswift_Concurrency.dylib and other
    // Swift runtime dylibs at launch.  These must be emitted from the *root*
    // package build script — cargo:rustc-link-arg from a dependency's
    // build.rs is NOT propagated to the final binary linker invocation.
    println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");

    if let Some(toolchain_swift) = swift_runtime_path() {
        println!("cargo:rustc-link-arg=-Wl,-rpath,{toolchain_swift}");
    }
}
