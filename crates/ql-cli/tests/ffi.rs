use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

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

    let Some(clang) = resolve_program_from_env_or_path("QLANG_CLANG", &clang_candidates()) else {
        eprintln!(
            "skipping static FFI integration tests: no clang-style compiler found on PATH and `QLANG_CLANG` is not set"
        );
        return;
    };
    if resolve_program_from_env_or_path("QLANG_AR", &archiver_candidates()).is_none() {
        eprintln!(
            "skipping static FFI integration tests: no archive tool found on PATH and `QLANG_AR` is not set"
        );
        return;
    }

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
    if resolve_program_from_env_or_path("QLANG_AR", &archiver_candidates()).is_none() {
        eprintln!(
            "skipping Rust static FFI integration tests: no archive tool found on PATH and `QLANG_AR` is not set"
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
    if resolve_program_from_env_or_path("QLANG_AR", &archiver_candidates()).is_none() {
        eprintln!(
            "skipping Cargo-based Rust FFI integration tests: no archive tool found on PATH and `QLANG_AR` is not set"
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
fn ffi_exports_load_from_c_dynamic_harnesses() {
    let workspace_root = workspace_root();
    let fixture_root = workspace_root.join("tests/ffi/pass");
    let cases = collect_dynamic_ffi_cases(&fixture_root);
    assert!(
        !cases.is_empty(),
        "expected at least one shared-library FFI fixture under `{}`",
        fixture_root.display()
    );

    let Some(clang) = resolve_program_from_env_or_path("QLANG_CLANG", &clang_candidates()) else {
        eprintln!(
            "skipping shared-library FFI integration tests: no clang-style compiler found on PATH and `QLANG_CLANG` is not set"
        );
        return;
    };

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

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(prefix: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let path = env::temp_dir().join(format!("{prefix}-{unique}"));
        fs::create_dir_all(&path).expect("create temporary ffi test directory");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
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

fn workspace_root() -> PathBuf {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let crates_dir = crate_dir
        .parent()
        .expect("ql-cli crate should have a parent directory");
    crates_dir
        .parent()
        .expect("workspace root should exist")
        .to_path_buf()
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

fn clang_candidates() -> Vec<&'static str> {
    if cfg!(windows) {
        vec!["clang.exe", "clang.cmd", "clang.bat", "clang"]
    } else {
        vec!["clang"]
    }
}

fn archiver_candidates() -> Vec<&'static str> {
    if cfg!(windows) {
        vec![
            "llvm-ar.exe",
            "llvm-ar.cmd",
            "llvm-ar.bat",
            "llvm-ar",
            "llvm-lib.exe",
            "llvm-lib.cmd",
            "llvm-lib.bat",
            "llvm-lib",
            "lib.exe",
            "lib.cmd",
            "lib.bat",
            "lib",
        ]
    } else {
        vec!["llvm-ar", "ar"]
    }
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

fn static_library_output_path(root: &Path, stem: &str) -> PathBuf {
    if cfg!(windows) {
        root.join(format!("{stem}.lib"))
    } else {
        root.join(format!("lib{stem}.a"))
    }
}

fn dynamic_library_output_path(root: &Path, stem: &str) -> PathBuf {
    if cfg!(windows) {
        root.join(format!("{stem}.dll"))
    } else if cfg!(target_os = "macos") {
        root.join(format!("lib{stem}.dylib"))
    } else {
        root.join(format!("lib{stem}.so"))
    }
}

fn header_output_path(root: &Path, stem: &str, surface: HeaderSurface) -> PathBuf {
    root.join(surface.header_file_name(stem))
}

fn executable_output_path(root: &Path, stem: &str) -> PathBuf {
    if cfg!(windows) {
        root.join(format!("{stem}.exe"))
    } else {
        root.join(stem)
    }
}

fn normalize(text: &str) -> String {
    text.replace("\r\n", "\n")
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

fn run_ql_build(
    workspace_root: &Path,
    case_name: &str,
    relative_ql: &str,
    emit: &str,
    output_path: &Path,
    header_surface: HeaderSurface,
    header_path: Option<&Path>,
) -> Result<(), String> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ql"));
    command.current_dir(workspace_root).args([
        "build",
        relative_ql,
        "--emit",
        emit,
        "--output",
        &output_path.to_string_lossy(),
    ]);
    if let Some(header_path) = header_path {
        command.args(["--header-surface", header_surface.cli_value()]);
        command.args(["--header-output", &header_path.to_string_lossy()]);
    }
    let build = command
        .output()
        .unwrap_or_else(|_| panic!("run `ql build {relative_ql} --emit {emit}`"));
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
