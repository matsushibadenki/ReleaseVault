use std::{env, path::PathBuf, process::Command};

fn main() {
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        build_device_unlock();
    }
    tauri_build::build()
}

fn build_device_unlock() {
    const SOURCE: &str = "native/DeviceUnlock.swift";
    println!("cargo:rerun-if-changed={SOURCE}");
    println!("cargo:rerun-if-env-changed=MACOSX_DEPLOYMENT_TARGET");
    println!("cargo:rerun-if-changed=native/Passkey.swift");
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let arch = match env::var("CARGO_CFG_TARGET_ARCH").unwrap().as_str() {
        "aarch64" => "arm64",
        "x86_64" => "x86_64",
        arch => panic!("Unsupported macOS architecture: {arch}"),
    };
    let version = env::var("MACOSX_DEPLOYMENT_TARGET").unwrap_or_else(|_| "11.0".into());
    let target = format!("{arch}-apple-macosx{version}");
    let sdk = Command::new("xcrun")
        .args(["--sdk", "macosx", "--show-sdk-path"])
        .output()
        .expect("macOS SDK");
    assert!(sdk.status.success(), "macOS SDK required");
    let sdk = String::from_utf8(sdk.stdout).unwrap();
    let status = Command::new("xcrun")
        .args([
            "swiftc",
            "-O",
            "-parse-as-library",
            "-emit-library",
            "-static",
            "-module-name",
            "ReleaseVaultDevice",
            "-sdk",
            sdk.trim(),
            "-target",
            &target,
            "-module-cache-path",
        ])
        .arg(out.join("swift-cache"))
        .arg(SOURCE)
        .arg("native/Passkey.swift")
        .arg("-o")
        .arg(out.join("libReleaseVaultDevice.a"))
        .status()
        .expect("Swift compiler");
    assert!(
        status.success(),
        "Could not compile device authentication support"
    );
    println!("cargo:rustc-link-search=native={}", out.display());
    println!(
        "cargo:rustc-link-search=native={}/usr/lib/swift",
        sdk.trim()
    );
    // Older deployment targets also need Swift's static back-deployment shims.
    let info = Command::new("xcrun")
        .args(["swiftc", "-print-target-info", "-target", &target])
        .output()
        .expect("Swift toolchain paths");
    assert!(info.status.success(), "Cannot locate the Swift runtime");
    let info: serde_json::Value = serde_json::from_slice(&info.stdout).expect("Swift target info");
    for path in info["paths"]["runtimeLibraryPaths"]
        .as_array()
        .expect("Swift runtime libraries")
    {
        println!(
            "cargo:rustc-link-search=native={}",
            path.as_str().expect("Swift runtime path")
        );
    }
    println!("cargo:rustc-link-lib=static=ReleaseVaultDevice");
    println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
}
