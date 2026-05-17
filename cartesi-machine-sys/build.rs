// (c) Cartesi and individual authors (see AUTHORS)
// SPDX-License-Identifier: Apache-2.0 (see LICENSE)
use std::{env, path::PathBuf, process::Command, fs};

fn get_machine_dir_path() -> PathBuf {
    // 1. Check CARTESI_MACHINE_SOURCES
    if let Ok(env_path) = env::var("CARTESI_MACHINE_SOURCES") {
        let pb = PathBuf::from(&env_path);
        if pb.exists() {
            return pb.canonicalize().expect("cannot canonicalize CARTESI_MACHINE_SOURCES");
        }
    }
    // 2. Check ../../emulator
    let default_path = PathBuf::from("../../emulator");
    if default_path.exists() {
        return default_path.canonicalize().expect("cannot canonicalize ../../emulator");
    }
    // 3. For external_cartesi, use the env var location as fallback
    #[cfg(feature = "external_cartesi")]
    {
        if let Ok(env_path) = env::var("INCLUDECARTESI_PATH") {
            let pb = PathBuf::from(&env_path);
            if pb.exists() {
                return pb.canonicalize().expect("cannot canonicalize INCLUDECARTESI_PATH");
            }
        }
        // If all else fails with external_cartesi, use the CARTESI_MACHINE_SOURCES or a default
        // The CARTESI_MACHINE_SOURCES was already checked above, so fallback:
        let fallback = PathBuf::from("../../machine-emulator/src");
        if fallback.exists() {
            return fallback.canonicalize().expect("cannot canonicalize fallback");
        }
        panic!("external_cartesi set but no machine-emulator source found. Set CARTESI_MACHINE_SOURCES or INCLUDECARTESI_PATH.");
    }
    #[cfg(not(feature = "external_cartesi"))]
    // 4. Download and extract
    download_and_extract_emulator()
}

#[cfg(not(feature = "external_cartesi"))]
fn download_and_extract_emulator() -> PathBuf {
    use std::io::Cursor;
    use reqwest::blocking::get;
    use flate2::read::GzDecoder;
    use tar::Archive;

    let url = "https://github.com/cartesi/machine-emulator/archive/refs/tags/v0.20.0.tar.gz";

    // Use a stable directory in OUT_DIR instead of a temporary one
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let cache_dir = out_dir.join("emulator_cache");
    let extracted = cache_dir.join("machine-emulator-0.20.0");
    
    // Only download and extract if not already present
    if !extracted.exists() {
        fs::create_dir_all(&cache_dir).expect("failed to create cache dir");
        let response = get(url).expect("failed to download emulator tarball");
        let bytes = response.bytes().expect("failed to read tarball bytes");
        let tar_gz = Cursor::new(bytes);
        let tar = GzDecoder::new(tar_gz);
        let mut archive = Archive::new(tar);
        archive.unpack(&cache_dir).expect("failed to unpack emulator tarball");
    }
    
    assert!(extracted.exists(), "Extracted emulator dir not found");
    extracted.canonicalize().expect("cannot canonicalize extracted emulator dir")
}

fn main() {
    let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());

    // Directory where `libcartesi.a` is located after it's built.
    let machine_dir_path = get_machine_dir_path();

    // Clean build artifacts and start from scratch
    // clean(&machine_dir_path);

    // tell Cargo where to look for libraries
    cfg_if::cfg_if! {
        if #[cfg(feature = "external_cartesi")] {
            let libpath =
                env::var("LIBCARTESI_PATH")
                .map(PathBuf::from)
                .unwrap_or_else(|_| machine_dir_path.join("src"));
            println!("cargo:rustc-link-search={}", libpath.to_str().unwrap());
            
            // Monitor external library files
            let libcartesi_path = libpath.join("libcartesi.a");
            let libcartesi_jsonrpc_path = libpath.join("libcartesi_jsonrpc.a");
            println!("cargo:rerun-if-changed={}", libcartesi_path.display());
            println!("cargo:rerun-if-changed={}", libcartesi_jsonrpc_path.display());
        } else if #[cfg(feature = "wasm32")] {
            build_wasm32::build(&machine_dir_path, &out_path);
            println!("cargo:rustc-link-search={}", out_path.to_str().unwrap());
        } else {
            build_cm::build(&machine_dir_path, &out_path);
            println!("cargo:rustc-link-search={}", out_path.to_str().unwrap());
        }
    }

    // static link
    // println!("cargo:rustc-link-lib=slirp");
    cfg_if::cfg_if! {
        if #[cfg(feature = "remote_machine")] {
            println!("cargo:rustc-link-lib=static=cartesi_jsonrpc");
        } else {
            println!("cargo:rustc-link-lib=static=cartesi");
        }
    }

    //
    //  Generate bindings
    //

    // find headers
    #[allow(clippy::needless_late_init)]
    let include_path;
    cfg_if::cfg_if! {
        if #[cfg(feature = "external_cartesi")] {
            include_path = env::var("INCLUDECARTESI_PATH")
                .map(PathBuf::from)
                .unwrap_or_else(|_| machine_dir_path.join("src"));

        } else {
            include_path = machine_dir_path.join("src");
        }
    };

    // generate machine api
    let mut builder = bindgen::Builder::default()
        .header(include_path.join("machine-c-api.h").to_str().unwrap())
        .allowlist_item("^cm_.*")
        .allowlist_item("^CM_.*")
        .merge_extern_blocks(true)
        .prepend_enum_name(false)
        .translate_enum_integer_types(true);

    // When building for wasm32, use wasi-sdk sysroot for bindgen
    #[cfg(feature = "wasm32")]
    {
        let wasi_sdk = build_wasm32::find_wasi_sdk();
        let sysroot = wasi_sdk.join("share/wasi-sysroot");
        builder = builder.clang_arg(format!("--sysroot={}", sysroot.display()));
        builder = builder.clang_arg("--target=wasm32-wasi");
    }

    let machine_bindings = builder
        .generate()
        .expect("Unable to generate machine bindings");

    // Write the bindings to the `$OUT_DIR/bindings.rs` and `$OUT_DIR/htif.rs` files.
    machine_bindings
        .write_to_file(out_path.join("bindings.rs"))
        .expect("Couldn't write machine bindings");

    // Setup reruns
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={}", include_path.join("machine-c-api.h").display());
    println!("cargo:rerun-if-env-changed=UARCH_PRISTINE_HASH_PATH");
    println!("cargo:rerun-if-env-changed=UARCH_PRISTINE_RAM_PATH");
    println!("cargo:rerun-if-env-changed=LIBCARTESI_PATH");
    println!("cargo:rerun-if-env-changed=INCLUDECARTESI_PATH");
    println!("cargo:rerun-if-env-changed=CARTESI_MACHINE_SOURCES");
}

#[cfg(not(feature = "external_cartesi"))]
mod build_cm {
    use std::{env, fs, path::Path, process::Command};

    #[cfg(not(feature = "wasm32"))]
    pub fn build(machine_dir_path: &Path, out_path: &Path) {
        // Get uarch
        cfg_if::cfg_if! {
            if #[cfg(feature = "build_uarch")] {
                // requires docker
                ()
            } else if #[cfg(feature = "copy_uarch")] {
                let uarch_path = machine_dir_path.join("uarch");
                copy_uarch::copy(&uarch_path)
            } else if #[cfg(feature = "download_uarch")] {
                download_uarch::download(machine_dir_path);
            } else {
                panic!("Internal error, no way specified to get uarch");
            }
        }

        // Monitor uarch files for changes
        let uarch_hash_path = machine_dir_path.join("uarch").join("uarch-pristine-hash.c");
        let uarch_ram_path = machine_dir_path.join("uarch").join("uarch-pristine-ram.c");
        if uarch_hash_path.exists() {
            println!("cargo:rerun-if-changed={}", uarch_hash_path.display());
        }
        if uarch_ram_path.exists() {
            println!("cargo:rerun-if-changed={}", uarch_ram_path.display());
        }

        let libcartesi_path = machine_dir_path.join("src").join("libcartesi.a");
        let libcartesi_dest_path = out_path.join("libcartesi.a");

        let libcartesi_jsonrpc_path = machine_dir_path.join("src").join("libcartesi_jsonrpc.a");
        let libcartesi_jsonrpc_dest_path = out_path.join("libcartesi_jsonrpc.a");

        // Only skip build if BOTH libraries already exist
        if !libcartesi_path.exists() || !libcartesi_jsonrpc_path.exists() {
            //
            // Build and link emulator
            //

            // Get number of parallel jobs from Cargo
            let num_jobs = env::var("NUM_JOBS").unwrap_or_else(|_| "1".to_string());
            let parallel_arg = format!("-j{}", num_jobs);

            // build dependencies
            Command::new("make")
                .args([&parallel_arg, "submodules"])
                .current_dir(machine_dir_path)
                .status()
                .expect("Failed to run setup `make submodules`");
            Command::new("make")
                .args([&parallel_arg, "bundle-boost"])
                .current_dir(machine_dir_path)
                .status()
                .expect("Failed to run `make bundle-boost`");

            // build `libcartesi.a` and `libcartesi_jsonrpc.a`, release, no `libslirp`
            Command::new("make")
                .args([
                    &parallel_arg,
                    "-C",
                    "src",
                    "release=yes",
                    "slirp=no",
                    "libcartesi.a",
                    "libcartesi_jsonrpc.a",
                ])
                .current_dir(machine_dir_path)
                .status()
                .expect("Failed to build `libcartesi.a` and/or `libcartesi_jsonrpc.a`");
        }

        // copy `libcartesi.a` to OUT_DIR
        fs::copy(&libcartesi_path, &libcartesi_dest_path).unwrap_or_else(|_| {
            panic!(
                "Failed to copy `libcartesi.a` {:?} to OUT_DIR {:?}",
                libcartesi_path, libcartesi_dest_path
            )
        });

        // copy `libcartesi_jsonrpc.a` to OUT_DIR
        fs::copy(&libcartesi_jsonrpc_path, &libcartesi_jsonrpc_dest_path).unwrap_or_else(|_| {
            panic!(
                "Failed to copy `libcartesi_jsonrpc.a` {:?} to OUT_DIR {:?}",
                libcartesi_jsonrpc_path, libcartesi_jsonrpc_dest_path
            )
        });

        // Monitor the library files for changes
        println!("cargo:rerun-if-changed={}", libcartesi_path.display());
        println!("cargo:rerun-if-changed={}", libcartesi_jsonrpc_path.display());
    }

    #[cfg(feature = "copy_uarch")]
    mod copy_uarch {
        use std::{env, fs, path::Path};

        fn copy(uarch_path: &Path) {
            let uarch_pristine_hash_path =
                env::var("UARCH_PRISTINE_HASH_PATH").expect("`UARCH_PRISTINE_HASH_PATH` not set");
            let uarch_pristine_ram_path =
                env::var("UARCH_PRISTINE_RAM_PATH").expect("`UARCH_PRISTINE_RAM_PATH` not set");

            fs::copy(
                uarch_pristine_hash_path,
                uarch_path.join("uarch-pristine-hash.c").to_str().unwrap(),
            )
            .expect("Failed to move `uarch-pristine-hash.c` to `uarch/`");

            fs::copy(
                uarch_pristine_ram_path,
                uarch_path.join("uarch-pristine-ram.c").to_str().unwrap(),
            )
            .expect("Failed to move `uarch-pristine-ram.c` to `uarch/`");
        }
    }

    #[cfg(any(feature = "download_uarch", feature = "wasm32"))]
    pub(crate) mod download_uarch {
        use bytes::Bytes;
        use std::{
            fs::{self, OpenOptions},
            io::{self, Read, Write},
            path::Path,
            process::{Command, Stdio},
        };

        const VERSION_STRING: &str = "v0.20.0";

        pub fn download(machine_dir_path: &Path) {
            let patch_file = machine_dir_path.join("add-generated-files.diff");
            let patched_marker = machine_dir_path.join(".patched");

            if patched_marker.exists() {
                let previous = fs::read_to_string(&patched_marker).unwrap_or_default();
                if previous.trim() == VERSION_STRING {
                    return;
                }
                panic!(
                    "{} exists but patches {:?} for `{}` while this build expects `{}`. \
Remove `add-generated-files.diff` and `{}` after updating emulator sources.",
                    patched_marker.display(),
                    machine_dir_path,
                    previous.trim(),
                    VERSION_STRING,
                    patched_marker.display(),
                );
            }

            download_git_patch(&patch_file, VERSION_STRING);
            apply_git_patch(&patch_file, machine_dir_path);

            fs::write(&patched_marker, VERSION_STRING)
                .expect("failed to create .patched marker file");
        }

        fn download_git_patch(patch_file: &Path, target_tag: &str) {
            let emulator_git_url = "https://github.com/cartesi/machine-emulator";

            let patch_url = format!(
                "{}/releases/download/{}/add-generated-files.diff",
                emulator_git_url, target_tag,
            );

            // get
            let diff_data = reqwest::blocking::get(patch_url)
                .expect("error downloading diff of generated files")
                .bytes()
                .expect("error getting diff request body");

            // write to file
            write_bytes_to_file(patch_file.to_str().unwrap(), diff_data)
                .expect("failed to write `add-generated-files.diff`");
        }

        fn apply_git_patch(patch_file: &Path, target_dir: &Path) {
            // Open the patch file
            let mut patch = fs::File::open(patch_file).expect("fail to open patch file");

            // Create a command to run `patch -Np1`
            let mut cmd = Command::new("patch")
                .arg("-Np1")
                .stdin(Stdio::piped())
                .current_dir(target_dir)
                .spawn()
                .expect("fail to spawn patch command");

            // Write the contents of the patch file to the command's stdin
            if let Some(ref mut stdin) = cmd.stdin {
                let mut buffer = Vec::new();
                patch
                    .read_to_end(&mut buffer)
                    .expect("fail to read patch content");
                stdin
                    .write_all(&buffer)
                    .expect("fail to write patch to pipe");
            }

            // Wait for the command to complete
            let status = cmd.wait().expect("fail to wait for patch command");

            if !status.success() {
                eprintln!("Patch command failed with status: {:?}", status);
            }
        }

        fn write_bytes_to_file(path: &str, data: Bytes) -> io::Result<()> {
            let mut file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(path)
                .unwrap_or_else(|_| panic!("failed to open file {}", path));

            file.write_all(&data)?;
            file.flush() // Ensure all data is written to disk
        }
    }
}

#[cfg(feature = "wasm32")]
mod build_wasm32 {
    use std::{
        env, fs,
        path::{Path, PathBuf},
        process::Command,
    };

    pub fn build(machine_dir_path: &Path, out_path: &Path) {
        // 1. Apply generated-files diff (same as download_uarch)
        super::build_cm::download_uarch::download(machine_dir_path);

        // 2. Apply our WASI compatibility patch
        apply_wasi_patch(machine_dir_path);

        // 3. Generate interpret-jump-table.h
        generate_jump_table(machine_dir_path);

        // 4. Download boost if needed
        ensure_deps(machine_dir_path);

        // 5. Compile with wasi-sdk
        let wasi_sdk = find_wasi_sdk();
        compile_libcartesi(&wasi_sdk, machine_dir_path, out_path);
    }

    pub fn find_wasi_sdk() -> PathBuf {
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
        // Search /tmp for extracted wasi-sdk
        if let Ok(entries) = fs::read_dir("/tmp") {
            for entry in entries.flatten() {
                let name = entry.file_name();
                if name.to_string_lossy().starts_with("wasi-sdk-")
                    && entry.path().join("bin/clang++").exists()
                {
                    return entry.path();
                }
            }
        }
        panic!(
            "wasi-sdk not found. Set WASI_SDK_PATH or install wasi-sdk to /opt/wasi-sdk.\n\
             Download: https://github.com/WebAssembly/wasi-sdk/releases"
        );
    }

    fn apply_wasi_patch(emu_dir: &Path) {
        // WASI compat patch is bundled alongside this build script
        let patch_path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("patches")
            .join("wasi-compat.diff");

        if !patch_path.exists() {
            panic!("WASI patch not found at {}", patch_path.display());
        }

        println!("cargo:warning=Applying WASI compat patch...");
        let status = Command::new("patch")
            .arg("-Np1")
            .arg("-i")
            .arg(&patch_path)
            .current_dir(emu_dir)
            .status()
            .expect("Failed to run patch for WASI compat");

        if !status.success() {
            // -Np1 returns 1 if already applied (harmless)
            eprintln!("cargo:warning=WASI patch may already be applied (non-zero exit)");
        }
    }

    fn generate_jump_table(emu_dir: &Path) {
        let out = emu_dir.join("src/interpret-jump-table.h");
        if out.exists() {
            return;
        }
        let script = emu_dir.join("tools/gen-interpret-jump-table.lua");
        if !script.exists() {
            panic!("interpret-jump-table.h generator not found");
        }
        println!("cargo:warning=Generating interpret-jump-table.h...");
        let output = Command::new("lua5.4")
            .arg(&script)
            .output()
            .expect("lua5.4 not found — install lua5.4 to generate interpret-jump-table.h");
        fs::write(&out, &output.stdout).expect("failed to write interpret-jump-table.h");
    }

    fn ensure_deps(emu_dir: &Path) {
        let downloads = emu_dir.join("third-party/downloads");
        let boost = downloads.join("boost");
        if !boost.exists() {
            println!("cargo:warning=Downloading boost headers...");
            let status = Command::new("make")
                .arg("bundle-boost")
                .current_dir(emu_dir)
                .status()
                .expect("Failed to run make bundle-boost");
            if !status.success() {
                panic!("make bundle-boost failed");
            }
        }
    }

    fn compile_libcartesi(wasi_sdk: &Path, emu_dir: &Path, out_path: &Path) {
        let libcartesi = out_path.join("libcartesi.a");
        if libcartesi.exists() {
            println!("cargo:warning=libcartesi.a already built, skipping compilation");
            return;
        }

        let sysroot = wasi_sdk.join("share/wasi-sysroot");
        let third_party = emu_dir.join("third-party");
        let src = emu_dir.join("src");
        let obj_dir = out_path.join("wasm-obj");
        fs::create_dir_all(&obj_dir).unwrap();

        let cxx = wasi_sdk.join("bin/clang++");
        let cc = wasi_sdk.join("bin/clang");
        let ar = wasi_sdk.join("bin/llvm-ar");

        let cxxflags = format!(
            "--target=wasm32-wasi --sysroot={sysroot} \
             -std=gnu++23 -O2 -g0 -fwasm-exceptions \
             -DNO_TTY -DNO_THREADS -DNO_MMAP -DNO_SLIRP -DNO_SELECT \
             -DNO_POSIX_FS -DNO_SIGACTION -DNO_FORK -DNO_FLOCK \
             -DNO_FICLONE -DNO_TUNTAP -DNO_USLEEP -DNO_MKDIR \
             -DJSON_HAS_FILESYSTEM=0 -D_FILE_OFFSET_BITS=64 \
             -DBOOST_ASIO_DISABLE_ERROR_LOCATION -DNDEBUG \
             -I{src} -I{third_party}/ankerl \
             -I{third_party}/llvm-flang-uint128 -I{third_party}/nlohmann-json \
             -I{third_party}/downloads",
            sysroot = sysroot.display(),
            src = src.display(),
            third_party = third_party.display(),
        );

        let cflags = format!(
            "--target=wasm32-wasi --sysroot={sysroot} -O2 -g0 -DNDEBUG -I{src}",
            sysroot = sysroot.display(),
            src = src.display(),
        );

        // C++ objects (no jsonrpc-machine, no clua, no remote)
        let cpp_files: &[&str] = &[
            "base64", "clint-address-range", "dtb", "hash-tree", "htif-address-range",
            "interpret", "json-util", "machine-c-api", "machine-config", "machine",
            "machine-address-ranges", "machine-console", "memory-address-range",
            "os", "os-mapped-memory", "os-filesystem", "plic-address-range",
            "back-merkle-tree", "send-cmio-response", "keccak-256-hasher",
            "sha-256-hasher", "is-pristine", "uarch-pristine-state-hash",
            "uarch-reset-state", "uarch-step", "local-machine", "uarch-interpret",
            "virtio-address-range", "virtio-console-address-range",
            "virtio-p9fs-address-range", "virtio-net-address-range",
            "virtio-net-tuntap-address-range", "virtio-net-user-address-range",
        ];

        println!("cargo:warning=Compiling {} C++ files with wasi-sdk...", cpp_files.len());

        for name in cpp_files {
            let src_file = src.join(format!("{}.cpp", name));
            let obj = obj_dir.join(format!("{}.o", name));
            let status = Command::new(&cxx)
                .args(cxxflags.split_whitespace())
                .arg("-c")
                .arg(&src_file)
                .arg("-o")
                .arg(&obj)
                .status()
                .unwrap_or_else(|_| panic!("Failed to spawn clang++ for {}", name));
            if !status.success() {
                panic!("Compilation failed for {}.cpp", name);
            }
        }

        // C objects (uarch generated files)
        let c_pairs: &[(&str, &str)] = &[
            ("uarch/uarch-pristine-hash.c", "uarch-pristine-hash"),
            ("uarch/uarch-pristine-ram.c", "uarch-pristine-ram"),
        ];
        for (rel_path, obj_name) in c_pairs {
            let src_file = emu_dir.join(rel_path);
            if !src_file.exists() {
                panic!("Missing uarch file: {}", src_file.display());
            }
            let obj = obj_dir.join(format!("{}.o", obj_name));
            let status = Command::new(&cc)
                .args(cflags.split_whitespace())
                .arg("-c")
                .arg(&src_file)
                .arg("-o")
                .arg(&obj)
                .status()
                .expect("Failed to spawn clang");
            if !status.success() {
                panic!("Compilation failed for {}", rel_path);
            }
        }

        // Archive
        println!("cargo:warning=Archiving libcartesi.a...");
        let mut ar_cmd = Command::new(&ar);
        ar_cmd.arg("rcs").arg(&libcartesi);
        for entry in fs::read_dir(&obj_dir).unwrap() {
            let entry = entry.unwrap();
            if entry.path().extension().map_or(false, |e| e == "o") {
                ar_cmd.arg(entry.path());
            }
        }
        let status = ar_cmd.status().expect("Failed to run llvm-ar");
        if !status.success() {
            panic!("Failed to create libcartesi.a");
        }
    }
}

mod feature_checks {
    #[cfg(all(feature = "build_uarch", feature = "copy_uarch",))]
    compile_error!("Features `build_uarch` and `copy_uarch` are mutually exclusive");

    #[cfg(all(feature = "build_uarch", feature = "download_uarch"))]
    compile_error!("Features `build_uarch` and `download_uarch` are mutually exclusive");

    #[cfg(all(feature = "copy_uarch", feature = "download_uarch"))]
    compile_error!("Features `copy_uarch`, and `download_uarch` are mutually exclusive");

    #[cfg(any(
        all(feature = "wasm32", feature = "download_uarch"),
        all(feature = "wasm32", feature = "build_uarch"),
        all(feature = "wasm32", feature = "copy_uarch"),
        all(feature = "wasm32", feature = "external_cartesi"),
    ))]
    compile_error!("Feature `wasm32` is mutually exclusive with `download_uarch`, `build_uarch`, `copy_uarch`, and `external_cartesi`");

    #[cfg(not(any(
        feature = "copy_uarch",
        feature = "download_uarch",
        feature = "build_uarch",
        feature = "external_cartesi",
        feature = "wasm32",
    )))]
    compile_error!(
        "At least one of `build_uarch`, `copy_uarch`, `download_uarch`, `wasm32`, and `external_cartesi` must be set"
    );
}

#[allow(unused)]
fn clean(path: &PathBuf) {
    // clean build artifacts
    Command::new("make")
        .args(["clean", "depclean", "distclean"])
        .current_dir(path)
        .status()
        .expect("Failed to run setup `make clean depclean distclean`");
    Command::new("rm")
        .args(["src/*o.tmp"])
        .current_dir(path)
        .status()
        .expect("Failed to delete src/*.o.tmp files");
}