use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn codegen_snapshots_match() {
    let workspace_root = workspace_root();

    let pass_cases = vec![
        PassCase {
            name: "minimal_build_llvm_ir",
            source_relative: "fixtures/codegen/pass/minimal_build.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/minimal_build.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "extern_c_build_llvm_ir",
            source_relative: "fixtures/codegen/pass/extern_c_build.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/extern_c_build.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "extern_c_export_llvm_ir",
            source_relative: "fixtures/codegen/pass/extern_c_export.ql",
            emit: "llvm-ir",
            expected_relative: "tests/codegen/pass/extern_c_export.ll",
            mock_compiler: false,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "minimal_build_object",
            source_relative: "fixtures/codegen/pass/minimal_build.ql",
            emit: "obj",
            expected_relative: "tests/codegen/pass/minimal_build.obj.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "minimal_build_executable",
            source_relative: "fixtures/codegen/pass/minimal_build.ql",
            emit: "exe",
            expected_relative: "tests/codegen/pass/minimal_build.exe.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "extern_c_export_dylib",
            source_relative: "fixtures/codegen/pass/extern_c_export.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "extern_c_export_dylib_with_header",
            source_relative: "fixtures/codegen/pass/extern_c_export.ql",
            emit: "dylib",
            expected_relative: "tests/codegen/pass/extern_c_export.dylib.txt",
            mock_compiler: true,
            mock_archiver: false,
            archiver_style: None,
            header_surface: Some("exports"),
            expected_header_relative: Some("tests/codegen/pass/extern_c_export.h"),
        },
        PassCase {
            name: "minimal_library_staticlib",
            source_relative: "fixtures/codegen/pass/minimal_library.ql",
            emit: "staticlib",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            mock_compiler: true,
            mock_archiver: true,
            archiver_style: Some(current_archiver_style()),
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_library_scalar_await_staticlib",
            source_relative: "fixtures/codegen/pass/async_library_scalar_await.ql",
            emit: "staticlib",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            mock_compiler: true,
            mock_archiver: true,
            archiver_style: Some(current_archiver_style()),
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_library_tuple_await_staticlib",
            source_relative: "fixtures/codegen/pass/async_library_tuple_await.ql",
            emit: "staticlib",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            mock_compiler: true,
            mock_archiver: true,
            archiver_style: Some(current_archiver_style()),
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "async_library_spawn_staticlib",
            source_relative: "fixtures/codegen/pass/async_library_spawn.ql",
            emit: "staticlib",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            mock_compiler: true,
            mock_archiver: true,
            archiver_style: Some(current_archiver_style()),
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "ffi_export_async_staticlib_with_header",
            source_relative: "fixtures/codegen/pass/ffi_export_async.ql",
            emit: "staticlib",
            expected_relative: "tests/codegen/pass/minimal_library.staticlib.txt",
            mock_compiler: true,
            mock_archiver: true,
            archiver_style: Some(current_archiver_style()),
            header_surface: Some("exports"),
            expected_header_relative: Some("tests/codegen/pass/ffi_export_async.h"),
        },
        PassCase {
            name: "extern_c_library_staticlib",
            source_relative: "fixtures/codegen/pass/extern_c_library.ql",
            emit: "staticlib",
            expected_relative: "tests/codegen/pass/extern_c_library.staticlib.txt",
            mock_compiler: true,
            mock_archiver: true,
            archiver_style: Some(current_archiver_style()),
            header_surface: None,
            expected_header_relative: None,
        },
        PassCase {
            name: "extern_c_library_staticlib_with_import_header",
            source_relative: "fixtures/codegen/pass/extern_c_library.ql",
            emit: "staticlib",
            expected_relative: "tests/codegen/pass/extern_c_library.staticlib.txt",
            mock_compiler: true,
            mock_archiver: true,
            archiver_style: Some(current_archiver_style()),
            header_surface: Some("imports"),
            expected_header_relative: Some("tests/codegen/pass/extern_c_library.imports.h"),
        },
        PassCase {
            name: "extern_c_import_top_level_staticlib_with_both_header",
            source_relative: "tests/ffi/pass/extern_c_import_top_level.ql",
            emit: "staticlib",
            expected_relative: "tests/codegen/pass/extern_c_import_top_level.staticlib.txt",
            mock_compiler: true,
            mock_archiver: true,
            archiver_style: Some(current_archiver_style()),
            header_surface: Some("both"),
            expected_header_relative: Some("tests/codegen/pass/extern_c_import_top_level.ffi.h"),
        },
        PassCase {
            name: "extern_c_top_level_library_staticlib",
            source_relative: "fixtures/codegen/pass/extern_c_top_level_library.ql",
            emit: "staticlib",
            expected_relative: "tests/codegen/pass/extern_c_top_level_library.staticlib.txt",
            mock_compiler: true,
            mock_archiver: true,
            archiver_style: Some(current_archiver_style()),
            header_surface: None,
            expected_header_relative: None,
        },
    ];
    let fail_cases = vec![
        FailCase {
            name: "unsupported_closure_build",
            source_relative: "tests/codegen/fail/unsupported_closure_build.ql",
            emit: "llvm-ir",
            expected_stderr_relative: "tests/codegen/fail/unsupported_closure_build.stderr",
            extra_args: &[],
        },
        FailCase {
            name: "unsupported_extern_rust_abi_build",
            source_relative: "tests/codegen/fail/unsupported_extern_rust_abi_build.ql",
            emit: "llvm-ir",
            expected_stderr_relative: "tests/codegen/fail/unsupported_extern_rust_abi_build.stderr",
            extra_args: &[],
        },
        FailCase {
            name: "unsupported_extern_rust_abi_definition_build",
            source_relative: "tests/codegen/fail/unsupported_extern_rust_abi_definition_build.ql",
            emit: "llvm-ir",
            expected_stderr_relative: "tests/codegen/fail/unsupported_extern_rust_abi_definition_build.stderr",
            extra_args: &[],
        },
        FailCase {
            name: "unsupported_function_value_build",
            source_relative: "tests/codegen/fail/unsupported_function_value_build.ql",
            emit: "llvm-ir",
            expected_stderr_relative: "tests/codegen/fail/unsupported_function_value_build.stderr",
            extra_args: &[],
        },
        FailCase {
            name: "unsupported_match_build",
            source_relative: "tests/codegen/fail/unsupported_match_build.ql",
            emit: "llvm-ir",
            expected_stderr_relative: "tests/codegen/fail/unsupported_match_build.stderr",
            extra_args: &[],
        },
        FailCase {
            name: "unsupported_for_build",
            source_relative: "tests/codegen/fail/unsupported_for_build.ql",
            emit: "llvm-ir",
            expected_stderr_relative: "tests/codegen/fail/unsupported_for_build.stderr",
            extra_args: &[],
        },
        FailCase {
            name: "unsupported_cleanup_build",
            source_relative: "tests/codegen/fail/unsupported_cleanup_build.ql",
            emit: "llvm-ir",
            expected_stderr_relative: "tests/codegen/fail/unsupported_cleanup_build.stderr",
            extra_args: &[],
        },
        FailCase {
            name: "unsupported_async_fn_build",
            source_relative: "tests/codegen/fail/unsupported_async_fn_build.ql",
            emit: "llvm-ir",
            expected_stderr_relative: "tests/codegen/fail/unsupported_async_fn_build.stderr",
            extra_args: &[],
        },
        FailCase {
            name: "unsupported_async_generic_main_build",
            source_relative: "tests/codegen/fail/unsupported_async_generic_main_build.ql",
            emit: "llvm-ir",
            expected_stderr_relative: "tests/codegen/fail/unsupported_async_generic_main_build.stderr",
            extra_args: &[],
        },
        FailCase {
            name: "unsupported_async_unsafe_main_build",
            source_relative: "tests/codegen/fail/unsupported_async_unsafe_main_build.ql",
            emit: "llvm-ir",
            expected_stderr_relative: "tests/codegen/fail/unsupported_async_unsafe_main_build.stderr",
            extra_args: &[],
        },
        FailCase {
            name: "unsupported_async_fn_dylib_build",
            source_relative: "tests/codegen/fail/unsupported_async_fn_dylib_build.ql",
            emit: "dylib",
            expected_stderr_relative: "tests/codegen/fail/unsupported_async_fn_dylib_build.stderr",
            extra_args: &[],
        },
        FailCase {
            name: "unsupported_async_spawn_build",
            source_relative: "tests/codegen/fail/unsupported_async_spawn_build.ql",
            emit: "llvm-ir",
            expected_stderr_relative: "tests/codegen/fail/unsupported_async_spawn_build.stderr",
            extra_args: &[],
        },
        FailCase {
            name: "unsupported_async_for_await_build",
            source_relative: "tests/codegen/fail/unsupported_async_for_await_build.ql",
            emit: "llvm-ir",
            expected_stderr_relative: "tests/codegen/fail/unsupported_async_for_await_build.stderr",
            extra_args: &[],
        },
        FailCase {
            name: "unsupported_async_for_await_library_build",
            source_relative: "tests/codegen/fail/unsupported_async_for_await_library_build.ql",
            emit: "staticlib",
            expected_stderr_relative: "tests/codegen/fail/unsupported_async_for_await_library_build.stderr",
            extra_args: &[],
        },
        FailCase {
            name: "unsupported_async_spawn_value_library_build",
            source_relative: "tests/codegen/fail/unsupported_async_spawn_value_library_build.ql",
            emit: "staticlib",
            expected_stderr_relative: "tests/codegen/fail/unsupported_async_spawn_value_library_build.stderr",
            extra_args: &[],
        },
        FailCase {
            name: "unsupported_deferred_multi_segment_type_build",
            source_relative: "tests/codegen/fail/unsupported_deferred_multi_segment_type_build.ql",
            emit: "dylib",
            expected_stderr_relative: "tests/codegen/fail/unsupported_deferred_multi_segment_type_build.stderr",
            extra_args: &[],
        },
        FailCase {
            name: "dylib_requires_export_build",
            source_relative: "tests/codegen/fail/dylib_requires_export_build.ql",
            emit: "dylib",
            expected_stderr_relative: "tests/codegen/fail/dylib_requires_export_build.stderr",
            extra_args: &[],
        },
        FailCase {
            name: "executable_header_build",
            source_relative: "fixtures/codegen/pass/minimal_build.ql",
            emit: "exe",
            expected_stderr_relative: "tests/codegen/fail/executable_header_build.stderr",
            extra_args: &["--header"],
        },
    ];

    let mut failures = Vec::new();

    for case in pass_cases {
        if let Err(message) = run_pass_case(&workspace_root, &case) {
            failures.push(message);
        }
    }

    for case in fail_cases {
        if let Err(message) = run_fail_case(&workspace_root, &case) {
            failures.push(message);
        }
    }

    assert!(
        failures.is_empty(),
        "codegen snapshot regressions:\n\n{}",
        failures.join("\n\n")
    );
}

#[derive(Clone, Copy)]
struct PassCase {
    name: &'static str,
    source_relative: &'static str,
    emit: &'static str,
    expected_relative: &'static str,
    mock_compiler: bool,
    mock_archiver: bool,
    archiver_style: Option<&'static str>,
    header_surface: Option<&'static str>,
    expected_header_relative: Option<&'static str>,
}

#[derive(Clone, Copy)]
struct FailCase {
    name: &'static str,
    source_relative: &'static str,
    emit: &'static str,
    expected_stderr_relative: &'static str,
    extra_args: &'static [&'static str],
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
        fs::create_dir_all(&path).expect("create temporary test directory");
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

fn run_pass_case(workspace_root: &Path, case: &PassCase) -> Result<(), String> {
    let temp = TempDir::new(&format!("ql-codegen-{}", case.name));
    let output_path = artifact_output_path(temp.path(), case.emit);
    let expected_path = workspace_root.join(case.expected_relative);
    let expected = normalize_artifact(&render_expected_snapshot(&normalize(
        &fs::read_to_string(&expected_path)
            .unwrap_or_else(|_| panic!("read expected snapshot `{}`", expected_path.display())),
    )));

    let mut command = Command::new(env!("CARGO_BIN_EXE_ql"));
    command.current_dir(workspace_root).args([
        "build",
        case.source_relative,
        "--emit",
        case.emit,
        "--output",
        &output_path.to_string_lossy(),
    ]);
    if let Some(surface) = case.header_surface {
        if surface == "exports" {
            command.arg("--header");
        } else {
            command.args(["--header-surface", surface]);
        }
    }

    let mut compiler_wrapper = None;
    if case.mock_compiler {
        compiler_wrapper = Some(make_mock_compiler_wrapper(temp.path()));
    }
    if let Some(wrapper) = &compiler_wrapper {
        command.env("QLANG_CLANG", wrapper);
    }

    let mut archiver_wrapper = None;
    if case.mock_archiver {
        archiver_wrapper = Some(make_mock_archiver_wrapper(temp.path()));
    }
    if let Some(wrapper) = &archiver_wrapper {
        command.env("QLANG_AR", wrapper);
    }
    if let Some(style) = case.archiver_style {
        command.env("QLANG_AR_STYLE", style);
    }

    let output = command.output().unwrap_or_else(|_| {
        panic!(
            "run `ql build {} --emit {}`",
            case.source_relative, case.emit
        )
    });
    let stdout = normalize(&String::from_utf8_lossy(&output.stdout));
    let stderr = normalize(&String::from_utf8_lossy(&output.stderr));

    if output.status.code().is_none_or(|code| code != 0) {
        return Err(format!(
            "[{}] expected exit code 0, got {:?}\nstdout:\n{}\nstderr:\n{}",
            case.name,
            output.status.code(),
            stdout,
            stderr
        ));
    }

    if !stderr.trim().is_empty() {
        return Err(format!(
            "[{}] expected no stderr for successful build\nstderr:\n{}",
            case.name, stderr
        ));
    }

    let actual = normalize_artifact(&normalize(
        &fs::read_to_string(&output_path)
            .unwrap_or_else(|_| panic!("read generated artifact `{}`", output_path.display())),
    ));
    if actual != expected {
        return Err(format!(
            "[{}] artifact snapshot mismatch\n--- expected ---\n{}\n--- actual ---\n{}",
            case.name, expected, actual
        ));
    }

    if let Some(expected_header_relative) = case.expected_header_relative {
        let expected_header_path = workspace_root.join(expected_header_relative);
        let expected_header = normalize_artifact(&normalize(
            &fs::read_to_string(&expected_header_path).unwrap_or_else(|_| {
                panic!(
                    "read expected header snapshot `{}`",
                    expected_header_path.display()
                )
            }),
        ));
        let surface = case
            .header_surface
            .expect("header snapshots require an explicit surface");
        let header_output_path =
            default_sidecar_header_output_path(&output_path, case.source_relative, surface);
        let actual_header = normalize_artifact(&normalize(
            &fs::read_to_string(&header_output_path).unwrap_or_else(|_| {
                panic!("read generated header `{}`", header_output_path.display())
            }),
        ));
        if actual_header != expected_header {
            return Err(format!(
                "[{}] header snapshot mismatch\n--- expected ---\n{}\n--- actual ---\n{}",
                case.name, expected_header, actual_header
            ));
        }
    }

    let leftovers = fs::read_dir(temp.path())
        .unwrap_or_else(|_| panic!("read temp dir `{}`", temp.path().display()))
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.contains(".codegen."))
        })
        .collect::<Vec<_>>();
    if !leftovers.is_empty() {
        let rendered = leftovers
            .into_iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!(
            "[{}] expected no preserved intermediate artifacts on success, found: {}",
            case.name, rendered
        ));
    }

    Ok(())
}

fn run_fail_case(workspace_root: &Path, case: &FailCase) -> Result<(), String> {
    let expected_path = workspace_root.join(case.expected_stderr_relative);
    let expected = normalize(&fs::read_to_string(&expected_path).unwrap_or_else(|_| {
        panic!(
            "read expected stderr snapshot `{}`",
            expected_path.display()
        )
    }));

    let mut command = Command::new(env!("CARGO_BIN_EXE_ql"));
    command
        .current_dir(workspace_root)
        .args(["build", case.source_relative, "--emit", case.emit]);
    command.args(case.extra_args);
    let output = command.output().unwrap_or_else(|_| {
        panic!(
            "run `ql build {} --emit {}`",
            case.source_relative, case.emit
        )
    });

    let stdout = normalize(&String::from_utf8_lossy(&output.stdout));
    let stderr = normalize(&String::from_utf8_lossy(&output.stderr));

    if output.status.code().is_none_or(|code| code != 1) {
        return Err(format!(
            "[{}] expected exit code 1, got {:?}\nstdout:\n{}\nstderr:\n{}",
            case.name,
            output.status.code(),
            stdout,
            stderr
        ));
    }

    if !stdout.trim().is_empty() {
        return Err(format!(
            "[{}] expected no stdout for failing build\nstdout:\n{}",
            case.name, stdout
        ));
    }

    if stderr != expected {
        return Err(format!(
            "[{}] stderr snapshot mismatch\n--- expected ---\n{}\n--- actual ---\n{}",
            case.name, expected, stderr
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

fn artifact_output_path(root: &Path, emit: &str) -> PathBuf {
    match emit {
        "llvm-ir" => root.join("artifact.ll"),
        "obj" => root.join(if cfg!(windows) {
            "artifact.obj"
        } else {
            "artifact.o"
        }),
        "exe" => root.join(if cfg!(windows) {
            "artifact.exe"
        } else {
            "artifact"
        }),
        "dylib" => root.join(if cfg!(windows) {
            "artifact.dll"
        } else if cfg!(target_os = "macos") {
            "libartifact.dylib"
        } else {
            "libartifact.so"
        }),
        "staticlib" => root.join(if cfg!(windows) {
            "artifact.lib"
        } else {
            "libartifact.a"
        }),
        other => panic!("unsupported emit kind `{other}`"),
    }
}

fn default_sidecar_header_output_path(
    artifact_path: &Path,
    source_relative: &str,
    surface: &str,
) -> PathBuf {
    let stem = Path::new(source_relative)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("module");
    let file_name = match surface {
        "exports" => format!("{stem}.h"),
        "imports" => format!("{stem}.imports.h"),
        "both" => format!("{stem}.ffi.h"),
        other => panic!("unsupported header surface `{other}`"),
    };

    artifact_path
        .parent()
        .expect("artifact output should have a parent directory")
        .join(file_name)
}

fn render_expected_snapshot(snapshot: &str) -> String {
    snapshot.replace("{{TARGET_TRIPLE}}", current_target_triple())
}

fn current_target_triple() -> &'static str {
    match (env::consts::ARCH, env::consts::OS) {
        ("x86_64", "windows") => "x86_64-pc-windows-msvc",
        ("x86_64", "linux") => "x86_64-pc-linux-gnu",
        ("aarch64", "macos") => "aarch64-apple-darwin",
        ("x86_64", "macos") => "x86_64-apple-darwin",
        ("aarch64", "linux") => "aarch64-unknown-linux-gnu",
        _ => "unknown-unknown-unknown",
    }
}

fn current_archiver_style() -> &'static str {
    if cfg!(windows) { "lib" } else { "ar" }
}

fn normalize(text: &str) -> String {
    text.replace("\r\n", "\n")
}

fn normalize_artifact(text: &str) -> String {
    normalize(text).trim_end().to_owned()
}

fn make_mock_compiler_wrapper(root: &Path) -> PathBuf {
    if cfg!(windows) {
        let script = root.join("mock-clang.ps1");
        fs::write(
            &script,
            r#"
param([string[]]$args)
$out = $null
$isCompile = $false
$isShared = $false
for ($i = 0; $i -lt $args.Count; $i++) {
    if ($args[$i] -eq '-c') { $isCompile = $true }
    if ($args[$i] -eq '-shared' -or $args[$i] -eq '-dynamiclib') { $isShared = $true }
    if ($args[$i] -eq '-o') { $out = $args[$i + 1] }
}
if ($null -eq $out) { Write-Error 'missing -o'; exit 1 }
if ($isCompile) {
    Set-Content -Path $out -NoNewline -Value 'mock-object'
} elseif ($isShared) {
    Set-Content -Path $out -NoNewline -Value 'mock-dylib'
} else {
    Set-Content -Path $out -NoNewline -Value 'mock-executable'
}
"#,
        )
        .expect("write mock clang powershell script");
        let wrapper = root.join("mock-clang.cmd");
        fs::write(
            &wrapper,
            format!(
                "@echo off\r\npowershell.exe -ExecutionPolicy Bypass -File \"{}\" %*\r\n",
                script.display()
            ),
        )
        .expect("write mock clang wrapper");
        wrapper
    } else {
        let script = root.join("mock-clang.sh");
        fs::write(
            &script,
            r#"#!/bin/sh
out=""
is_compile=0
is_shared=0
while [ "$#" -gt 0 ]; do
  if [ "$1" = "-c" ]; then
    is_compile=1
    shift
    continue
  fi
  if [ "$1" = "-shared" ] || [ "$1" = "-dynamiclib" ]; then
    is_shared=1
    shift
    continue
  fi
  if [ "$1" = "-o" ]; then
    out="$2"
    shift 2
    continue
  fi
  shift
done
if [ "$out" = "" ]; then
  echo "missing -o" 1>&2
  exit 1
fi
if [ "$is_compile" -eq 1 ]; then
  printf 'mock-object' > "$out"
elif [ "$is_shared" -eq 1 ]; then
  printf 'mock-dylib' > "$out"
else
  printf 'mock-executable' > "$out"
fi
"#,
        )
        .expect("write mock clang shell script");
        make_executable(&script);
        script
    }
}

fn make_mock_archiver_wrapper(root: &Path) -> PathBuf {
    if cfg!(windows) {
        let script = root.join("mock-archiver.ps1");
        fs::write(
            &script,
            r#"
param([string[]]$args)
$out = $null
for ($i = 0; $i -lt $args.Count; $i++) {
    if ($args[$i] -like '/OUT:*') { $out = $args[$i].Substring(5) }
}
if ($null -eq $out) { Write-Error 'missing /OUT'; exit 1 }
Set-Content -Path $out -NoNewline -Value 'mock-staticlib'
"#,
        )
        .expect("write mock archiver powershell script");
        let wrapper = root.join("mock-archiver.cmd");
        fs::write(
            &wrapper,
            format!(
                "@echo off\r\npowershell.exe -ExecutionPolicy Bypass -File \"{}\" %*\r\n",
                script.display()
            ),
        )
        .expect("write mock archiver wrapper");
        wrapper
    } else {
        let script = root.join("mock-archiver.sh");
        fs::write(
            &script,
            r#"#!/bin/sh
out="$2"
printf 'mock-staticlib' > "$out"
"#,
        )
        .expect("write mock archiver shell script");
        make_executable(&script);
        script
    }
}

#[cfg(unix)]
fn make_executable(path: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path)
        .unwrap_or_else(|_| panic!("read metadata for `{}`", path.display()))
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)
        .unwrap_or_else(|_| panic!("set executable bit on `{}`", path.display()));
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) {}
