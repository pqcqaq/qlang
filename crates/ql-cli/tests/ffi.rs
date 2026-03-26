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
    let header = header_output_path(temp.path(), &case.name);
    let staticlib = static_library_output_path(temp.path(), &case.name);
    let executable = executable_output_path(temp.path(), &case.name);
    let relative_ql = relative_ql_path(workspace_root, &case.ql_path);

    run_ql_build(
        workspace_root,
        &case.name,
        &relative_ql,
        "staticlib",
        &staticlib,
    )?;
    if !staticlib.is_file() {
        return Err(format!(
            "[{}] expected static library `{}` to exist after build",
            case.name,
            staticlib.display()
        ));
    }

    run_ql_ffi_header(workspace_root, &case.name, &relative_ql, &header)?;
    if !header.is_file() {
        return Err(format!(
            "[{}] expected generated header `{}` to exist after `ql ffi header`",
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
    let header = header_output_path(temp.path(), &case.name);
    let dynamic_library = dynamic_library_output_path(temp.path(), &case.name);
    let executable = executable_output_path(temp.path(), &format!("{}_shared", case.name));
    let relative_ql = relative_ql_path(workspace_root, &case.ql_path);

    run_ql_build(
        workspace_root,
        &case.name,
        &relative_ql,
        "dylib",
        &dynamic_library,
    )?;
    if !dynamic_library.is_file() {
        return Err(format!(
            "[{}] expected dynamic library `{}` to exist after build",
            case.name,
            dynamic_library.display()
        ));
    }

    run_ql_ffi_header(workspace_root, &case.name, &relative_ql, &header)?;
    if !header.is_file() {
        return Err(format!(
            "[{}] expected generated header `{}` to exist after `ql ffi header`",
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
        cases.push(FfiCase {
            name,
            ql_path: path,
            harness_path,
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
        cases.push(FfiCase {
            name,
            ql_path: path,
            harness_path,
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

fn header_output_path(root: &Path, stem: &str) -> PathBuf {
    root.join(format!("{stem}.h"))
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

fn run_ql_build(
    workspace_root: &Path,
    case_name: &str,
    relative_ql: &str,
    emit: &str,
    output_path: &Path,
) -> Result<(), String> {
    let build = Command::new(env!("CARGO_BIN_EXE_ql"))
        .current_dir(workspace_root)
        .args([
            "build",
            relative_ql,
            "--emit",
            emit,
            "--output",
            &output_path.to_string_lossy(),
        ])
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

fn run_ql_ffi_header(
    workspace_root: &Path,
    case_name: &str,
    relative_ql: &str,
    header_path: &Path,
) -> Result<(), String> {
    let header_emit = Command::new(env!("CARGO_BIN_EXE_ql"))
        .current_dir(workspace_root)
        .args([
            "ffi",
            "header",
            relative_ql,
            "--output",
            &header_path.to_string_lossy(),
        ])
        .output()
        .unwrap_or_else(|_| panic!("run `ql ffi header {relative_ql}`"));
    let header_stdout = normalize(&String::from_utf8_lossy(&header_emit.stdout));
    let header_stderr = normalize(&String::from_utf8_lossy(&header_emit.stderr));
    if header_emit.status.code().is_none_or(|code| code != 0) {
        return Err(format!(
            "[{}] expected `ql ffi header` to succeed, got {:?}\nstdout:\n{}\nstderr:\n{}",
            case_name,
            header_emit.status.code(),
            header_stdout,
            header_stderr
        ));
    }
    Ok(())
}
