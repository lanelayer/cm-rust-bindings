// Build script for cartesi-machine-wasm
// 
// Two modes:
// 1. With WASM_LIBCARTESI_PATH set: skip C++ compilation, link to prebuilt libcartesi.a
// 2. Otherwise: compile libcartesi from machine-emulator source using wasi-sdk
//
// Mode 1 is used for fast iteration after the initial full build.

use std::env;
use std::path::PathBuf;

fn main() {
    // Check for prebuilt libcartesi
    if let Ok(prebuilt) = env::var("WASM_LIBCARTESI_PATH") {
        let lib_path = PathBuf::from(&prebuilt);
        if lib_path.join("libcartesi.a").exists() {
            println!("cargo:warning=Using prebuilt libcartesi.a from {}", prebuilt);
            println!("cargo:rustc-link-search={}", prebuilt);
            println!("cargo:rustc-link-lib=static=cartesi");
            return;
        }
    }

    // Fallback: compile from source using wasi-sdk
    // (This requires wasi-sdk + machine-emulator source)
    compile_from_source();
}

#[allow(unused)]
fn find_wasi_sdk() -> PathBuf {
    if let Ok(p) = env::var("WASI_SDK_PATH") {
        let pb = PathBuf::from(&p);
        if pb.join("bin/clang++").exists() {
            return pb;
        }
    }
    for candidate in &["/opt/wasi-sdk"] {
        let pb = PathBuf::from(candidate);
        if pb.join("bin/clang++").exists() {
            return pb;
        }
    }
    if let Ok(entries) = std::fs::read_dir("/tmp") {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("wasi-sdk-") && entry.path().join("bin/clang++").exists() {
                return entry.path();
            }
        }
    }
    panic!("wasi-sdk not found. Set WASI_SDK_PATH or install to /opt/wasi-sdk");
}

#[allow(unused)]
fn compile_from_source() {
    let wasi_sdk = find_wasi_sdk();
    let emu_src = if let Ok(p) = env::var("MACHINE_EMULATOR_PATH") {
        PathBuf::from(p)
    } else {
        PathBuf::from("../../machine-emulator")
    };
    let emu_src = emu_src.canonicalize().expect("machine-emulator not found");

    println!("cargo:warning=Compiling libcartesi for wasm32-wasi from source...");
    println!(
        "cargo:warning=Run with WASM_LIBCARTESI_PATH set to skip this step."
    );

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let wasm_obj_dir = out_dir.join("wasm-obj");
    std::fs::create_dir_all(&wasm_obj_dir).unwrap();

    let sysroot = wasi_sdk.join("share/wasi-sysroot");
    let third_party = emu_src.join("third-party");

    // Compile all C++ sources, archive into libcartesi.a, output to OUT_DIR
    // (...production implementation would go here...)
    
    panic!(
        "Source compilation not yet automated in build.rs. \
         Use WASM_LIBCARTESI_PATH=/tmp/wasm_full to point to prebuilt libcartesi.a"
    );
}
