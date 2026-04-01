mod support;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use ql_driver::{ToolchainOptions, discover_toolchain};
use support::{
    TempDir, dynamic_library_output_path, executable_output_path, normalize, run_ql_build_capture,
    static_library_output_path, workspace_root,
};

#[test]
fn ffi_exports_link_from_c_static_harnesses() {
    let workspace_root = workspace_root();
    let fixture_root = workspace_root.join("tests/ffi/pass");
    let cases = collect_static_ffi_cases(&fixture_root);
    assert!(
        !cases.is_empty(),
        "expected at least one static FFI fixture under `{}`",
        fixture_root.display()
    );

    let Ok(toolchain) = discover_toolchain(&ToolchainOptions::default()) else {
        eprintln!(
            "skipping static FFI integration tests: no clang-style compiler found via ql-driver toolchain discovery"
        );
        return;
    };
    if toolchain.archiver().is_none() {
        eprintln!(
            "skipping static FFI integration tests: no archive tool found via ql-driver toolchain discovery"
        );
        return;
    }
    let clang = toolchain.clang().program.clone();

    let mut failures = Vec::new();
    for case in cases {
        if let Err(message) = run_static_ffi_case(&workspace_root, &clang, &case) {
            failures.push(message);
        }
    }

    assert!(
        failures.is_empty(),
        "static FFI integration regressions:\n\n{}",
        failures.join("\n\n")
    );
}

#[test]
fn ffi_exports_link_from_rust_static_harnesses() {
    let workspace_root = workspace_root();
    let fixture_root = workspace_root.join("tests/ffi/pass");
    let cases = collect_rust_static_ffi_cases(&fixture_root);
    assert!(
        !cases.is_empty(),
        "expected at least one Rust static FFI fixture under `{}`",
        fixture_root.display()
    );

    let Some(rustc) = resolve_program_from_env_or_path("RUSTC", &rustc_candidates()) else {
        eprintln!(
            "skipping Rust static FFI integration tests: no rustc found on PATH and `RUSTC` is not set"
        );
        return;
    };
    let Ok(toolchain) = discover_toolchain(&ToolchainOptions::default()) else {
        eprintln!(
            "skipping Rust static FFI integration tests: no clang-style compiler found via ql-driver toolchain discovery"
        );
        return;
    };
    if toolchain.archiver().is_none() {
        eprintln!(
            "skipping Rust static FFI integration tests: no archive tool found via ql-driver toolchain discovery"
        );
        return;
    }

    let mut failures = Vec::new();
    for case in cases {
        if let Err(message) = run_static_rust_ffi_case(&workspace_root, &rustc, &case) {
            failures.push(message);
        }
    }

    assert!(
        failures.is_empty(),
        "Rust static FFI integration regressions:\n\n{}",
        failures.join("\n\n")
    );
}

#[test]
fn ffi_exports_link_from_rust_cargo_static_harnesses() {
    let workspace_root = workspace_root();
    let fixture_root = workspace_root.join("tests/ffi/pass");
    let cases = collect_rust_static_ffi_cases(&fixture_root);
    assert!(
        !cases.is_empty(),
        "expected at least one Rust static FFI fixture under `{}`",
        fixture_root.display()
    );

    let Some(cargo) = resolve_program_from_env_or_path("CARGO", &cargo_candidates()) else {
        eprintln!(
            "skipping Cargo-based Rust FFI integration tests: no cargo found on PATH and `CARGO` is not set"
        );
        return;
    };
    let Ok(toolchain) = discover_toolchain(&ToolchainOptions::default()) else {
        eprintln!(
            "skipping Cargo-based Rust FFI integration tests: no clang-style compiler found via ql-driver toolchain discovery"
        );
        return;
    };
    if toolchain.archiver().is_none() {
        eprintln!(
            "skipping Cargo-based Rust FFI integration tests: no archive tool found via ql-driver toolchain discovery"
        );
        return;
    }

    let mut failures = Vec::new();
    for case in cases {
        if let Err(message) = run_cargo_rust_ffi_case(&workspace_root, &cargo, &case) {
            failures.push(message);
        }
    }

    assert!(
        failures.is_empty(),
        "Cargo-based Rust FFI integration regressions:\n\n{}",
        failures.join("\n\n")
    );
}

#[test]
fn ffi_committed_examples_run() {
    let workspace_root = workspace_root();
    let cases = committed_example_cases();
    assert!(
        !cases.is_empty(),
        "expected at least one committed FFI example under `{}`",
        workspace_root.join("examples").display()
    );

    let mut failures = Vec::new();
    for case in cases {
        if let Err(message) = run_committed_example_case(&workspace_root, case) {
            failures.push(message);
        }
    }

    assert!(
        failures.is_empty(),
        "committed FFI example regressions:\n\n{}",
        failures.join("\n\n")
    );
}

#[test]
fn ffi_exports_load_from_c_dynamic_harnesses() {
    let workspace_root = workspace_root();
    let fixture_root = workspace_root.join("tests/ffi/pass");
    let cases = collect_dynamic_ffi_cases(&fixture_root);
    assert!(
        !cases.is_empty(),
        "expected at least one shared-library FFI fixture under `{}`",
        fixture_root.display()
    );

    let Ok(toolchain) = discover_toolchain(&ToolchainOptions::default()) else {
        eprintln!(
            "skipping shared-library FFI integration tests: no clang-style compiler found via ql-driver toolchain discovery"
        );
        return;
    };
    let clang = toolchain.clang().program.clone();

    let mut failures = Vec::new();
    for case in cases {
        if let Err(message) = run_dynamic_ffi_case(&workspace_root, &clang, &case) {
            failures.push(message);
        }
    }

    assert!(
        failures.is_empty(),
        "shared-library FFI integration regressions:\n\n{}",
        failures.join("\n\n")
    );
}

#[derive(Clone, Debug)]
struct FfiCase {
    name: String,
    ql_path: PathBuf,
    harness_path: PathBuf,
    header_surface: HeaderSurface,
}

#[derive(Clone, Copy, Debug)]
struct CommittedExampleCase {
    name: &'static str,
    example_relative: &'static str,
    ql_relative: &'static str,
    host_relative: &'static str,
    host_kind: CommittedExampleHostKind,
    expected_stdout_fragments: &'static [&'static str],
}

#[derive(Clone, Copy, Debug)]
enum CommittedExampleHostKind {
    CStaticlib,
    CDylib,
    RustCargoStaticlib,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum HeaderSurface {
    #[default]
    Exports,
    Imports,
    Both,
}

impl HeaderSurface {
    fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "exports" => Some(Self::Exports),
            "imports" => Some(Self::Imports),
            "both" => Some(Self::Both),
            _ => None,
        }
    }

    fn cli_value(self) -> &'static str {
        match self {
            Self::Exports => "exports",
            Self::Imports => "imports",
            Self::Both => "both",
        }
    }

    fn header_file_name(self, stem: &str) -> String {
        match self {
            Self::Exports => format!("{stem}.h"),
            Self::Imports => format!("{stem}.imports.h"),
            Self::Both => format!("{stem}.ffi.h"),
        }
    }
}

fn run_static_ffi_case(workspace_root: &Path, clang: &Path, case: &FfiCase) -> Result<(), String> {
    let temp = TempDir::new(&format!("ql-ffi-static-{}", case.name));
    let header = header_output_path(temp.path(), &case.name, case.header_surface);
    let staticlib = static_library_output_path(temp.path(), &case.name);
    let executable = executable_output_path(temp.path(), &case.name);
    let relative_ql = relative_ql_path(workspace_root, &case.ql_path);

    run_ql_build(
        workspace_root,
        &case.name,
        &relative_ql,
        "staticlib",
        &staticlib,
        case.header_surface,
        Some(&header),
    )?;
    if !staticlib.is_file() {
        return Err(format!(
            "[{}] expected static library `{}` to exist after build",
            case.name,
            staticlib.display()
        ));
    }
    if !header.is_file() {
        return Err(format!(
            "[{}] expected generated header `{}` to exist after `ql build --header-output`",
            case.name,
            header.display()
        ));
    }

    let compile = Command::new(clang)
        .current_dir(workspace_root)
        .arg("-I")
        .arg(temp.path())
        .arg(&case.harness_path)
        .arg(&staticlib)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap_or_else(|_| {
            panic!(
                "run static C harness compiler `{}` for `{}`",
                clang.display(),
                case.name
            )
        });
    let compile_stdout = normalize(&String::from_utf8_lossy(&compile.stdout));
    let compile_stderr = normalize(&String::from_utf8_lossy(&compile.stderr));
    if compile.status.code().is_none_or(|code| code != 0) {
        return Err(format!(
            "[{}] expected static C harness link to succeed, got {:?}\nstdout:\n{}\nstderr:\n{}",
            case.name,
            compile.status.code(),
            compile_stdout,
            compile_stderr
        ));
    }
    if !executable.is_file() {
        return Err(format!(
            "[{}] expected executable `{}` to exist after static C link",
            case.name,
            executable.display()
        ));
    }

    let run = Command::new(&executable)
        .current_dir(workspace_root)
        .output()
        .unwrap_or_else(|_| panic!("run static FFI executable `{}`", executable.display()));
    let run_stdout = normalize(&String::from_utf8_lossy(&run.stdout));
    let run_stderr = normalize(&String::from_utf8_lossy(&run.stderr));
    if run.status.code().is_none_or(|code| code != 0) {
        return Err(format!(
            "[{}] expected static FFI executable to exit with 0, got {:?}\nstdout:\n{}\nstderr:\n{}",
            case.name,
            run.status.code(),
            run_stdout,
            run_stderr
        ));
    }

    Ok(())
}

fn run_dynamic_ffi_case(workspace_root: &Path, clang: &Path, case: &FfiCase) -> Result<(), String> {
    let temp = TempDir::new(&format!("ql-ffi-shared-{}", case.name));
    let header = header_output_path(temp.path(), &case.name, case.header_surface);
    let dynamic_library = dynamic_library_output_path(temp.path(), &case.name);
    let executable = executable_output_path(temp.path(), &format!("{}_shared", case.name));
    let relative_ql = relative_ql_path(workspace_root, &case.ql_path);

    run_ql_build(
        workspace_root,
        &case.name,
        &relative_ql,
        "dylib",
        &dynamic_library,
        case.header_surface,
        Some(&header),
    )?;
    if !dynamic_library.is_file() {
        return Err(format!(
            "[{}] expected dynamic library `{}` to exist after build",
            case.name,
            dynamic_library.display()
        ));
    }
    if !header.is_file() {
        return Err(format!(
            "[{}] expected generated header `{}` to exist after `ql build --header-output`",
            case.name,
            header.display()
        ));
    }

    let mut compile = Command::new(clang);
    compile
        .current_dir(workspace_root)
        .arg("-I")
        .arg(temp.path())
        .arg(&case.harness_path)
        .arg("-o")
        .arg(&executable);
    if cfg!(target_os = "linux") {
        compile.arg("-ldl");
    }
    let compile = compile.output().unwrap_or_else(|_| {
        panic!(
            "run shared-library C harness compiler `{}` for `{}`",
            clang.display(),
            case.name
        )
    });
    let compile_stdout = normalize(&String::from_utf8_lossy(&compile.stdout));
    let compile_stderr = normalize(&String::from_utf8_lossy(&compile.stderr));
    if compile.status.code().is_none_or(|code| code != 0) {
        return Err(format!(
            "[{}] expected shared-library C harness build to succeed, got {:?}\nstdout:\n{}\nstderr:\n{}",
            case.name,
            compile.status.code(),
            compile_stdout,
            compile_stderr
        ));
    }
    if !executable.is_file() {
        return Err(format!(
            "[{}] expected executable `{}` to exist after shared-library C harness build",
            case.name,
            executable.display()
        ));
    }

    let run = Command::new(&executable)
        .current_dir(workspace_root)
        .arg(&dynamic_library)
        .output()
        .unwrap_or_else(|_| {
            panic!(
                "run shared-library FFI executable `{}`",
                executable.display()
            )
        });
    let run_stdout = normalize(&String::from_utf8_lossy(&run.stdout));
    let run_stderr = normalize(&String::from_utf8_lossy(&run.stderr));
    if run.status.code().is_none_or(|code| code != 0) {
        return Err(format!(
            "[{}] expected shared-library FFI executable to exit with 0, got {:?}\nstdout:\n{}\nstderr:\n{}",
            case.name,
            run.status.code(),
            run_stdout,
            run_stderr
        ));
    }

    Ok(())
}

fn committed_example_cases() -> &'static [CommittedExampleCase] {
    &[
        CommittedExampleCase {
            name: "ffi-c",
            example_relative: "examples/ffi-c",
            ql_relative: "ql/callback_add.ql",
            host_relative: "host/main.c",
            host_kind: CommittedExampleHostKind::CStaticlib,
            expected_stdout_fragments: &["q_add_two(40) = 42", "q_scale(6, 7) = 42"],
        },
        CommittedExampleCase {
            name: "ffi-c-dylib",
            example_relative: "examples/ffi-c-dylib",
            ql_relative: "ql/callback_add.ql",
            host_relative: "host/main.c",
            host_kind: CommittedExampleHostKind::CDylib,
            expected_stdout_fragments: &["q_add(20, 22) = 42"],
        },
        CommittedExampleCase {
            name: "ffi-rust",
            example_relative: "examples/ffi-rust",
            ql_relative: "ql/callback_add.ql",
            host_relative: "host/Cargo.toml",
            host_kind: CommittedExampleHostKind::RustCargoStaticlib,
            expected_stdout_fragments: &["q_add_two(40) = 42", "q_scale(6, 7) = 42"],
        },
    ]
}

fn run_committed_example_case(
    workspace_root: &Path,
    case: &CommittedExampleCase,
) -> Result<(), String> {
    let example_root = workspace_root.join(case.example_relative);
    let ql_source = example_root.join(case.ql_relative);
    let host_path = example_root.join(case.host_relative);
    if !ql_source.is_file() {
        return Err(format!(
            "[{}] expected committed example Qlang source at `{}`",
            case.name,
            ql_source.display()
        ));
    }
    if !host_path.is_file() {
        return Err(format!(
            "[{}] expected committed example host file at `{}`",
            case.name,
            host_path.display()
        ));
    }

    match case.host_kind {
        CommittedExampleHostKind::CStaticlib => {
            let Ok(toolchain) = discover_toolchain(&ToolchainOptions::default()) else {
                eprintln!(
                    "skipping committed FFI example `{}`: no clang-style compiler found via ql-driver toolchain discovery",
                    case.name
                );
                return Ok(());
            };
            if toolchain.archiver().is_none() {
                eprintln!(
                    "skipping committed FFI example `{}`: no archive tool found via ql-driver toolchain discovery",
                    case.name
                );
                return Ok(());
            }
            run_committed_c_example(
                workspace_root,
                &toolchain.clang().program,
                &ql_source,
                &host_path,
                case.expected_stdout_fragments,
            )
        }
        CommittedExampleHostKind::CDylib => {
            let Ok(toolchain) = discover_toolchain(&ToolchainOptions::default()) else {
                eprintln!(
                    "skipping committed FFI example `{}`: no clang-style compiler found via ql-driver toolchain discovery",
                    case.name
                );
                return Ok(());
            };
            run_committed_c_dylib_example(
                workspace_root,
                &toolchain.clang().program,
                &ql_source,
                &host_path,
                case.expected_stdout_fragments,
            )
        }
        CommittedExampleHostKind::RustCargoStaticlib => {
            let Some(cargo) = resolve_program_from_env_or_path("CARGO", &cargo_candidates()) else {
                eprintln!(
                    "skipping committed FFI example `{}`: no cargo found on PATH and `CARGO` is not set",
                    case.name
                );
                return Ok(());
            };
            let Ok(toolchain) = discover_toolchain(&ToolchainOptions::default()) else {
                eprintln!(
                    "skipping committed FFI example `{}`: no clang-style compiler found via ql-driver toolchain discovery",
                    case.name
                );
                return Ok(());
            };
            if toolchain.archiver().is_none() {
                eprintln!(
                    "skipping committed FFI example `{}`: no archive tool found via ql-driver toolchain discovery",
                    case.name
                );
                return Ok(());
            }
            run_committed_rust_example(&example_root, &cargo, case.expected_stdout_fragments)
        }
    }
}

fn collect_static_ffi_cases(root: &Path) -> Vec<FfiCase> {
    let mut cases = Vec::new();
    for entry in
        fs::read_dir(root).unwrap_or_else(|_| panic!("read FFI fixture dir `{}`", root.display()))
    {
        let entry = entry.expect("read static FFI fixture entry");
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("ql") {
            continue;
        }

        let name = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("ffi_case")
            .to_owned();
        let harness_path = path.with_extension("c");
        assert!(
            harness_path.is_file(),
            "expected matching static C harness `{}` for `{}`",
            harness_path.display(),
            path.display()
        );
        let header_surface = read_header_surface_metadata(&path);
        cases.push(FfiCase {
            name,
            ql_path: path,
            harness_path,
            header_surface,
        });
    }
    cases.sort_by(|left, right| left.name.cmp(&right.name));
    cases
}

fn collect_rust_static_ffi_cases(root: &Path) -> Vec<FfiCase> {
    let mut cases = Vec::new();
    for entry in
        fs::read_dir(root).unwrap_or_else(|_| panic!("read FFI fixture dir `{}`", root.display()))
    {
        let entry = entry.expect("read Rust static FFI fixture entry");
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("ql") {
            continue;
        }

        let name = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("ffi_case")
            .to_owned();
        let harness_path = path.with_extension("rs");
        if !harness_path.is_file() {
            continue;
        }
        cases.push(FfiCase {
            name,
            ql_path: path,
            harness_path,
            header_surface: HeaderSurface::Exports,
        });
    }
    cases.sort_by(|left, right| left.name.cmp(&right.name));
    cases
}

fn collect_dynamic_ffi_cases(root: &Path) -> Vec<FfiCase> {
    let mut cases = Vec::new();
    for entry in
        fs::read_dir(root).unwrap_or_else(|_| panic!("read FFI fixture dir `{}`", root.display()))
    {
        let entry = entry.expect("read shared-library FFI fixture entry");
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("ql") {
            continue;
        }

        let name = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("ffi_case")
            .to_owned();
        let harness_path = root.join(format!("{name}.shared.c"));
        if !harness_path.is_file() {
            continue;
        }
        let header_surface = read_header_surface_metadata(&path);
        cases.push(FfiCase {
            name,
            ql_path: path,
            harness_path,
            header_surface,
        });
    }
    cases.sort_by(|left, right| left.name.cmp(&right.name));
    cases
}

fn relative_ql_path(workspace_root: &Path, ql_path: &Path) -> String {
    ql_path
        .strip_prefix(workspace_root)
        .expect("ffi fixture should be inside workspace root")
        .to_string_lossy()
        .replace('\\', "/")
}

fn resolve_program_from_env_or_path(env_var: &str, candidates: &[&str]) -> Option<PathBuf> {
    if let Ok(override_path) = env::var(env_var) {
        let trimmed = override_path.trim();
        if !trimmed.is_empty() {
            let path = PathBuf::from(trimmed);
            if path.is_file() {
                return Some(path);
            }
        }
    }

    let path_var = env::var_os("PATH")?;
    for directory in env::split_paths(&path_var) {
        for candidate in candidates {
            let path = directory.join(candidate);
            if path.is_file() {
                return Some(path);
            }
        }
    }

    None
}

fn rustc_candidates() -> Vec<&'static str> {
    if cfg!(windows) {
        vec!["rustc.exe", "rustc.cmd", "rustc.bat", "rustc"]
    } else {
        vec!["rustc"]
    }
}

fn cargo_candidates() -> Vec<&'static str> {
    if cfg!(windows) {
        vec!["cargo.exe", "cargo.cmd", "cargo.bat", "cargo"]
    } else {
        vec!["cargo"]
    }
}

fn header_output_path(root: &Path, stem: &str, surface: HeaderSurface) -> PathBuf {
    root.join(surface.header_file_name(stem))
}

fn rust_crate_name(stem: &str) -> String {
    let mut name = String::from("ql_ffi_");
    for ch in stem.chars() {
        if ch.is_ascii_alphanumeric() {
            name.push(ch.to_ascii_lowercase());
        } else {
            name.push('_');
        }
    }
    name
}

fn write_rust_cargo_project(
    project_root: &Path,
    case: &FfiCase,
    link_dir: &Path,
) -> Result<(), String> {
    let src_dir = project_root.join("src");
    fs::create_dir_all(&src_dir).map_err(|error| {
        format!(
            "[{}] create Cargo host src dir `{}`: {error}",
            case.name,
            src_dir.display()
        )
    })?;
    fs::copy(&case.harness_path, src_dir.join("main.rs")).map_err(|error| {
        format!(
            "[{}] copy Rust harness `{}` into Cargo project: {error}",
            case.name,
            case.harness_path.display()
        )
    })?;

    let crate_name = rust_crate_name(&case.name);
    fs::write(
        project_root.join("Cargo.toml"),
        format!("[package]\nname = \"{crate_name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n"),
    )
    .map_err(|error| {
        format!(
            "[{}] write Cargo manifest `{}`: {error}",
            case.name,
            project_root.join("Cargo.toml").display()
        )
    })?;

    let build_rs = format!(
        "fn main() {{\n    let link_dir = {:?};\n    println!(\"cargo:rustc-link-search=native={{}}\", link_dir);\n    println!(\"cargo:rustc-link-lib=static={}\");\n}}\n",
        link_dir.to_string_lossy().to_string(),
        case.name
    );
    fs::write(project_root.join("build.rs"), build_rs).map_err(|error| {
        format!(
            "[{}] write Cargo build script `{}`: {error}",
            case.name,
            project_root.join("build.rs").display()
        )
    })?;

    Ok(())
}

fn copy_directory_recursive(source: &Path, destination: &Path) -> Result<(), String> {
    fs::create_dir_all(destination).map_err(|error| {
        format!(
            "create example directory `{}` from `{}`: {error}",
            destination.display(),
            source.display()
        )
    })?;

    for entry in fs::read_dir(source)
        .map_err(|error| format!("read example directory `{}`: {error}", source.display()))?
    {
        let entry = entry.map_err(|error| {
            format!(
                "read example directory entry under `{}`: {error}",
                source.display()
            )
        })?;
        let entry_type = entry.file_type().map_err(|error| {
            format!(
                "read file type for example entry `{}`: {error}",
                entry.path().display()
            )
        })?;
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if entry_type.is_dir() {
            copy_directory_recursive(&source_path, &destination_path)?;
            continue;
        }
        if !entry_type.is_file() {
            continue;
        }
        fs::copy(&source_path, &destination_path).map_err(|error| {
            format!(
                "copy example file `{}` -> `{}`: {error}",
                source_path.display(),
                destination_path.display()
            )
        })?;
    }

    Ok(())
}

fn run_ql_build(
    workspace_root: &Path,
    case_name: &str,
    relative_ql: &str,
    emit: &str,
    output_path: &Path,
    header_surface: HeaderSurface,
    header_path: Option<&Path>,
) -> Result<(), String> {
    let mut extra_args = Vec::new();
    if let Some(header_path) = header_path {
        extra_args.push("--header-surface".to_owned());
        extra_args.push(header_surface.cli_value().to_owned());
        extra_args.push("--header-output".to_owned());
        extra_args.push(header_path.to_string_lossy().to_string());
    }
    let build = run_ql_build_capture(workspace_root, relative_ql, emit, output_path, &extra_args);
    let build_stdout = normalize(&String::from_utf8_lossy(&build.stdout));
    let build_stderr = normalize(&String::from_utf8_lossy(&build.stderr));
    if build.status.code().is_none_or(|code| code != 0) {
        return Err(format!(
            "[{}] expected `ql build --emit {}` to succeed, got {:?}\nstdout:\n{}\nstderr:\n{}",
            case_name,
            emit,
            build.status.code(),
            build_stdout,
            build_stderr
        ));
    }
    Ok(())
}

fn run_static_rust_ffi_case(
    workspace_root: &Path,
    rustc: &Path,
    case: &FfiCase,
) -> Result<(), String> {
    let temp = TempDir::new(&format!("ql-ffi-rust-static-{}", case.name));
    let staticlib = static_library_output_path(temp.path(), &case.name);
    let executable = executable_output_path(temp.path(), &format!("{}_rust", case.name));
    let relative_ql = relative_ql_path(workspace_root, &case.ql_path);

    run_ql_build(
        workspace_root,
        &case.name,
        &relative_ql,
        "staticlib",
        &staticlib,
        HeaderSurface::Exports,
        None,
    )?;
    if !staticlib.is_file() {
        return Err(format!(
            "[{}] expected static library `{}` to exist after build",
            case.name,
            staticlib.display()
        ));
    }

    let compile = Command::new(rustc)
        .current_dir(workspace_root)
        .arg("--edition=2021")
        .arg("--crate-name")
        .arg(rust_crate_name(&case.name))
        .arg(&case.harness_path)
        .arg("-L")
        .arg(format!("native={}", temp.path().display()))
        .arg("-l")
        .arg(format!("static={}", case.name))
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap_or_else(|_| {
            panic!(
                "run Rust FFI harness compiler `{}` for `{}`",
                rustc.display(),
                case.name
            )
        });
    let compile_stdout = normalize(&String::from_utf8_lossy(&compile.stdout));
    let compile_stderr = normalize(&String::from_utf8_lossy(&compile.stderr));
    if compile.status.code().is_none_or(|code| code != 0) {
        return Err(format!(
            "[{}] expected Rust static FFI harness link to succeed, got {:?}\nstdout:\n{}\nstderr:\n{}",
            case.name,
            compile.status.code(),
            compile_stdout,
            compile_stderr
        ));
    }
    if !executable.is_file() {
        return Err(format!(
            "[{}] expected Rust executable `{}` to exist after static link",
            case.name,
            executable.display()
        ));
    }

    let run = Command::new(&executable)
        .current_dir(workspace_root)
        .output()
        .unwrap_or_else(|_| panic!("run Rust FFI executable `{}`", executable.display()));
    let run_stdout = normalize(&String::from_utf8_lossy(&run.stdout));
    let run_stderr = normalize(&String::from_utf8_lossy(&run.stderr));
    if run.status.code().is_none_or(|code| code != 0) {
        return Err(format!(
            "[{}] expected Rust FFI executable to exit with 0, got {:?}\nstdout:\n{}\nstderr:\n{}",
            case.name,
            run.status.code(),
            run_stdout,
            run_stderr
        ));
    }

    Ok(())
}

fn run_cargo_rust_ffi_case(
    workspace_root: &Path,
    cargo: &Path,
    case: &FfiCase,
) -> Result<(), String> {
    let temp = TempDir::new(&format!("ql-ffi-rust-cargo-{}", case.name));
    let staticlib = static_library_output_path(temp.path(), &case.name);
    let relative_ql = relative_ql_path(workspace_root, &case.ql_path);

    run_ql_build(
        workspace_root,
        &case.name,
        &relative_ql,
        "staticlib",
        &staticlib,
        HeaderSurface::Exports,
        None,
    )?;
    if !staticlib.is_file() {
        return Err(format!(
            "[{}] expected static library `{}` to exist after build",
            case.name,
            staticlib.display()
        ));
    }

    let project_dir = temp.path().join("cargo-host");
    write_rust_cargo_project(&project_dir, case, temp.path())?;

    let cargo_run = Command::new(cargo)
        .current_dir(&project_dir)
        .env("CARGO_TARGET_DIR", project_dir.join("target"))
        .args(["run", "--quiet"])
        .output()
        .unwrap_or_else(|_| {
            panic!(
                "run Cargo-based Rust FFI harness `{}` for `{}`",
                cargo.display(),
                case.name
            )
        });
    let cargo_stdout = normalize(&String::from_utf8_lossy(&cargo_run.stdout));
    let cargo_stderr = normalize(&String::from_utf8_lossy(&cargo_run.stderr));
    if cargo_run.status.code().is_none_or(|code| code != 0) {
        return Err(format!(
            "[{}] expected Cargo-based Rust FFI harness to succeed, got {:?}\nstdout:\n{}\nstderr:\n{}",
            case.name,
            cargo_run.status.code(),
            cargo_stdout,
            cargo_stderr
        ));
    }

    Ok(())
}

fn run_committed_rust_example(
    example_root: &Path,
    cargo: &Path,
    expected_stdout_fragments: &[&str],
) -> Result<(), String> {
    let temp = TempDir::new("ql-ffi-rust-example");
    let copied_root = temp.path().join("ffi-rust");
    copy_directory_recursive(example_root, &copied_root)?;

    let host_dir = copied_root.join("host");
    let cargo_run = Command::new(cargo)
        .current_dir(&host_dir)
        .env("QLANG_BIN", env!("CARGO_BIN_EXE_ql"))
        .env("CARGO_TARGET_DIR", host_dir.join("target"))
        .args(["run", "--quiet"])
        .output()
        .unwrap_or_else(|_| {
            panic!(
                "run committed Rust FFI example with Cargo `{}`",
                cargo.display()
            )
        });
    let cargo_stdout = normalize(&String::from_utf8_lossy(&cargo_run.stdout));
    let cargo_stderr = normalize(&String::from_utf8_lossy(&cargo_run.stderr));
    if cargo_run.status.code().is_none_or(|code| code != 0) {
        return Err(format!(
            "[ffi-rust-example] expected committed Cargo host example to succeed, got {:?}\nstdout:\n{}\nstderr:\n{}",
            cargo_run.status.code(),
            cargo_stdout,
            cargo_stderr
        ));
    }
    assert_stdout_contains_all("ffi-rust-example", &cargo_stdout, expected_stdout_fragments)?;

    Ok(())
}

fn run_committed_c_example(
    workspace_root: &Path,
    clang: &Path,
    ql_source: &Path,
    host_source: &Path,
    expected_stdout_fragments: &[&str],
) -> Result<(), String> {
    let temp = TempDir::new("ql-ffi-c-example");
    let stem = ql_source
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("ffi_c_example");
    let header_surface = HeaderSurface::Both;
    let header = header_output_path(temp.path(), stem, header_surface);
    let staticlib = static_library_output_path(temp.path(), stem);
    let executable = executable_output_path(temp.path(), "ffi_c_host");
    let relative_ql = relative_ql_path(workspace_root, ql_source);

    run_ql_build(
        workspace_root,
        "ffi-c-example",
        &relative_ql,
        "staticlib",
        &staticlib,
        header_surface,
        Some(&header),
    )?;
    if !staticlib.is_file() {
        return Err(format!(
            "[ffi-c-example] expected static library `{}` to exist after build",
            staticlib.display()
        ));
    }
    if !header.is_file() {
        return Err(format!(
            "[ffi-c-example] expected generated header `{}` to exist after build",
            header.display()
        ));
    }

    let compile = Command::new(clang)
        .current_dir(workspace_root)
        .arg("-I")
        .arg(temp.path())
        .arg(host_source)
        .arg(&staticlib)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap_or_else(|_| {
            panic!(
                "run committed C FFI example compiler `{}` for `{}`",
                clang.display(),
                host_source.display()
            )
        });
    let compile_stdout = normalize(&String::from_utf8_lossy(&compile.stdout));
    let compile_stderr = normalize(&String::from_utf8_lossy(&compile.stderr));
    if compile.status.code().is_none_or(|code| code != 0) {
        return Err(format!(
            "[ffi-c-example] expected committed C FFI example link to succeed, got {:?}\nstdout:\n{}\nstderr:\n{}",
            compile.status.code(),
            compile_stdout,
            compile_stderr
        ));
    }
    if !executable.is_file() {
        return Err(format!(
            "[ffi-c-example] expected executable `{}` to exist after C host link",
            executable.display()
        ));
    }

    let run = Command::new(&executable)
        .current_dir(workspace_root)
        .output()
        .unwrap_or_else(|_| panic!("run committed C FFI example `{}`", executable.display()));
    let run_stdout = normalize(&String::from_utf8_lossy(&run.stdout));
    let run_stderr = normalize(&String::from_utf8_lossy(&run.stderr));
    if run.status.code().is_none_or(|code| code != 0) {
        return Err(format!(
            "[ffi-c-example] expected committed C FFI example to exit with 0, got {:?}\nstdout:\n{}\nstderr:\n{}",
            run.status.code(),
            run_stdout,
            run_stderr
        ));
    }
    assert_stdout_contains_all("ffi-c-example", &run_stdout, expected_stdout_fragments)?;
    if !run_stderr.trim().is_empty() {
        return Err(format!(
            "[ffi-c-example] expected committed C FFI example stderr to be empty, got:\n{}",
            run_stderr
        ));
    }

    Ok(())
}

fn run_committed_c_dylib_example(
    workspace_root: &Path,
    clang: &Path,
    ql_source: &Path,
    host_source: &Path,
    expected_stdout_fragments: &[&str],
) -> Result<(), String> {
    let temp = TempDir::new("ql-ffi-c-dylib-example");
    let stem = ql_source
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("ffi_c_dylib_example");
    let dynamic_library = dynamic_library_output_path(temp.path(), stem);
    let executable = executable_output_path(temp.path(), "ffi_c_dylib_host");
    let relative_ql = relative_ql_path(workspace_root, ql_source);

    run_ql_build(
        workspace_root,
        "ffi-c-dylib-example",
        &relative_ql,
        "dylib",
        &dynamic_library,
        HeaderSurface::Exports,
        None,
    )?;
    if !dynamic_library.is_file() {
        return Err(format!(
            "[ffi-c-dylib-example] expected dynamic library `{}` to exist after build",
            dynamic_library.display()
        ));
    }

    let mut compile = Command::new(clang);
    compile
        .current_dir(workspace_root)
        .arg(host_source)
        .arg("-o")
        .arg(&executable);
    if cfg!(target_os = "linux") {
        compile.arg("-ldl");
    }
    let compile = compile.output().unwrap_or_else(|_| {
        panic!(
            "run committed C dylib FFI example compiler `{}` for `{}`",
            clang.display(),
            host_source.display()
        )
    });
    let compile_stdout = normalize(&String::from_utf8_lossy(&compile.stdout));
    let compile_stderr = normalize(&String::from_utf8_lossy(&compile.stderr));
    if compile.status.code().is_none_or(|code| code != 0) {
        return Err(format!(
            "[ffi-c-dylib-example] expected committed C dylib FFI example build to succeed, got {:?}\nstdout:\n{}\nstderr:\n{}",
            compile.status.code(),
            compile_stdout,
            compile_stderr
        ));
    }
    if !executable.is_file() {
        return Err(format!(
            "[ffi-c-dylib-example] expected executable `{}` to exist after C host build",
            executable.display()
        ));
    }

    let run = Command::new(&executable)
        .current_dir(workspace_root)
        .arg(&dynamic_library)
        .output()
        .unwrap_or_else(|_| {
            panic!(
                "run committed C dylib FFI example `{}`",
                executable.display()
            )
        });
    let run_stdout = normalize(&String::from_utf8_lossy(&run.stdout));
    let run_stderr = normalize(&String::from_utf8_lossy(&run.stderr));
    if run.status.code().is_none_or(|code| code != 0) {
        return Err(format!(
            "[ffi-c-dylib-example] expected committed C dylib FFI example to exit with 0, got {:?}\nstdout:\n{}\nstderr:\n{}",
            run.status.code(),
            run_stdout,
            run_stderr
        ));
    }
    assert_stdout_contains_all(
        "ffi-c-dylib-example",
        &run_stdout,
        expected_stdout_fragments,
    )?;
    if !run_stderr.trim().is_empty() {
        return Err(format!(
            "[ffi-c-dylib-example] expected committed C dylib FFI example stderr to be empty, got:\n{}",
            run_stderr
        ));
    }

    Ok(())
}

fn assert_stdout_contains_all(
    case_name: &str,
    stdout: &str,
    expected_fragments: &[&str],
) -> Result<(), String> {
    for fragment in expected_fragments {
        if !stdout.contains(fragment) {
            return Err(format!(
                "[{case_name}] expected stdout to contain `{fragment}`, got:\n{}",
                stdout
            ));
        }
    }
    Ok(())
}

fn read_header_surface_metadata(ql_path: &Path) -> HeaderSurface {
    let metadata_path = ql_path.with_extension("header-surface");
    if !metadata_path.is_file() {
        return HeaderSurface::Exports;
    }

    let contents = fs::read_to_string(&metadata_path).unwrap_or_else(|_| {
        panic!(
            "read FFI header-surface metadata `{}`",
            metadata_path.display()
        )
    });
    HeaderSurface::parse(contents.trim()).unwrap_or_else(|| {
        panic!(
            "unsupported FFI header-surface `{}` in `{}`",
            contents.trim(),
            metadata_path.display()
        )
    })
}
