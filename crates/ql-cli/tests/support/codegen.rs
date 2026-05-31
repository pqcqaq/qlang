use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use super::{
    TempDir, expect_empty_stderr, expect_empty_stdout, expect_exit_code, expect_file_exists,
    expect_snapshot_matches, expect_success, normalize_trimmed, ql_command, read_normalized_file,
    read_normalized_trimmed_file, run_command_capture,
};

#[derive(Clone, Copy)]
pub struct PassCase<'a> {
    pub name: &'a str,
    pub source_relative: &'a str,
    pub emit: &'a str,
    pub expected_relative: &'a str,
    pub mock_compiler: bool,
    pub mock_archiver: bool,
    pub archiver_style: Option<&'a str>,
    pub header_surface: Option<&'a str>,
    pub expected_header_relative: Option<&'a str>,
}

#[derive(Clone, Copy)]
pub struct FailCase {
    pub name: &'static str,
    pub source_relative: &'static str,
    pub emit: &'static str,
    pub expected_stderr_relative: &'static str,
    pub extra_args: &'static [&'static str],
}

pub fn run_pass_case(workspace_root: &Path, case: &PassCase<'_>) -> Result<(), String> {
    let temp_prefix = short_temp_prefix(case.name);
    let temp = TempDir::new(&temp_prefix);
    let output_path = artifact_output_path(temp.path(), case.emit);
    let expected_path = workspace_root.join(case.expected_relative);
    let expected = normalize_trimmed(&render_expected_snapshot(&read_normalized_file(
        &expected_path,
        "expected snapshot",
    )));

    let mut command = ql_command(workspace_root);
    command.args([
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

    let output = run_command_capture(
        &mut command,
        format!("`ql build {} --emit {}`", case.source_relative, case.emit),
    );
    let (_, stderr) = expect_success(case.name, "successful build", &output)?;
    expect_empty_stderr(case.name, "successful build", &stderr)?;
    expect_file_exists(
        case.name,
        &output_path,
        "generated artifact",
        "successful build",
    )?;

    let actual = read_normalized_trimmed_file(&output_path, "generated artifact");
    expect_snapshot_matches(case.name, "artifact snapshot", &expected, &actual)?;

    if let Some(expected_header_relative) = case.expected_header_relative {
        let expected_header_path = workspace_root.join(expected_header_relative);
        let expected_header =
            read_normalized_trimmed_file(&expected_header_path, "expected header snapshot");
        let surface = case
            .header_surface
            .expect("header snapshots require an explicit surface");
        let header_output_path =
            default_sidecar_header_output_path(&output_path, case.source_relative, surface);
        expect_file_exists(
            case.name,
            &header_output_path,
            "generated header",
            "successful build",
        )?;
        let actual_header = read_normalized_trimmed_file(&header_output_path, "generated header");
        expect_snapshot_matches(
            case.name,
            "header snapshot",
            &expected_header,
            &actual_header,
        )?;
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

fn short_temp_prefix(name: &str) -> String {
    format!("ql-codegen-{:016x}", stable_hash(name))
}

fn stable_hash(value: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

pub fn run_fail_case(workspace_root: &Path, case: &FailCase) -> Result<(), String> {
    let expected_path = workspace_root.join(case.expected_stderr_relative);
    let expected = read_normalized_file(&expected_path, "expected stderr snapshot");

    let mut command = ql_command(workspace_root);
    command.args(["build", case.source_relative, "--emit", case.emit]);
    command.args(case.extra_args);
    let output = run_command_capture(
        &mut command,
        format!("`ql build {} --emit {}`", case.source_relative, case.emit),
    );
    let (stdout, stderr) = expect_exit_code(case.name, "failing build", &output, 1)?;
    expect_empty_stdout(case.name, "failing build", &stdout)?;

    expect_snapshot_matches(case.name, "stderr snapshot", &expected, &stderr)?;

    Ok(())
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

pub fn current_archiver_style() -> &'static str {
    if cfg!(windows) { "lib" } else { "ar" }
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
