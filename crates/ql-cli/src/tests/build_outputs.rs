use std::fs;

use ql_driver::{BuildEmit, BuildOptions, BuildProfile, ToolchainOptions};

use crate::build_pipeline::build_path;
use crate::project_targets::ProjectTargetSelector;
use crate::tests::support::{TestDir, mock_success_archiver_invocation, mock_success_invocation};

#[test]
fn build_path_emits_llvm_ir_for_supported_source() {
    let dir = TestDir::new("ql-cli-build");
    dir.write(
        "sample.ql",
        r#"
fn add_one(value: Int) -> Int {
    return value + 1
}

fn main() -> Int {
    return add_one(41)
}
"#,
    );
    let output = dir.path().join("artifacts/sample.ll");
    let options = BuildOptions {
        emit: BuildEmit::LlvmIr,
        profile: BuildProfile::Debug,
        output: Some(output.clone()),
        c_header: None,
        toolchain: ToolchainOptions::default(),
    };

    assert!(
        build_path(
            &dir.path().join("sample.ql"),
            &options,
            &ProjectTargetSelector::default(),
            false,
            false,
            false,
            false,
        )
        .is_ok()
    );

    let rendered = fs::read_to_string(output).expect("read emitted LLVM IR");
    assert!(rendered.contains("define i32 @main()"));
    assert!(rendered.contains("define i64 @ql_1_main()"));
}

#[test]
fn build_path_emits_assembly_for_supported_source() {
    let dir = TestDir::new("ql-cli-build-asm");
    dir.write(
        "sample.ql",
        r#"
fn main() -> Int {
    return 1
}
"#,
    );
    let output = dir.path().join("artifacts/sample.s");
    let options = BuildOptions {
        emit: BuildEmit::Assembly,
        profile: BuildProfile::Debug,
        output: Some(output.clone()),
        c_header: None,
        toolchain: ToolchainOptions {
            clang: Some(mock_success_invocation(&dir)),
            ..ToolchainOptions::default()
        },
    };

    assert!(
        build_path(
            &dir.path().join("sample.ql"),
            &options,
            &ProjectTargetSelector::default(),
            false,
            false,
            false,
            false,
        )
        .is_ok()
    );

    let rendered = fs::read_to_string(output).expect("read emitted assembly placeholder");
    assert_eq!(rendered, "mock-assembly");
}

#[test]
fn build_path_emits_object_for_supported_source() {
    let dir = TestDir::new("ql-cli-build-obj");
    dir.write(
        "sample.ql",
        r#"
fn main() -> Int {
    return 1
}
"#,
    );
    let output = dir.path().join(if cfg!(windows) {
        "artifacts/sample.obj"
    } else {
        "artifacts/sample.o"
    });
    let options = BuildOptions {
        emit: BuildEmit::Object,
        profile: BuildProfile::Debug,
        output: Some(output.clone()),
        c_header: None,
        toolchain: ToolchainOptions {
            clang: Some(mock_success_invocation(&dir)),
            ..ToolchainOptions::default()
        },
    };

    assert!(
        build_path(
            &dir.path().join("sample.ql"),
            &options,
            &ProjectTargetSelector::default(),
            false,
            false,
            false,
            false,
        )
        .is_ok()
    );

    let rendered = fs::read_to_string(output).expect("read emitted object placeholder");
    assert_eq!(rendered, "mock-object");
}

#[test]
fn build_path_emits_executable_for_supported_source() {
    let dir = TestDir::new("ql-cli-build-exe");
    dir.write(
        "sample.ql",
        r#"
fn main() -> Int {
    return 1
}
"#,
    );
    let output = dir.path().join(if cfg!(windows) {
        "artifacts/sample.exe"
    } else {
        "artifacts/sample"
    });
    let options = BuildOptions {
        emit: BuildEmit::Executable,
        profile: BuildProfile::Debug,
        output: Some(output.clone()),
        c_header: None,
        toolchain: ToolchainOptions {
            clang: Some(mock_success_invocation(&dir)),
            ..ToolchainOptions::default()
        },
    };

    assert!(
        build_path(
            &dir.path().join("sample.ql"),
            &options,
            &ProjectTargetSelector::default(),
            false,
            false,
            false,
            false,
        )
        .is_ok()
    );

    let rendered = fs::read_to_string(output).expect("read emitted executable placeholder");
    assert_eq!(rendered, "mock-executable");
}

#[test]
fn build_path_emits_dynamic_library_for_supported_source() {
    let dir = TestDir::new("ql-cli-build-dylib");
    dir.write(
        "ffi_export.ql",
        r#"
extern "c" pub fn q_add(left: Int, right: Int) -> Int {
    return left + right
}
"#,
    );
    let output = dir.path().join(if cfg!(windows) {
        "artifacts/ffi_export.dll"
    } else if cfg!(target_os = "macos") {
        "artifacts/libffi_export.dylib"
    } else {
        "artifacts/libffi_export.so"
    });
    let options = BuildOptions {
        emit: BuildEmit::DynamicLibrary,
        profile: BuildProfile::Debug,
        output: Some(output.clone()),
        c_header: None,
        toolchain: ToolchainOptions {
            clang: Some(mock_success_invocation(&dir)),
            ..ToolchainOptions::default()
        },
    };

    assert!(
        build_path(
            &dir.path().join("ffi_export.ql"),
            &options,
            &ProjectTargetSelector::default(),
            false,
            false,
            false,
            false,
        )
        .is_ok()
    );

    let rendered = fs::read_to_string(output).expect("read emitted dynamic library placeholder");
    assert_eq!(rendered, "mock-dylib");
}

#[test]
fn build_path_emits_static_library_for_supported_source() {
    let dir = TestDir::new("ql-cli-build-staticlib");
    dir.write(
        "math.ql",
        r#"
fn add_one(value: Int) -> Int {
    return value + 1
}
"#,
    );
    let output = dir.path().join(if cfg!(windows) {
        "artifacts/math.lib"
    } else {
        "artifacts/libmath.a"
    });
    let options = BuildOptions {
        emit: BuildEmit::StaticLibrary,
        profile: BuildProfile::Debug,
        output: Some(output.clone()),
        c_header: None,
        toolchain: ToolchainOptions {
            clang: Some(mock_success_invocation(&dir)),
            archiver: Some(mock_success_archiver_invocation(&dir)),
        },
    };

    assert!(
        build_path(
            &dir.path().join("math.ql"),
            &options,
            &ProjectTargetSelector::default(),
            false,
            false,
            false,
            false,
        )
        .is_ok()
    );

    let rendered = fs::read_to_string(output).expect("read emitted static library placeholder");
    assert_eq!(rendered, "mock-staticlib");
}
