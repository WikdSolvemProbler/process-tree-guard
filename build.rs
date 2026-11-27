// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 Peter Shiller

//! Build script: gives each program a Windows version resource, so Task Manager and
//! File Explorer show a description for it.
//!
//! It writes the compiled resource (`.res`) bytes itself, with no resource compiler
//! and no crates, and links each one into its own program with
//! `cargo:rustc-link-arg-bin`. The resource is a VS_VERSIONINFO structure with a
//! StringFileInfo table; the company name is left out. Only the Microsoft (MSVC)
//! toolchain is supported: on another target no resource is linked. The functions
//! that build the bytes are public so that `tests/guard.rs` can include this file
//! and check what they write.

use std::env;
use std::fs;
use std::path::PathBuf;

const PRODUCT_NAME: &str = "Process Tree Guard";
const COPYRIGHT: &str = "Copyright 2026 Peter Shiller";

/// The programs, each with the description Task Manager shows.
pub const PROGRAMS: [(&str, &str); 5] = [
    ("process-tree-guard", "Process Tree Guard"),
    ("process-tree-guard-deadline", "Process Tree Guard: Deadline"),
    ("process-tree-guard-gate", "Process Tree Guard: Gate"),
    ("process-tree-guard-contain", "Process Tree Guard: Contain"),
    ("ptg", "Process Tree Guard"),
];

/// US English with the Unicode code page, as the key of the one string table.
const LANGUAGE_AND_CODE_PAGE: &str = "040904B0";
const LANGUAGE: u16 = 0x0409;
const CODE_PAGE: u16 = 0x04B0;

/// UTF-16 little endian text with a terminating NUL.
fn utf16z(text: &str) -> Vec<u8> {
    text.encode_utf16().chain(std::iter::once(0)).flat_map(|unit| unit.to_le_bytes()).collect()
}

fn align4(bytes: &mut Vec<u8>) {
    while !bytes.len().is_multiple_of(4) {
        bytes.push(0);
    }
}

/// One version information block: length, value length, type (1 text, 0 binary),
/// key, padding, value, then children, each starting on a four byte boundary. The
/// length covers the block and its children without the padding after the last one.
fn block(key: &str, text: bool, value_length: usize, value: &[u8], children: &[Vec<u8>]) -> Vec<u8> {
    let mut out = vec![0u8; 2];
    out.extend_from_slice(&(value_length as u16).to_le_bytes());
    out.extend_from_slice(&(text as u16).to_le_bytes());
    out.extend_from_slice(&utf16z(key));
    align4(&mut out);
    out.extend_from_slice(value);
    for child in children {
        align4(&mut out);
        out.extend_from_slice(child);
    }
    let length = out.len() as u16;
    out[..2].copy_from_slice(&length.to_le_bytes());
    out
}

/// A name and its text value, as one entry of the string table.
fn string_entry(name: &str, value: &str) -> Vec<u8> {
    let bytes = utf16z(value);
    block(name, true, bytes.len() / 2, &bytes, &[])
}

/// VS_FIXEDFILEINFO for version `major.minor.patch.0`, flagged as a pre-release build when asked.
fn fixed_file_info(major: u16, minor: u16, patch: u16, prerelease: bool) -> Vec<u8> {
    let most = (major as u32) << 16 | minor as u32;
    let least = (patch as u32) << 16;
    let words: [u32; 13] = [
        0xFEEF_04BD,                 // signature
        0x0001_0000,                 // structure version
        most, least, most, least,    // file version, product version
        0x3F,                        // flags that are valid
        if prerelease { 0x2 } else { 0 }, // VS_FF_PRERELEASE
        0x0004_0004,                 // VOS_NT_WINDOWS32
        1,                           // VFT_APP
        0, 0, 0,                     // subtype, date
    ];
    words.iter().flat_map(|word| word.to_le_bytes()).collect()
}

/// The VS_VERSIONINFO data for one program.
pub fn version_info(description: &str, original_filename: &str, version: &str, numbers: (u16, u16, u16), prerelease: bool) -> Vec<u8> {
    let strings = [
        string_entry("FileDescription", description),
        string_entry("FileVersion", version),
        string_entry("LegalCopyright", COPYRIGHT),
        string_entry("OriginalFilename", original_filename),
        string_entry("ProductName", PRODUCT_NAME),
        string_entry("ProductVersion", version),
    ];
    let table = block(LANGUAGE_AND_CODE_PAGE, true, 0, &[], &strings);
    let string_file_info = block("StringFileInfo", true, 0, &[], &[table]);
    let translation: Vec<u8> = [LANGUAGE.to_le_bytes(), CODE_PAGE.to_le_bytes()].concat();
    let var = block("Translation", false, translation.len(), &translation, &[]);
    let var_file_info = block("VarFileInfo", true, 0, &[], &[var]);
    let fixed = fixed_file_info(numbers.0, numbers.1, numbers.2, prerelease);
    block("VS_VERSION_INFO", false, fixed.len(), &fixed, &[string_file_info, var_file_info])
}

/// A `.res` file holding one RT_VERSION resource named 1: the empty 32 bit
/// resource that starts every `.res` file, then the resource with its header.
pub fn res_file(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&0u32.to_le_bytes()); // data size
    out.extend_from_slice(&32u32.to_le_bytes()); // header size
    out.extend_from_slice(&[0xFF, 0xFF, 0, 0, 0xFF, 0xFF, 0, 0]); // type 0, name 0
    out.extend_from_slice(&[0u8; 16]); // data version, flags, language, version, characteristics
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(&32u32.to_le_bytes());
    out.extend_from_slice(&[0xFF, 0xFF, 16, 0, 0xFF, 0xFF, 1, 0]); // type RT_VERSION, name 1
    out.extend_from_slice(&0u32.to_le_bytes()); // data version
    out.extend_from_slice(&0x0030u16.to_le_bytes()); // MOVEABLE and PURE
    out.extend_from_slice(&LANGUAGE.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // version
    out.extend_from_slice(&0u32.to_le_bytes()); // characteristics
    out.extend_from_slice(data);
    align4(&mut out);
    out
}

fn number(name: &str) -> u16 {
    env::var(name).ok().and_then(|text| text.parse().ok()).unwrap_or(0)
}

/// Fixed native assets for the opt-in separate-service binaries only.
const LINUX_NATIVE_OBJECTS: [&str; 6] = [
    "kernel_signal_abi", "native_arming", "payload_bootstrap",
    "terminal_bridge", "waitid_bridge", "entry_timer",
];
const LINUX_NATIVE_HEADERS: [&str; 7] = [
    "native_arming.h", "owner_poll.h", "owner_watch.h", "payload_bootstrap.h",
    "terminal_bridge.h", "waitid_bridge.h", "entry_timer.h",
];

fn directive_path(path: &std::path::Path) -> &str {
    let text = path.to_str().expect("native build path must be UTF-8");
    assert!(!text.contains(['\r', '\n']), "native build path must be one line");
    text
}

fn native_tool(name: &str) -> PathBuf {
    env::split_paths(&env::var_os("PATH").expect("PATH for native cc and ar"))
        .map(|folder| folder.join(name)).find(|path| path.is_file())
        .unwrap_or_else(|| panic!("native Linux build requires existing {name} on PATH"))
}

fn build_linux_native() {
    let source = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"))
        .join("src/sys/linux");
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    assert!(out.is_absolute() && out.is_dir(), "OUT_DIR must be an existing absolute directory");
    let archive = out.join("libptg_linux_native.a");
    directive_path(&archive);
    let cc = native_tool("cc");
    let ar = native_tool("ar");
    // Check every source/header before starting the first compiler.
    for asset in LINUX_NATIVE_OBJECTS.iter().map(|name| format!("{name}.c"))
        .chain(LINUX_NATIVE_HEADERS.iter().map(|name| (*name).to_string())) {
        let path = source.join(asset);
        assert!(path.is_file(), "missing native asset: {}", path.display());
        println!("cargo:rerun-if-changed={}", directive_path(&path));
    }
    for asset in ["50-process-tree-guard-ref.rules.in", "process-tree-guard.socket", "process-tree-guard@.service"] {
        let path = source.join("deployment").join(asset);
        assert!(path.is_file(), "missing deployment asset: {}", path.display());
        println!("cargo:rerun-if-changed={}", directive_path(&path));
    }
    println!("cargo:rerun-if-env-changed=PATH");
    let mut objects = Vec::with_capacity(LINUX_NATIVE_OBJECTS.len());
    for name in LINUX_NATIVE_OBJECTS {
        let object = out.join(format!("{name}.o"));
        let status = std::process::Command::new(&cc)
            .args(["-std=c11", "-O2", "-Wall", "-Wextra", "-Werror", "-fstack-protector-strong", "-c"])
            .arg(source.join(format!("{name}.c"))).arg("-o").arg(&object)
            .status().expect("run native C compiler");
        assert!(status.success(), "native C compiler failed for {name}: {status}");
        objects.push(object);
    }
    let status = std::process::Command::new(&ar).arg("rcs").arg(&archive).args(&objects)
        .status().expect("run native archive tool");
    assert!(status.success(), "native archive tool failed: {status}");
    // The two adapter crates own the static native dependency; rustc orders it
    // with native libraries before its upstream Rust/runtime libraries.
    println!("cargo:rustc-link-search=native={}", directive_path(&out));
}

// The candidate custody directory contains the exact source of the audited ELF.
const ORDINARY_SOURCE: &str = "tests/linux/ordinary/implementation-007";
const ORDINARY_C: [&str; 15] = [
    "bootstrap", "bootstrap_raw", "bootstrap_wire", "caller", "credentials", "descriptors",
    "enrollment", "evidence", "helper_main", "helper_startup", "lease", "lifecycle",
    "native_control", "owners", "request",
];
const ORDINARY_HEADERS: [&str; 17] = [
    "bootstrap.h", "bootstrap_raw.h", "bootstrap_steps.h", "bootstrap_wire.h",
    "bootstrap_wire_primitives.h", "credentials.h", "descriptors.h", "enrollment.h",
    "evidence.h", "helper_startup.h", "lease.h", "lifecycle.h", "native_control.h",
    "native_control_primitives.h", "native_syscall.h", "owners.h", "request.h",
];

fn ordinary_tool_output(tool: &std::path::Path, arg: &str) -> String {
    let result = std::process::Command::new(tool).arg(arg).output().expect("inspect ordinary Linux tool");
    assert!(result.status.success(), "ordinary Linux tool query failed: {} {arg}", tool.display());
    String::from_utf8(result.stdout).expect("ordinary Linux tool identity must be UTF-8")
}

fn build_linux_ordinary() {
    let target = env::var("TARGET").expect("TARGET");
    let host = env::var("HOST").expect("HOST");
    assert_eq!(target, "x86_64-unknown-linux-gnu",
        "linux-ordinary-library requires an x86_64-unknown-linux-gnu Cargo target");
    let cross = host != target;
    assert!(!cross || host == "aarch64-unknown-linux-gnu",
        "linux-ordinary-library requires a native x86_64 or aarch64 GNU Linux Cargo host");
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let source = manifest.join(ORDINARY_SOURCE);
    let shared = manifest.join("src/sys/linux");
    let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    assert!(out.is_absolute() && out.is_dir(), "OUT_DIR must be an existing absolute directory");
    const CC: &str = "PTG_ORDINARY_CC";
    const AR: &str = "PTG_ORDINARY_AR";
    println!("cargo:rerun-if-env-changed={CC}");
    println!("cargo:rerun-if-env-changed={AR}");
    let explicit_tool = |name: &str, value: std::ffi::OsString| {
        let path = PathBuf::from(value);
        assert!(path.is_absolute() && path.is_file(),
            "{name} must name an absolute existing program");
        println!("cargo:rerun-if-changed={}", directive_path(&path));
        path
    };
    let (gcc, ar) = match (env::var_os(CC), env::var_os(AR)) {
        (None, None) if !cross => (native_tool("gcc"), native_tool("ar")),
        (Some(cc), Some(ar)) => (explicit_tool(CC, cc), explicit_tool(AR, ar)),
        _ => panic!("{CC} and {AR} must both be set for an explicit or cross toolchain"),
    };
    assert_eq!(ordinary_tool_output(&gcc, "-dumpmachine").trim(), "x86_64-linux-gnu",
        "linux-ordinary-library requires GCC targeting x86-64 GNU Linux");
    assert_eq!(ordinary_tool_output(&gcc, "-dumpfullversion").trim(), "13.3.0",
        "linux-ordinary-library requires the scoped GCC 13.3.0 toolchain");
    assert!(ordinary_tool_output(&ar, "--version").starts_with("GNU ar "),
        "linux-ordinary-library requires GNU ar");
    let linker_name = ordinary_tool_output(&gcc, "-print-prog-name=ld");
    let linker = PathBuf::from(linker_name.trim());
    assert!(!cross || (linker.is_absolute() && linker.is_file()),
        "the cross compiler must identify its absolute existing GNU linker");
    let linker = if linker.is_absolute() { linker }
        else { native_tool(linker.to_str().expect("linker name must be UTF-8")) };
    assert!(ordinary_tool_output(&linker, "--version").starts_with("GNU ld "),
        "linux-ordinary-library requires the GCC-selected GNU ld");

    for asset in ORDINARY_C.iter().map(|name| source.join(format!("{name}.c")))
        .chain(ORDINARY_HEADERS.iter().map(|name| source.join(name)))
        .chain(["owner_watch.h", "waitid_bridge.h"].iter().map(|name| shared.join(name))) {
        assert!(asset.is_file(), "missing ordinary Linux source asset: {}", asset.display());
        println!("cargo:rerun-if-changed={}", directive_path(&asset));
    }
    println!("cargo:rerun-if-env-changed=PATH");

    let objects = out.join("ptg-ordinary-objects");
    fs::create_dir_all(&objects).expect("create ordinary Linux object directory");
    let mut caller = Vec::with_capacity(ORDINARY_C.len() - 1);
    let mut helper = Vec::with_capacity(ORDINARY_C.len());
    for name in ORDINARY_C {
        let object = objects.join(format!("{name}.o"));
        let mut compile = std::process::Command::new(&gcc);
        compile.args(["-m64", "-std=c11", "-O2", "-g", "-Wall", "-Wextra", "-Werror", "-fno-lto"]);
        if name == "bootstrap_raw" {
            compile.args(["-ffreestanding", "-fno-stack-protector", "-fno-tree-loop-distribute-patterns", "-fstack-usage"]);
        } else {
            compile.arg("-fstack-protector-strong");
        }
        let status = compile.arg("-I").arg(&shared).arg("-c").arg(source.join(format!("{name}.c")))
            .arg("-o").arg(&object).status().expect("compile ordinary Linux object");
        assert!(status.success(), "ordinary Linux compiler failed for {name}: {status}");
        if name != "helper_main" {
            caller.push(object.clone());
        }
        helper.push(object);
    }
    let archive = out.join("libptg_ordinary_caller.a");
    if archive.exists() {
        fs::remove_file(&archive).expect("replace prior ordinary Linux caller archive");
    }
    let status = std::process::Command::new(&ar).arg("rcs").arg(&archive).args(&caller)
        .status().expect("archive ordinary Linux caller objects");
    assert!(status.success(), "ordinary Linux caller archive failed: {status}");
    let helper_path = out.join("ordinary-linux-helper");
    let status = std::process::Command::new(&gcc)
        .args(["-m64", "-static", "-no-pie", "-fno-lto", "-fno-use-linker-plugin", "-Wl,--fatal-warnings"])
        .args(&helper).arg("-o").arg(&helper_path)
        .status().expect("link independent ordinary Linux helper");
    assert!(status.success(), "ordinary Linux helper link failed: {status}");
    println!("cargo:rustc-link-search=native={}", directive_path(&out));
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rustc-check-cfg=cfg(ptg_linux_separate_service_build)");
    let is_windows = env::var("CARGO_CFG_TARGET_OS").is_ok_and(|os| os == "windows");
    println!("cargo:rustc-check-cfg=cfg(ptg_linux_ordinary_build)");
    if !is_windows && env::var_os("CARGO_FEATURE_LINUX_ORDINARY_LIBRARY").is_some() {
        build_linux_ordinary();
        println!("cargo:rustc-cfg=ptg_linux_ordinary_build");
    }
    if !is_windows && env::var_os("CARGO_FEATURE_LINUX_SEPARATE_SERVICE").is_some() {
        let target = env::var("TARGET").expect("TARGET");
        let host = env::var("HOST").expect("HOST");
        assert!(matches!(target.as_str(), "x86_64-unknown-linux-gnu" | "aarch64-unknown-linux-gnu") && host == target,
            "linux-separate-service requires a native x86_64/aarch64 GNU Linux build (HOST == TARGET)");
        build_linux_native();
        println!("cargo:rustc-cfg=ptg_linux_separate_service_build");
        return;
    }
    let is_msvc = env::var("CARGO_CFG_TARGET_ENV").is_ok_and(|abi| abi == "msvc");
    // The GNU linker does not accept a `.res` file, so that target (unsupported)
    // builds the same programs without a version resource.
    if !(is_windows && is_msvc) {
        return;
    }
    let version = env::var("CARGO_PKG_VERSION").expect("CARGO_PKG_VERSION");
    let numbers = (number("CARGO_PKG_VERSION_MAJOR"), number("CARGO_PKG_VERSION_MINOR"), number("CARGO_PKG_VERSION_PATCH"));
    let prerelease = env::var("CARGO_PKG_VERSION_PRE").is_ok_and(|pre| !pre.is_empty());
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    for (name, description) in PROGRAMS {
        let data = version_info(description, &format!("{}.exe", name), &version, numbers, prerelease);
        let path = out_dir.join(format!("{}.res", name));
        fs::write(&path, res_file(&data)).expect("write the version resource");
        println!("cargo:rustc-link-arg-bin={}={}", name, path.display());
    }
}
