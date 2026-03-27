use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("Cargo should set CARGO_MANIFEST_DIR"));
    let example_root = manifest_dir
        .parent()
        .expect("host project should live under examples/ffi-rust");
    let ql_source = example_root.join("ql").join("callback_add.ql");
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("Cargo should set OUT_DIR"));
    let staticlib = if cfg!(windows) {
        out_dir.join("ffi_rust_example.lib")
    } else {
        out_dir.join("libffi_rust_example.a")
    };
    let qlang = env::var_os("QLANG_BIN").unwrap_or_else(|| "ql".into());

    println!("cargo:rerun-if-changed={}", ql_source.display());
    println!("cargo:rerun-if-env-changed=QLANG_BIN");
    println!("cargo:rerun-if-env-changed=QLANG_AR");
    println!("cargo:rerun-if-env-changed=QLANG_AR_STYLE");

    let status = Command::new(&qlang)
        .arg("build")
        .arg(&ql_source)
        .args(["--emit", "staticlib"])
        .arg("--output")
        .arg(&staticlib)
        .status()
        .unwrap_or_else(|error| {
            panic!(
                "failed to run `{}`: {error}",
                PathBuf::from(&qlang).display()
            )
        });
    assert!(
        status.success(),
        "expected `ql build` to produce `{}`",
        staticlib.display()
    );

    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=ffi_rust_example");
}
