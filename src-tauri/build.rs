fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        build_apple_bridge();
    }
    tauri_build::build()
}

/// Compiles `swift/BlurtApple.swift` into a static library and links it. FoundationModels
/// is weak-linked so the app still launches on macOS versions that don't have it.
/// (Approach adapted from Handy, MIT: https://github.com/cjpais/Handy)
fn build_apple_bridge() {
    use std::path::{Path, PathBuf};
    use std::process::Command;

    const SRC: &str = "swift/BlurtApple.swift";
    println!("cargo:rerun-if-changed={SRC}");

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let object = out_dir.join("blurt_apple.o");
    let lib = out_dir.join("libblurt_apple.a");

    let run = |cmd: &str, args: &[&str]| -> String {
        let out = Command::new(cmd)
            .args(args)
            .output()
            .unwrap_or_else(|e| panic!("{cmd}: {e}"));
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    };
    let sdk = std::env::var("SDKROOT")
        .unwrap_or_else(|_| run("xcrun", &["--sdk", "macosx", "--show-sdk-path"]));
    let swiftc = std::env::var("SWIFTC").unwrap_or_else(|_| run("xcrun", &["--find", "swiftc"]));
    let arch = match std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
        Ok("x86_64") => "x86_64",
        _ => "arm64",
    };
    let target = format!("{arch}-apple-macosx13.0");

    let status = Command::new(&swiftc)
        .args([
            "-parse-as-library",
            "-swift-version",
            "5",
            "-target",
            &target,
            "-sdk",
            &sdk,
            "-O",
            "-c",
            SRC,
            "-o",
            object.to_str().unwrap(),
        ])
        .status()
        .expect("failed to run swiftc");
    assert!(status.success(), "swiftc failed to compile {SRC}");

    let status = Command::new("libtool")
        .args([
            "-static",
            "-o",
            lib.to_str().unwrap(),
            object.to_str().unwrap(),
        ])
        .status()
        .expect("failed to run libtool");
    assert!(status.success(), "libtool failed");

    let toolchain_swift = Path::new(&swiftc)
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("lib/swift/macosx");
    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=blurt_apple");
    println!(
        "cargo:rustc-link-search=native={}",
        toolchain_swift.display()
    );
    println!(
        "cargo:rustc-link-search=native={}",
        Path::new(&sdk).join("usr/lib/swift").display()
    );
    println!("cargo:rustc-link-lib=framework=Foundation");
    println!("cargo:rustc-link-lib=framework=AVFoundation");
    if Path::new(&sdk)
        .join("System/Library/Frameworks/FoundationModels.framework")
        .exists()
    {
        println!("cargo:rustc-link-arg=-weak_framework");
        println!("cargo:rustc-link-arg=FoundationModels");
    }
    println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
}
