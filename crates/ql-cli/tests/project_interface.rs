mod support;

use support::{
    TempDir, dynamic_library_output_path, expect_empty_stdout, expect_exit_code,
    expect_file_exists, expect_snapshot_matches, expect_stderr_contains,
    expect_stderr_not_contains, expect_stdout_contains_all, expect_success, ql_command,
    read_normalized_file, run_command_capture, static_library_output_path, workspace_root,
};

#[cfg(windows)]
use std::os::windows::fs::OpenOptionsExt;

fn write_mock_clang_failure_script(temp: &TempDir) -> std::path::PathBuf {
    if cfg!(windows) {
        temp.write(
            "mock-clang-fail.cmd",
            "@echo off\r\necho mock clang failure 1>&2\r\nexit /b 9\r\n",
        )
    } else {
        let script = temp.write(
            "mock-clang-fail.sh",
            "#!/bin/sh\necho 'mock clang failure' 1>&2\nexit 9\n",
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = std::fs::metadata(&script)
                .expect("read mock clang failure script metadata")
                .permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&script, permissions)
                .expect("mark mock clang failure script executable");
        }
        script
    }
}

fn write_mock_clang_output_path_failure_script(temp: &TempDir) -> std::path::PathBuf {
    if cfg!(windows) {
        let script = temp.write(
            "mock-clang-output-path-fail.ps1",
            r#"
param([string[]]$args)
$out = $null
for ($i = 0; $i -lt $args.Count; $i++) {
    if ($args[$i] -eq '-o') { $out = $args[$i + 1] }
}
if ($null -eq $out) { Write-Error 'missing -o'; exit 1 }
Write-Error "unable to open output file '$out': Permission denied"
exit 9
"#,
        );
        temp.write(
            "mock-clang-output-path-fail.cmd",
            &format!(
                "@echo off\r\npowershell.exe -ExecutionPolicy Bypass -File \"{}\" %*\r\n",
                script.display()
            ),
        )
    } else {
        let script = temp.write(
            "mock-clang-output-path-fail.sh",
            r#"#!/bin/sh
out=""
while [ "$#" -gt 0 ]; do
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
echo "unable to open output file '$out': Permission denied" 1>&2
exit 9
"#,
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = std::fs::metadata(&script)
                .expect("read mock clang output-path failure script metadata")
                .permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&script, permissions)
                .expect("mark mock clang output-path failure script executable");
        }
        script
    }
}

fn write_mock_archiver_output_path_failure_script(temp: &TempDir) -> std::path::PathBuf {
    if cfg!(windows) {
        let script = temp.write(
            "mock-archiver-output-path-fail.ps1",
            r#"
param([string[]]$args)
$out = $null
foreach ($arg in $args) {
    if ($arg.StartsWith('/OUT:')) {
        $out = $arg.Substring(5)
    }
}
if ($null -eq $out -and $args.Count -ge 2) {
    $out = $args[1]
}
if ($null -eq $out) { Write-Error 'missing output path'; exit 1 }
Write-Error "cannot open file '$out': Permission denied"
exit 8
"#,
        );
        temp.write(
            "mock-archiver-output-path-fail.cmd",
            &format!(
                "@echo off\r\npowershell.exe -ExecutionPolicy Bypass -File \"{}\" %*\r\n",
                script.display()
            ),
        )
    } else {
        let script = temp.write(
            "mock-archiver-output-path-fail.sh",
            r#"#!/bin/sh
out=""
if [ "$#" -ge 2 ]; then
  out="$2"
fi
if [ "$out" = "" ]; then
  echo "missing output path" 1>&2
  exit 1
fi
echo "cannot open file '$out': Permission denied" 1>&2
exit 8
"#,
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = std::fs::metadata(&script)
                .expect("read mock archiver output-path failure script metadata")
                .permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&script, permissions)
                .expect("mark mock archiver output-path failure script executable");
        }
        script
    }
}

fn write_mock_clang_success_script(temp: &TempDir) -> std::path::PathBuf {
    if cfg!(windows) {
        let script = temp.write(
            "mock-clang-success.ps1",
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
        );
        temp.write(
            "mock-clang-success.cmd",
            &format!(
                "@echo off\r\npowershell.exe -ExecutionPolicy Bypass -File \"{}\" %*\r\n",
                script.display()
            ),
        )
    } else {
        let script = temp.write(
            "mock-clang-success.sh",
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
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = std::fs::metadata(&script)
                .expect("read mock clang success script metadata")
                .permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&script, permissions)
                .expect("mark mock clang success script executable");
        }
        script
    }
}

#[test]
fn project_emit_interface_writes_public_qi_surface() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-interface");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src").join("nested"))
        .expect("create project source directory for interface emit test");
    let interface_path = project_root.join("app.qi");
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace/app/src/lib.ql",
        r#"
package demo.api

pub const DEFAULT_PORT: Int = 8080
const INTERNAL_PORT: Int = 3000

pub struct Buffer[T] {
    value: T,
    count: Int = 0,
}

pub trait Writer {
    fn flush(var self) -> Int
}

impl Buffer[Int] {
    pub fn len(self) -> Int {
        return 1
    }

    fn hidden(self) -> Int {
        return 0
    }
}

extend Buffer[Int] {
    pub fn twice(self) -> Int {
        return 2
    }

    fn private_twice(self) -> Int {
        return 1
    }
}

pub fn sum(left: Int, right: Int) -> Int {
    return left + right
}

pub extern "c" {
    fn puts(ptr: *const U8) -> I32
}
"#,
    );
    temp.write(
        "workspace/app/src/nested/types.ql",
        r#"
package demo.api

pub static BUILD_ID: Int = 1

pub enum Shape {
    Unit,
    Pair(Int, Int),
}

pub type Pair = (Int, Int)
"#,
    );

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "emit-interface"])
        .arg(&project_root);
    let output = run_command_capture(&mut command, "`ql project emit-interface`");
    let (stdout, stderr) = expect_success(
        "project-interface-success",
        "project interface emission",
        &output,
    )
    .expect("project interface emission should succeed");
    expect_snapshot_matches(
        "project-interface-success",
        "project interface stdout",
        &format!("wrote interface: {}\n", interface_path.display()),
        &stdout,
    )
    .expect("interface emission should report the written artifact path");
    expect_snapshot_matches(
        "project-interface-success",
        "project interface stderr",
        "",
        &stderr,
    )
    .expect("successful interface emission should stay silent on stderr");
    expect_file_exists(
        "project-interface-success",
        &interface_path,
        "generated interface",
        "project interface emission",
    )
    .expect("interface emission should create the default package qi artifact");

    let expected = "\
// qlang interface v1
// package: app

// source: src/lib.ql
package demo.api

pub const DEFAULT_PORT: Int

pub struct Buffer[T] {
    value: T,
    count: Int,
}

pub trait Writer {
    fn flush(var self) -> Int
}

impl Buffer[Int] {
    pub fn len(self) -> Int
}

extend Buffer[Int] {
    pub fn twice(self) -> Int
}

pub fn sum(left: Int, right: Int) -> Int

pub extern \"c\" {
    fn puts(ptr: *const U8) -> I32
}

// source: src/nested/types.ql
package demo.api

pub static BUILD_ID: Int

pub enum Shape {
    Unit,
    Pair(Int, Int),
}

pub type Pair = (Int, Int)
";
    let actual = read_normalized_file(&interface_path, "generated qi artifact");
    expect_snapshot_matches(
        "project-interface-success",
        "generated qi artifact",
        expected,
        &actual,
    )
    .expect("generated qi artifact should match the public interface snapshot");
}

#[test]
fn project_emit_interface_reports_all_failing_package_sources() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-interface-package-source-failures");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create project source directory for package source failure test");
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace/app/src/lib.ql",
        r#"
package demo.api

pub fn exported() -> Int {
    return 1
}
"#,
    );
    let first_failure = temp.write(
        "workspace/app/src/a_broken.ql",
        r#"
package demo.api

pub fn broken_first(value: MissingFirst) -> Int {
    return value
}
"#,
    );
    temp.write(
        "workspace/app/src/z_broken.ql",
        r#"
package demo.api

pub fn broken_second(value: MissingSecond) -> Int {
    return value
}
"#,
    );
    let interface_path = project_root.join("app.qi");
    let manifest_path = project_root.join("qlang.toml");
    let first_failure_display = first_failure.to_string_lossy().replace('\\', "/");
    let manifest_display = manifest_path.to_string_lossy().replace('\\', "/");

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "emit-interface"])
        .arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project emit-interface` package with multiple failing sources",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-interface-package-source-failures",
        "package interface emission with multiple failing sources",
        &output,
        1,
    )
    .expect("package interface emission with multiple failing sources should fail");
    expect_empty_stdout(
        "project-interface-package-source-failures",
        "package interface emission with multiple failing sources",
        &stdout,
    )
    .expect("failing package interface emission should not report a written artifact");
    expect_stderr_contains(
        "project-interface-package-source-failures",
        "package interface emission with multiple failing sources",
        &stderr,
        "a_broken.ql",
    )
    .expect("package interface emission should report the first failing source file");
    expect_stderr_contains(
        "project-interface-package-source-failures",
        "package interface emission with multiple failing sources",
        &stderr,
        "z_broken.ql",
    )
    .expect("package interface emission should continue reporting later failing source files");
    expect_stderr_contains(
        "project-interface-package-source-failures",
        "package interface emission with multiple failing sources",
        &stderr,
        "interface emission found 2 failing source file(s)",
    )
    .expect("package interface emission should summarize all failing source files");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stderr_contains(
        "project-interface-package-source-failures",
        "package interface emission with multiple failing sources",
        &normalized_stderr,
        &format!("note: first failing source file: {first_failure_display}"),
    )
    .expect("package interface emission should point to the first failing source file");
    expect_stderr_contains(
        "project-interface-package-source-failures",
        "package interface emission with multiple failing sources",
        &normalized_stderr,
        &format!("note: failing package manifest: {manifest_display}"),
    )
    .expect("package interface emission should point to the failing package manifest");
    expect_stderr_contains(
        "project-interface-package-source-failures",
        "package interface emission with multiple failing sources",
        &normalized_stderr,
        &format!(
            "hint: rerun `ql project emit-interface {}` after fixing the package interface error",
            manifest_display
        ),
    )
    .expect("package interface emission should suggest rerunning package interface emission");
    assert!(
        !interface_path.is_file(),
        "failing package interface emission should not create `{}`",
        interface_path.display()
    );
}

#[test]
fn project_emit_interface_dedupes_single_failing_package_source_summary() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-interface-single-package-source-failure");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create project source directory for single package source failure test");
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace/app/src/lib.ql",
        r#"
package demo.api

pub fn exported() -> Int {
    return 1
}
"#,
    );
    let broken_source = temp.write(
        "workspace/app/src/broken.ql",
        r#"
package demo.api

pub fn broken(value: MissingType) -> Int {
    return value
}
"#,
    );
    let interface_path = project_root.join("app.qi");
    let manifest_path = project_root.join("qlang.toml");
    let normalized_stderr_path = broken_source.to_string_lossy().replace('\\', "/");
    let manifest_display = manifest_path.to_string_lossy().replace('\\', "/");

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "emit-interface"])
        .arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project emit-interface` package with single failing source",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-interface-single-package-source-failure",
        "package interface emission with single failing source",
        &output,
        1,
    )
    .expect("package interface emission with a single failing source should fail");
    expect_empty_stdout(
        "project-interface-single-package-source-failure",
        "package interface emission with single failing source",
        &stdout,
    )
    .expect("single failing package interface emission should not report a written artifact");
    expect_stderr_contains(
        "project-interface-single-package-source-failure",
        "package interface emission with single failing source",
        &stderr,
        "broken.ql",
    )
    .expect("package interface emission should surface the broken source file");
    expect_stderr_contains(
        "project-interface-single-package-source-failure",
        "package interface emission with single failing source",
        &stderr,
        "interface emission found 1 failing source file(s)",
    )
    .expect("package interface emission should summarize the single failing source");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stderr_not_contains(
        "project-interface-single-package-source-failure",
        "package interface emission with single failing source",
        &normalized_stderr,
        "note: first failing source file:",
    )
    .expect(
        "single failing package sources should not repeat the source path in the final summary",
    );
    expect_stderr_contains(
        "project-interface-single-package-source-failure",
        "package interface emission with single failing source",
        &normalized_stderr,
        &format!("note: failing package manifest: {manifest_display}"),
    )
    .expect("package interface emission should still point to the failing package manifest");
    expect_stderr_contains(
        "project-interface-single-package-source-failure",
        "package interface emission with single failing source",
        &normalized_stderr,
        &format!(
            "hint: rerun `ql project emit-interface {}` after fixing the package interface error",
            manifest_display
        ),
    )
    .expect("package interface emission should still suggest rerunning package interface emission");
    expect_stderr_contains(
        "project-interface-single-package-source-failure",
        "package interface emission with single failing source",
        &normalized_stderr,
        &normalized_stderr_path,
    )
    .expect("package interface emission should still surface the broken source path locally");
    assert!(
        !interface_path.is_file(),
        "single failing package interface emission should not create `{}`",
        interface_path.display()
    );
}

#[test]
fn project_emit_interface_points_to_invalid_package_manifest() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-interface-invalid-package-manifest");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(&project_root)
        .expect("create project root for invalid package manifest emit-interface test");
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
"#,
    );
    let manifest_path = project_root.join("qlang.toml");
    let interface_path = project_root.join("app.qi");
    let manifest_display = manifest_path.to_string_lossy().replace('\\', "/");

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "emit-interface"])
        .arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project emit-interface` invalid package manifest",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-interface-invalid-package-manifest",
        "package interface emission with invalid package manifest",
        &output,
        1,
    )
    .expect("package interface emission should fail when the manifest does not declare `[package].name`");
    expect_empty_stdout(
        "project-interface-invalid-package-manifest",
        "package interface emission with invalid package manifest",
        &stdout,
    )
    .expect("invalid package manifest should not report a written interface");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stderr_contains(
        "project-interface-invalid-package-manifest",
        "package interface emission with invalid package manifest",
        &normalized_stderr,
        &format!(
            "error: `ql project emit-interface` manifest `{}` does not declare `[package].name`",
            manifest_display
        ),
    )
    .expect("invalid package manifest should preserve the emit-interface command label");
    expect_stderr_contains(
        "project-interface-invalid-package-manifest",
        "package interface emission with invalid package manifest",
        &normalized_stderr,
        &format!("note: failing package manifest: {manifest_display}"),
    )
    .expect("invalid package manifest should point to the failing package manifest");
    expect_stderr_contains(
        "project-interface-invalid-package-manifest",
        "package interface emission with invalid package manifest",
        &normalized_stderr,
        &format!(
            "hint: rerun `ql project emit-interface {}` after fixing the package manifest",
            manifest_display
        ),
    )
    .expect("invalid package manifest should suggest fixing the package manifest directly");
    expect_stderr_not_contains(
        "project-interface-invalid-package-manifest",
        "package interface emission with invalid package manifest",
        &normalized_stderr,
        "after fixing the package interface error",
    )
    .expect("invalid package manifest should not fall back to the generic interface-error hint");
    assert!(
        !interface_path.exists(),
        "invalid package manifest should not create `{}`",
        interface_path.display()
    );
}

#[test]
fn project_emit_interface_points_to_missing_package_source_root() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-interface-missing-package-source-root");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(&project_root)
        .expect("create project root for missing package source root emit-interface test");
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    let manifest_path = project_root.join("qlang.toml");
    let source_root = project_root.join("src");
    let interface_path = project_root.join("app.qi");
    let manifest_display = manifest_path.to_string_lossy().replace('\\', "/");
    let source_root_display = source_root.to_string_lossy().replace('\\', "/");

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "emit-interface"])
        .arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project emit-interface` missing package source root",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-interface-missing-package-source-root",
        "package interface emission with missing package source root",
        &output,
        1,
    )
    .expect("package interface emission should fail when the package source root is missing");
    expect_empty_stdout(
        "project-interface-missing-package-source-root",
        "package interface emission with missing package source root",
        &stdout,
    )
    .expect("missing package source root should not report a written interface");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stderr_contains(
        "project-interface-missing-package-source-root",
        "package interface emission with missing package source root",
        &normalized_stderr,
        &format!(
            "error: `ql project emit-interface` package source directory `{}` does not exist",
            source_root_display
        ),
    )
    .expect("missing package source root should preserve the emit-interface command label");
    expect_stderr_contains(
        "project-interface-missing-package-source-root",
        "package interface emission with missing package source root",
        &normalized_stderr,
        &format!("note: failing package manifest: {manifest_display}"),
    )
    .expect("missing package source root should point to the failing package manifest");
    expect_stderr_contains(
        "project-interface-missing-package-source-root",
        "package interface emission with missing package source root",
        &normalized_stderr,
        &format!("note: failing package source root: {source_root_display}"),
    )
    .expect("missing package source root should point to the missing source root");
    expect_stderr_contains(
        "project-interface-missing-package-source-root",
        "package interface emission with missing package source root",
        &normalized_stderr,
        &format!(
            "hint: rerun `ql project emit-interface {}` after fixing the package source root",
            manifest_display
        ),
    )
    .expect("missing package source root should suggest fixing the package source root directly");
    expect_stderr_not_contains(
        "project-interface-missing-package-source-root",
        "package interface emission with missing package source root",
        &normalized_stderr,
        "after fixing the package interface error",
    )
    .expect("missing package source root should not fall back to the generic interface-error hint");
    assert!(
        !interface_path.exists(),
        "missing package source root should not create `{}`",
        interface_path.display()
    );
}

#[test]
fn project_emit_interface_points_to_empty_package_source_root() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-interface-empty-package-source-root");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create empty package source root for emit-interface test");
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    let interface_path = project_root.join("app.qi");
    let manifest_path = project_root.join("qlang.toml");
    let source_root = project_root.join("src");
    let manifest_display = manifest_path.to_string_lossy().replace('\\', "/");
    let source_root_display = source_root.to_string_lossy().replace('\\', "/");

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "emit-interface"])
        .arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project emit-interface` package with empty source root",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-interface-empty-source-root",
        "package interface emission with empty source root",
        &output,
        1,
    )
    .expect(
        "package interface emission should fail when the package source root has no `.ql` files",
    );
    expect_empty_stdout(
        "project-interface-empty-source-root",
        "package interface emission with empty source root",
        &stdout,
    )
    .expect("empty package source root should not report a written interface");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stderr_contains(
        "project-interface-empty-source-root",
        "package interface emission with empty source root",
        &normalized_stderr,
        &format!(
            "error: `ql project emit-interface` no `.ql` files found under `{}`",
            source_root_display
        ),
    )
    .expect("empty package source root should be reported as a no-source failure");
    expect_stderr_contains(
        "project-interface-empty-source-root",
        "package interface emission with empty source root",
        &normalized_stderr,
        &format!("note: failing package manifest: {manifest_display}"),
    )
    .expect("empty package source root should point to the failing package manifest");
    expect_stderr_contains(
        "project-interface-empty-source-root",
        "package interface emission with empty source root",
        &normalized_stderr,
        &format!("note: failing package source root: {source_root_display}"),
    )
    .expect("empty package source root should point to the empty source root");
    expect_stderr_contains(
        "project-interface-empty-source-root",
        "package interface emission with empty source root",
        &normalized_stderr,
        &format!(
            "hint: rerun `ql project emit-interface {}` after adding package source files",
            manifest_display
        ),
    )
    .expect("empty package source root should suggest adding package source files");
    expect_stderr_not_contains(
        "project-interface-empty-source-root",
        "package interface emission with empty source root",
        &normalized_stderr,
        "after fixing the package interface error",
    )
    .expect("empty package source root should not fall back to the generic interface-error hint");
    assert!(
        !interface_path.exists(),
        "empty package source root should not create `{}`",
        interface_path.display()
    );
}

#[test]
fn project_emit_interface_points_blocked_output_paths_at_interface_target() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-interface-output-path-failure");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create project source directory for blocked output path test");
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace/app/src/lib.ql",
        r#"
package demo.app

pub fn exported() -> Int {
    return 1
}
"#,
    );
    let interface_path = project_root.join("app.qi");
    std::fs::create_dir_all(&interface_path)
        .expect("create blocking interface directory for emit-interface test");
    let manifest_path = project_root.join("qlang.toml");
    let manifest_display = manifest_path.to_string_lossy().replace('\\', "/");
    let interface_display = interface_path.to_string_lossy().replace('\\', "/");

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "emit-interface"])
        .arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project emit-interface` package with blocked interface output path",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-interface-output-path-failure",
        "package interface emission with blocked output path",
        &output,
        1,
    )
    .expect("package interface emission should fail when the interface output path is blocked");
    expect_empty_stdout(
        "project-interface-output-path-failure",
        "package interface emission with blocked output path",
        &stdout,
    )
    .expect("blocked output path should not report a written interface");
    expect_stderr_contains(
        "project-interface-output-path-failure",
        "package interface emission with blocked output path",
        &stderr,
        "failed to write interface",
    )
    .expect("blocked output path should surface a write failure");
    let normalized_stderr = stderr.replace('\\', "/");
    let package_note = format!("note: failing package manifest: {manifest_display}");
    let output_note = format!("note: failing interface output path: {interface_display}");
    let rerun_hint = format!(
        "hint: rerun `ql project emit-interface {}` after fixing the interface output path",
        manifest_display
    );
    let old_hint = format!(
        "hint: rerun `ql project emit-interface {}` after fixing the package interface error",
        manifest_display
    );
    expect_stderr_contains(
        "project-interface-output-path-failure",
        "package interface emission with blocked output path",
        &normalized_stderr,
        &package_note,
    )
    .expect("blocked output path should still point to the package manifest");
    expect_stderr_contains(
        "project-interface-output-path-failure",
        "package interface emission with blocked output path",
        &normalized_stderr,
        &output_note,
    )
    .expect("blocked output path should point to the failing interface target");
    expect_stderr_contains(
        "project-interface-output-path-failure",
        "package interface emission with blocked output path",
        &normalized_stderr,
        &rerun_hint,
    )
    .expect("blocked output path should suggest fixing the interface output path");
    expect_stderr_not_contains(
        "project-interface-output-path-failure",
        "package interface emission with blocked output path",
        &normalized_stderr,
        &old_hint,
    )
    .expect("blocked output path should not pretend that package source diagnostics are the issue");
    let package_note_index = normalized_stderr
        .find(&package_note)
        .expect("blocked output path should include the package manifest note");
    let output_note_index = normalized_stderr
        .find(&output_note)
        .expect("blocked output path should include the output-path note");
    let rerun_hint_index = normalized_stderr
        .find(&rerun_hint)
        .expect("blocked output path should include the rerun hint");
    assert!(
        package_note_index < output_note_index && output_note_index < rerun_hint_index,
        "expected blocked output path context before rerun hint, got:\n{stderr}"
    );
    assert!(
        interface_path.is_dir(),
        "blocked output path test should preserve `{}` as a directory",
        interface_path.display()
    );
}

#[test]
fn project_emit_interface_preserves_custom_output_in_source_failure_hints() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-interface-custom-output-source-failure");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create project source directory for custom output source failure test");
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace/app/src/lib.ql",
        r#"
package demo.app

pub fn exported() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace/app/src/broken.ql",
        r#"
package demo.app

pub fn broken(value: MissingType) -> Int {
    return value
}
"#,
    );
    let output_path = project_root.join("artifacts").join("custom.qi");
    let manifest_path = project_root.join("qlang.toml");
    let manifest_display = manifest_path.to_string_lossy().replace('\\', "/");
    let output_display = output_path.to_string_lossy().replace('\\', "/");

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "emit-interface", "--output"])
        .arg(&output_path)
        .arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project emit-interface --output` package with source failure",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-interface-custom-output-source-failure",
        "package interface emission with custom output and source failure",
        &output,
        1,
    )
    .expect("custom-output package interface emission should still fail on source errors");
    expect_empty_stdout(
        "project-interface-custom-output-source-failure",
        "package interface emission with custom output and source failure",
        &stdout,
    )
    .expect("source failures should not report a written custom output artifact");
    let normalized_stderr = stderr.replace('\\', "/");
    let rerun_hint = format!(
        "hint: rerun `ql project emit-interface {} --output {}` after fixing the package interface error",
        manifest_display, output_display
    );
    let old_hint = format!(
        "hint: rerun `ql project emit-interface {}` after fixing the package interface error",
        manifest_display
    );
    expect_stderr_contains(
        "project-interface-custom-output-source-failure",
        "package interface emission with custom output and source failure",
        &normalized_stderr,
        &rerun_hint,
    )
    .expect("custom-output source failures should preserve `--output` in the rerun hint");
    expect_stderr_not_contains(
        "project-interface-custom-output-source-failure",
        "package interface emission with custom output and source failure",
        &normalized_stderr,
        &old_hint,
    )
    .expect("custom-output source failures should not drop back to the default rerun hint");
    assert!(
        !output_path.is_file(),
        "custom-output source failure should not create `{}`",
        output_path.display()
    );
}

#[test]
fn project_emit_interface_preserves_custom_output_in_output_path_hints() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-interface-custom-output-path-failure");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create project source directory for custom output path failure test");
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace/app/src/lib.ql",
        r#"
package demo.app

pub fn exported() -> Int {
    return 1
}
"#,
    );
    let output_path = project_root.join("artifacts").join("custom.qi");
    std::fs::create_dir_all(&output_path)
        .expect("create blocking directory at the custom interface output path");
    let manifest_path = project_root.join("qlang.toml");
    let manifest_display = manifest_path.to_string_lossy().replace('\\', "/");
    let output_display = output_path.to_string_lossy().replace('\\', "/");

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "emit-interface", "--output"])
        .arg(&output_path)
        .arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project emit-interface --output` package with blocked custom output path",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-interface-custom-output-path-failure",
        "package interface emission with blocked custom output path",
        &output,
        1,
    )
    .expect(
        "custom-output package interface emission should fail when the custom target is blocked",
    );
    expect_empty_stdout(
        "project-interface-custom-output-path-failure",
        "package interface emission with blocked custom output path",
        &stdout,
    )
    .expect("blocked custom output path should not report a written artifact");
    let normalized_stderr = stderr.replace('\\', "/");
    let rerun_hint = format!(
        "hint: rerun `ql project emit-interface {} --output {}` after fixing the interface output path",
        manifest_display, output_display
    );
    let old_hint = format!(
        "hint: rerun `ql project emit-interface {}` after fixing the interface output path",
        manifest_display
    );
    expect_stderr_contains(
        "project-interface-custom-output-path-failure",
        "package interface emission with blocked custom output path",
        &normalized_stderr,
        &rerun_hint,
    )
    .expect("custom-output path failures should preserve `--output` in the rerun hint");
    expect_stderr_not_contains(
        "project-interface-custom-output-path-failure",
        "package interface emission with blocked custom output path",
        &normalized_stderr,
        &old_hint,
    )
    .expect("custom-output path failures should not drop back to the default rerun hint");
    assert!(
        output_path.is_dir(),
        "blocked custom output path test should preserve `{}` as a directory",
        output_path.display()
    );
}

#[test]
fn project_emit_interface_changed_only_skips_up_to_date_package_interface() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-interface-changed-only-package");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create project source directory for changed-only package test");
    let interface_path = project_root.join("app.qi");
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace/app/src/lib.ql",
        r#"
package demo.app

pub fn exported() -> Int {
    return 1
}
"#,
    );
    let expected = "\
// qlang interface v1
// package: app

// source: src/lib.ql
package demo.app

pub fn exported() -> Int
";
    temp.write("workspace/app/app.qi", expected);

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "emit-interface", "--changed-only"])
        .arg(&project_root);
    let output = run_command_capture(&mut command, "`ql project emit-interface --changed-only`");
    let (stdout, stderr) = expect_success(
        "project-interface-changed-only-package",
        "changed-only package interface emission",
        &output,
    )
    .expect("changed-only package interface emission should succeed");
    expect_snapshot_matches(
        "project-interface-changed-only-package",
        "changed-only package interface stdout",
        &format!("up-to-date interface: {}\n", interface_path.display()),
        &stdout,
    )
    .expect("changed-only package interface emission should skip up-to-date artifact");
    expect_snapshot_matches(
        "project-interface-changed-only-package",
        "changed-only package interface stderr",
        "",
        &stderr,
    )
    .expect("changed-only package interface emission should stay silent on stderr");
    let actual = read_normalized_file(&interface_path, "changed-only generated qi artifact");
    expect_snapshot_matches(
        "project-interface-changed-only-package",
        "changed-only package qi artifact",
        expected,
        &actual,
    )
    .expect("changed-only package interface emission should leave up-to-date artifact unchanged");
}

#[test]
fn project_emit_interface_changed_only_preserves_source_failure_hints() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-interface-changed-only-package-source-failure");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create project source directory for changed-only source failure test");
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace/app/src/lib.ql",
        r#"
package demo.app

pub fn exported() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace/app/src/broken.ql",
        r#"
package demo.app

pub fn broken(value: MissingType) -> Int {
    return value
}
"#,
    );
    let manifest_path = project_root.join("qlang.toml");
    let manifest_display = manifest_path.to_string_lossy().replace('\\', "/");

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "emit-interface", "--changed-only"])
        .arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project emit-interface --changed-only` package with source failure",
    );
    let (_stdout, stderr) = expect_exit_code(
        "project-interface-changed-only-package-source-failure",
        "changed-only package interface emission with source failure",
        &output,
        1,
    )
    .expect("changed-only package interface emission with source failure should fail");
    let normalized_stderr = stderr.replace('\\', "/");
    let package_note = format!("note: failing package manifest: {manifest_display}");
    let rerun_hint = format!(
        "hint: rerun `ql project emit-interface {} --changed-only` after fixing the package interface error",
        manifest_display
    );
    let default_rerun_hint = format!(
        "hint: rerun `ql project emit-interface {}` after fixing the package interface error",
        manifest_display
    );
    expect_stderr_contains(
        "project-interface-changed-only-package-source-failure",
        "changed-only package interface emission with source failure",
        &normalized_stderr,
        &rerun_hint,
    )
    .expect("changed-only package source failures should preserve `--changed-only`");
    assert!(
        !normalized_stderr.contains(&default_rerun_hint),
        "changed-only package source failures should not fall back to the default rerun hint, got:\n{stderr}"
    );
    let package_note_index = normalized_stderr
        .find(&package_note)
        .expect("changed-only package source failure should include the package manifest note");
    let rerun_hint_index = normalized_stderr
        .find(&rerun_hint)
        .expect("changed-only package source failure should include the rerun hint");
    assert!(
        package_note_index < rerun_hint_index,
        "expected changed-only package source failure context before rerun hint, got:\n{stderr}"
    );
}

#[test]
fn project_emit_interface_changed_only_preserves_workspace_non_package_member_command_label() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-interface-changed-only-workspace-not-package");
    let project_root = temp.path().join("workspace-only");
    let app_root = project_root.join("packages").join("app");
    let broken_root = project_root.join("packages").join("broken");
    let tool_root = project_root.join("packages").join("tool");
    std::fs::create_dir_all(app_root.join("src"))
        .expect("create app package source directory for changed-only workspace semantic-invalid member test");
    std::fs::create_dir_all(&broken_root).expect(
        "create broken package directory for changed-only workspace semantic-invalid member test",
    );
    std::fs::create_dir_all(tool_root.join("src"))
        .expect("create tool package source directory for changed-only workspace semantic-invalid member test");
    temp.write(
        "workspace-only/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/broken", "packages/tool"]
"#,
    );
    temp.write(
        "workspace-only/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace-only/packages/app/src/lib.ql",
        r#"
package demo.app

pub fn exported() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace-only/packages/app/app.qi",
        "\
// qlang interface v1
// package: app

// source: src/lib.ql
package demo.app

pub fn exported() -> Int
",
    );
    temp.write(
        "workspace-only/packages/broken/qlang.toml",
        r#"
[workspace]
members = []
"#,
    );
    temp.write(
        "workspace-only/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );
    temp.write(
        "workspace-only/packages/tool/src/lib.ql",
        r#"
package demo.tool

pub fn exported() -> Int {
    return 2
}
"#,
    );
    temp.write(
        "workspace-only/packages/tool/tool.qi",
        "\
// qlang interface v1
// package: tool

// source: src/lib.ql
package demo.tool

pub fn exported() -> Int
",
    );

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "emit-interface", "--changed-only"])
        .arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project emit-interface --changed-only` workspace manifest with non-package member",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-interface-changed-only-workspace-not-package",
        "changed-only workspace interface emission with non-package member",
        &output,
        1,
    )
    .expect("changed-only workspace interface emission with non-package member should fail");
    let normalized_stdout = stdout.replace('\\', "/");
    let normalized_stderr = stderr.replace('\\', "/");
    let app_interface = app_root
        .join("app.qi")
        .display()
        .to_string()
        .replace('\\', "/");
    let tool_interface = tool_root
        .join("tool.qi")
        .display()
        .to_string()
        .replace('\\', "/");
    expect_stdout_contains_all(
        "project-interface-changed-only-workspace-not-package",
        &normalized_stdout,
        &[
            &format!("up-to-date interface: {app_interface}"),
            &format!("up-to-date interface: {tool_interface}"),
        ],
    )
    .expect(
        "changed-only workspace interface emission should continue reporting later valid members",
    );
    let broken_manifest = broken_root
        .join("qlang.toml")
        .display()
        .to_string()
        .replace('\\', "/");
    let error_line = format!(
        "error: `ql project emit-interface --changed-only` manifest `{broken_manifest}` does not declare `[package].name`"
    );
    let old_error_line = format!(
        "error: `ql project emit-interface` manifest `{broken_manifest}` does not declare `[package].name`"
    );
    let package_note = format!("note: failing package manifest: {broken_manifest}");
    let member_note = format!("note: failing workspace member manifest: {broken_manifest}");
    let rerun_hint = format!(
        "hint: rerun `ql project emit-interface {broken_manifest} --changed-only` after fixing the package interface error"
    );
    expect_stderr_contains(
        "project-interface-changed-only-workspace-not-package",
        "changed-only workspace interface emission with non-package member",
        &normalized_stderr,
        &error_line,
    )
    .expect("changed-only workspace semantic-invalid member errors should preserve the full command label");
    expect_stderr_not_contains(
        "project-interface-changed-only-workspace-not-package",
        "changed-only workspace interface emission with non-package member",
        &normalized_stderr,
        &old_error_line,
    )
    .expect("changed-only workspace semantic-invalid member errors should not fall back to the default command label");
    let error_line_index = normalized_stderr
        .find(&error_line)
        .expect("changed-only workspace semantic-invalid member errors should include the full command label");
    let package_note_index = normalized_stderr.find(&package_note).expect(
        "changed-only workspace semantic-invalid member errors should include the failing package note",
    );
    let member_note_index = normalized_stderr.find(&member_note).expect(
        "changed-only workspace semantic-invalid member errors should include the local member note",
    );
    let rerun_hint_index = normalized_stderr.find(&rerun_hint).expect(
        "changed-only workspace semantic-invalid member errors should include the rerun hint",
    );
    assert!(
        error_line_index < package_note_index
            && package_note_index < member_note_index
            && member_note_index < rerun_hint_index,
        "expected changed-only workspace semantic-invalid member context before rerun hint, got:\n{stderr}"
    );
    expect_stderr_contains(
        "project-interface-changed-only-workspace-not-package",
        "changed-only workspace interface emission with non-package member",
        &stderr,
        "interface emission found 1 failing member(s)",
    )
    .expect("changed-only workspace semantic-invalid member failures should still summarize failing members");
    expect_stderr_not_contains(
        "project-interface-changed-only-workspace-not-package",
        "changed-only workspace interface emission with non-package member",
        &normalized_stderr,
        "note: first failing member manifest:",
    )
    .expect("single changed-only workspace semantic-invalid member failures should not repeat the manifest in the final summary");
}

#[test]
fn project_emit_interface_check_accepts_valid_package_interface() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-interface-check-package");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create project source directory for interface check test");
    let interface_path = project_root.join("app.qi");
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace/app/src/lib.ql",
        r#"
package demo.app

pub fn exported() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace/app/app.qi",
        "\
// qlang interface v1
// package: app

// source: src/lib.ql
package demo.app

pub fn exported() -> Int
",
    );

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "emit-interface", "--check"])
        .arg(&project_root);
    let output = run_command_capture(&mut command, "`ql project emit-interface --check` package");
    let (stdout, stderr) = expect_success(
        "project-interface-check-package",
        "package interface check",
        &output,
    )
    .expect("package interface check should succeed");
    expect_snapshot_matches(
        "project-interface-check-package",
        "package interface check stdout",
        &format!("ok interface: {}\n", interface_path.display()),
        &stdout,
    )
    .expect("package interface check should report a valid interface");
    expect_snapshot_matches(
        "project-interface-check-package",
        "package interface check stderr",
        "",
        &stderr,
    )
    .expect("package interface check should stay silent on stderr");
}

#[test]
fn project_emit_interface_check_changed_only_accepts_valid_package_interface() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-interface-check-changed-only-package");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create project source directory for changed-only interface check test");
    let interface_path = project_root.join("app.qi");
    let expected = "\
// qlang interface v1
// package: app

// source: src/lib.ql
package demo.app

pub fn exported() -> Int
";
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace/app/src/lib.ql",
        r#"
package demo.app

pub fn exported() -> Int {
    return 1
}
"#,
    );
    temp.write("workspace/app/app.qi", expected);
    let metadata_before = std::fs::metadata(&interface_path)
        .expect("read interface metadata before changed-only package check")
        .modified()
        .expect("read interface modification time before changed-only package check");

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "emit-interface", "--changed-only", "--check"])
        .arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project emit-interface --changed-only --check` package",
    );
    let (stdout, stderr) = expect_success(
        "project-interface-check-changed-only-package",
        "changed-only package interface check",
        &output,
    )
    .expect("changed-only package interface check should succeed");
    expect_snapshot_matches(
        "project-interface-check-changed-only-package",
        "changed-only package interface check stdout",
        &format!("up-to-date interface: {}\n", interface_path.display()),
        &stdout,
    )
    .expect("changed-only package interface check should report a valid interface as up to date");
    expect_snapshot_matches(
        "project-interface-check-changed-only-package",
        "changed-only package interface check stderr",
        "",
        &stderr,
    )
    .expect("changed-only package interface check should stay silent on stderr");
    let actual = read_normalized_file(
        &interface_path,
        "changed-only package check interface artifact after check",
    );
    expect_snapshot_matches(
        "project-interface-check-changed-only-package",
        "changed-only package check qi artifact",
        expected,
        &actual,
    )
    .expect("changed-only package interface check should not rewrite a valid artifact");
    let metadata_after = std::fs::metadata(&interface_path)
        .expect("read interface metadata after changed-only package check")
        .modified()
        .expect("read interface modification time after changed-only package check");
    assert_eq!(
        metadata_before,
        metadata_after,
        "expected changed-only package interface check not to rewrite `{}`",
        interface_path.display()
    );
}

#[test]
fn project_emit_interface_check_rejects_invalid_package_interface() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-interface-check-invalid-package");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create project source directory for invalid interface check test");
    let manifest_path = temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace/app/src/lib.ql",
        r#"
package demo.app

pub fn exported() -> Int {
    return 1
}
"#,
    );
    temp.write("workspace/app/app.qi", "broken interface\n");

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "emit-interface", "--check"])
        .arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project emit-interface --check` invalid package",
    );
    let (_stdout, stderr) = expect_exit_code(
        "project-interface-check-invalid-package",
        "invalid package interface check",
        &output,
        1,
    )
    .expect("invalid package interface check should fail");
    expect_stderr_contains(
        "project-interface-check-invalid-package",
        "invalid package interface check",
        &stderr,
        "is invalid",
    )
    .expect("invalid package interface check should report invalid status");
    expect_stderr_contains(
        "project-interface-check-invalid-package",
        "invalid package interface check",
        &stderr,
        "detail: expected `// qlang interface v1` header",
    )
    .expect("invalid package interface check should report parse detail");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stderr_contains(
        "project-interface-check-invalid-package",
        "invalid package interface check",
        &normalized_stderr,
        &format!(
            "note: failing package manifest: {}",
            manifest_path.display().to_string().replace('\\', "/")
        ),
    )
    .expect("invalid package interface check should point to the failing package manifest");
    let error_line = format!(
        "error: interface artifact `{}` is invalid",
        project_root
            .join("app.qi")
            .display()
            .to_string()
            .replace('\\', "/")
    );
    let detail_line = "detail: expected `// qlang interface v1` header";
    let package_note = format!(
        "note: failing package manifest: {}",
        manifest_path.display().to_string().replace('\\', "/")
    );
    let rerun_hint = format!(
        "hint: rerun `ql project emit-interface {}` to regenerate it",
        manifest_path.display().to_string().replace('\\', "/")
    );
    let error_index = normalized_stderr
        .find(&error_line)
        .expect("invalid package interface check should report the error line");
    let detail_index = normalized_stderr
        .find(detail_line)
        .expect("invalid package interface check should report parse detail");
    let package_note_index = normalized_stderr
        .find(&package_note)
        .expect("invalid package interface check should point to the package manifest");
    let rerun_hint_index = normalized_stderr
        .find(&rerun_hint)
        .expect("invalid package interface check should include the rerun hint");
    assert!(
        error_index < detail_index
            && detail_index < package_note_index
            && package_note_index < rerun_hint_index,
        "expected invalid package interface check to keep detail before manifest and hint, got:\n{stderr}"
    );
}

#[test]
fn project_emit_interface_writes_member_qi_for_workspace_only_manifest() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-interface-workspace-only");
    let project_root = temp.path().join("workspace-only");
    let app_root = project_root.join("packages").join("app");
    let tool_root = project_root.join("packages").join("tool");
    std::fs::create_dir_all(app_root.join("src")).expect("create app package source directory");
    std::fs::create_dir_all(tool_root.join("src")).expect("create tool package source directory");
    temp.write(
        "workspace-only/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/tool"]
"#,
    );
    temp.write(
        "workspace-only/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace-only/packages/app/src/lib.ql",
        r#"
package demo.app

pub fn exported() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace-only/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );
    temp.write(
        "workspace-only/packages/tool/src/lib.ql",
        r#"
package demo.tool

pub struct Config {
    value: Int,
}
"#,
    );
    let app_interface = app_root.join("app.qi");
    let tool_interface = tool_root.join("tool.qi");

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "emit-interface"])
        .arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project emit-interface` workspace-only manifest",
    );
    let (stdout, stderr) = expect_success(
        "project-interface-workspace-only",
        "workspace-only interface emission",
        &output,
    )
    .expect("workspace-only interface emission should succeed");
    let normalized_stdout = stdout.replace('\\', "/");
    let normalized_app_interface = app_interface.display().to_string().replace('\\', "/");
    let normalized_tool_interface = tool_interface.display().to_string().replace('\\', "/");
    expect_stdout_contains_all(
        "project-interface-workspace-only",
        &normalized_stdout,
        &[
            &format!("wrote interface: {normalized_app_interface}"),
            &format!("wrote interface: {normalized_tool_interface}"),
        ],
    )
    .expect("workspace-only interface emission should report each written artifact");
    expect_snapshot_matches(
        "project-interface-workspace-only",
        "workspace-only interface emission stderr",
        &stderr,
        "",
    )
    .expect("workspace-only interface emission should stay silent on stderr");
    expect_file_exists(
        "project-interface-workspace-only",
        &app_interface,
        "workspace app qi",
        "workspace-only interface emission",
    )
    .expect("workspace-only interface emission should create app qi");
    expect_file_exists(
        "project-interface-workspace-only",
        &tool_interface,
        "workspace tool qi",
        "workspace-only interface emission",
    )
    .expect("workspace-only interface emission should create tool qi");
}

#[test]
fn project_emit_interface_keeps_writing_other_workspace_members_when_one_member_fails() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-interface-workspace-partial-failure");
    let project_root = temp.path().join("workspace-only");
    let app_root = project_root.join("packages").join("app");
    let broken_root = project_root.join("packages").join("broken");
    std::fs::create_dir_all(app_root.join("src"))
        .expect("create app package source directory for partial workspace emit test");
    std::fs::create_dir_all(&broken_root)
        .expect("create broken package directory for partial workspace emit test");
    temp.write(
        "workspace-only/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/broken"]
"#,
    );
    temp.write(
        "workspace-only/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace-only/packages/app/src/lib.ql",
        r#"
package demo.app

pub fn exported() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace-only/packages/broken/qlang.toml",
        r#"
[package
name = "broken"
"#,
    );
    let app_interface = app_root.join("app.qi");
    let broken_interface = broken_root.join("broken.qi");

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "emit-interface"])
        .arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project emit-interface` workspace manifest with failing member",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-interface-workspace-partial-failure",
        "workspace interface emission with failing member",
        &output,
        1,
    )
    .expect("workspace interface emission with failing member should fail");
    let normalized_stdout = stdout.replace('\\', "/");
    let normalized_stderr = stderr.replace('\\', "/");
    let normalized_app_interface = app_interface.display().to_string().replace('\\', "/");
    expect_stdout_contains_all(
        "project-interface-workspace-partial-failure",
        &normalized_stdout,
        &[&format!("wrote interface: {normalized_app_interface}")],
    )
    .expect("workspace interface emission should still write healthy members before failing");
    expect_stderr_contains(
        "project-interface-workspace-partial-failure",
        "workspace interface emission with failing member",
        &stderr,
        "invalid manifest",
    )
    .expect("workspace interface emission should surface the failing member manifest error");
    expect_stderr_contains(
        "project-interface-workspace-partial-failure",
        "workspace interface emission with failing member",
        &normalized_stderr,
        &format!(
            "note: failing workspace member manifest: {}",
            broken_root
                .join("qlang.toml")
                .display()
                .to_string()
                .replace('\\', "/")
        ),
    )
    .expect("workspace interface emission should point invalid member manifests locally");
    let rerun_hint = format!(
        "hint: rerun `ql project emit-interface {}` after fixing the workspace member manifest",
        broken_root
            .join("qlang.toml")
            .display()
            .to_string()
            .replace('\\', "/")
    );
    expect_stderr_contains(
        "project-interface-workspace-partial-failure",
        "workspace interface emission with failing member",
        &normalized_stderr,
        &rerun_hint,
    )
    .expect("workspace interface emission should suggest rerunning the broken member directly after repair");
    expect_stderr_contains(
        "project-interface-workspace-partial-failure",
        "workspace interface emission with failing member",
        &stderr,
        "interface emission found 1 failing member(s)",
    )
    .expect("workspace interface emission should summarize failing members");
    expect_stderr_not_contains(
        "project-interface-workspace-partial-failure",
        "workspace interface emission with failing member",
        &normalized_stderr,
        "note: first failing member manifest:",
    )
    .expect("single failing workspace members should not repeat the manifest in the final summary");
    expect_file_exists(
        "project-interface-workspace-partial-failure",
        &app_interface,
        "workspace app qi",
        "workspace interface emission with failing member",
    )
    .expect("workspace interface emission should still create app qi");
    assert!(
        !broken_interface.is_file(),
        "expected failing workspace member not to create `{}`",
        broken_interface.display()
    );
}

#[test]
fn project_emit_interface_points_workspace_member_source_failures_at_manifest() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-interface-workspace-source-failure");
    let project_root = temp.path().join("workspace-only");
    let app_root = project_root.join("packages").join("app");
    let broken_root = project_root.join("packages").join("broken");
    std::fs::create_dir_all(app_root.join("src"))
        .expect("create app package source directory for workspace source failure test");
    std::fs::create_dir_all(broken_root.join("src"))
        .expect("create broken package source directory for workspace source failure test");
    temp.write(
        "workspace-only/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/broken"]
"#,
    );
    temp.write(
        "workspace-only/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace-only/packages/app/src/lib.ql",
        r#"
package demo.app

pub fn exported() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace-only/packages/broken/qlang.toml",
        r#"
[package]
name = "broken"
"#,
    );
    let first_failure = temp.write(
        "workspace-only/packages/broken/src/a_broken.ql",
        r#"
package demo.broken

pub fn broken_first(value: MissingFirst) -> Int {
    return value
}
"#,
    );
    temp.write(
        "workspace-only/packages/broken/src/z_broken.ql",
        r#"
package demo.broken

pub fn broken_second(value: MissingSecond) -> Int {
    return value
}
"#,
    );
    let app_interface = app_root.join("app.qi");
    let broken_manifest = broken_root.join("qlang.toml");
    let normalized_app_interface = app_interface.display().to_string().replace('\\', "/");
    let normalized_broken_manifest = broken_manifest.display().to_string().replace('\\', "/");
    let normalized_first_failure = first_failure.display().to_string().replace('\\', "/");

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "emit-interface"])
        .arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project emit-interface` workspace member source failure",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-interface-workspace-source-failure",
        "workspace interface emission with member source failure",
        &output,
        1,
    )
    .expect("workspace interface emission with member source failure should fail");
    let normalized_stdout = stdout.replace('\\', "/");
    expect_stdout_contains_all(
        "project-interface-workspace-source-failure",
        &normalized_stdout,
        &[&format!("wrote interface: {normalized_app_interface}")],
    )
    .expect("workspace interface emission should still write healthy members");
    expect_stderr_contains(
        "project-interface-workspace-source-failure",
        "workspace interface emission with member source failure",
        &stderr,
        "a_broken.ql",
    )
    .expect("workspace interface emission should surface the first failing member source");
    expect_stderr_contains(
        "project-interface-workspace-source-failure",
        "workspace interface emission with member source failure",
        &stderr,
        "z_broken.ql",
    )
    .expect("workspace interface emission should continue surfacing later member source failures");
    expect_stderr_contains(
        "project-interface-workspace-source-failure",
        "workspace interface emission with member source failure",
        &stderr,
        "interface emission found 2 failing source file(s)",
    )
    .expect("workspace interface emission should preserve member source aggregation");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stderr_contains(
        "project-interface-workspace-source-failure",
        "workspace interface emission with member source failure",
        &normalized_stderr,
        &format!("note: first failing source file: {normalized_first_failure}"),
    )
    .expect("workspace interface emission should point to the first failing member source file");
    expect_stderr_contains(
        "project-interface-workspace-source-failure",
        "workspace interface emission with member source failure",
        &normalized_stderr,
        &format!("note: failing package manifest: {normalized_broken_manifest}"),
    )
    .expect(
        "workspace interface emission should point member source failures at the member manifest",
    );
    expect_stderr_contains(
        "project-interface-workspace-source-failure",
        "workspace interface emission with member source failure",
        &normalized_stderr,
        &format!("note: failing workspace member manifest: {normalized_broken_manifest}"),
    )
    .expect("workspace interface emission should also keep the workspace member boundary visible");
    expect_stderr_contains(
        "project-interface-workspace-source-failure",
        "workspace interface emission with member source failure",
        &normalized_stderr,
        &format!(
            "hint: rerun `ql project emit-interface {}` after fixing the package interface error",
            normalized_broken_manifest
        ),
    )
    .expect("workspace interface emission should suggest rerunning the failing member manifest directly");
    let package_note = format!("note: failing package manifest: {normalized_broken_manifest}");
    let member_note =
        format!("note: failing workspace member manifest: {normalized_broken_manifest}");
    let rerun_hint = format!(
        "hint: rerun `ql project emit-interface {}` after fixing the package interface error",
        normalized_broken_manifest
    );
    let package_note_index = normalized_stderr
        .find(&package_note)
        .expect("workspace source failure should include the package manifest note");
    let member_note_index = normalized_stderr
        .find(&member_note)
        .expect("workspace source failure should include the workspace member note");
    let rerun_hint_index = normalized_stderr
        .find(&rerun_hint)
        .expect("workspace source failure should include the rerun hint");
    assert!(
        package_note_index < member_note_index && member_note_index < rerun_hint_index,
        "expected workspace source failure context before hint, got:\n{stderr}"
    );
    expect_stderr_contains(
        "project-interface-workspace-source-failure",
        "workspace interface emission with member source failure",
        &stderr,
        "interface emission found 1 failing member(s)",
    )
    .expect("workspace interface emission should still summarize failing members");
    expect_stderr_not_contains(
        "project-interface-workspace-source-failure",
        "workspace interface emission with member source failure",
        &normalized_stderr,
        "note: first failing member manifest:",
    )
    .expect("single failing workspace members should not repeat the manifest in the final summary");
}

#[test]
fn project_emit_interface_check_changed_only_preserves_regenerate_hint_for_invalid_package() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-interface-check-changed-only-invalid-package");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create project source directory for changed-only invalid interface check test");
    let manifest_path = temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace/app/src/lib.ql",
        r#"
package demo.app

pub fn exported() -> Int {
    return 1
}
"#,
    );
    temp.write("workspace/app/app.qi", "broken interface\n");

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "emit-interface", "--changed-only", "--check"])
        .arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project emit-interface --changed-only --check` invalid package",
    );
    let (_stdout, stderr) = expect_exit_code(
        "project-interface-check-changed-only-invalid-package",
        "changed-only invalid package interface check",
        &output,
        1,
    )
    .expect("changed-only invalid package interface check should fail");
    let normalized_stderr = stderr.replace('\\', "/");
    let package_note = format!(
        "note: failing package manifest: {}",
        manifest_path.display().to_string().replace('\\', "/")
    );
    let rerun_hint = format!(
        "hint: rerun `ql project emit-interface {} --changed-only` to regenerate it",
        manifest_path.display().to_string().replace('\\', "/")
    );
    let default_rerun_hint = format!(
        "hint: rerun `ql project emit-interface {}` to regenerate it",
        manifest_path.display().to_string().replace('\\', "/")
    );
    expect_stderr_contains(
        "project-interface-check-changed-only-invalid-package",
        "changed-only invalid package interface check",
        &normalized_stderr,
        &rerun_hint,
    )
    .expect("changed-only invalid package interface check should preserve `--changed-only`");
    assert!(
        !normalized_stderr.contains(&default_rerun_hint),
        "changed-only invalid package interface check should not fall back to the default rerun hint, got:\n{stderr}"
    );
    let detail_line = "detail: expected `// qlang interface v1` header";
    let detail_index = normalized_stderr
        .find(detail_line)
        .expect("changed-only invalid package interface check should report parse detail");
    let package_note_index = normalized_stderr.find(&package_note).expect(
        "changed-only invalid package interface check should include the package manifest note",
    );
    let rerun_hint_index = normalized_stderr
        .find(&rerun_hint)
        .expect("changed-only invalid package interface check should include the rerun hint");
    assert!(
        detail_index < package_note_index && package_note_index < rerun_hint_index,
        "expected changed-only invalid package interface check to keep detail before manifest and hint, got:\n{stderr}"
    );
}

#[test]
fn project_emit_interface_changed_only_rewrites_only_stale_workspace_members() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-interface-changed-only-workspace");
    let project_root = temp.path().join("workspace-only");
    let app_root = project_root.join("packages").join("app");
    let tool_root = project_root.join("packages").join("tool");
    std::fs::create_dir_all(app_root.join("src"))
        .expect("create app package source directory for changed-only workspace test");
    std::fs::create_dir_all(tool_root.join("src"))
        .expect("create tool package source directory for changed-only workspace test");
    temp.write(
        "workspace-only/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/tool"]
"#,
    );
    temp.write(
        "workspace-only/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace-only/packages/app/src/lib.ql",
        r#"
package demo.app

pub fn exported() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace-only/packages/app/app.qi",
        "\
// qlang interface v1
// package: app

// source: src/lib.ql
package demo.app

pub fn exported() -> Int
",
    );
    temp.write(
        "workspace-only/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );
    temp.write(
        "workspace-only/packages/tool/src/lib.ql",
        r#"
package demo.tool

pub fn exported() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace-only/packages/tool/tool.qi",
        "\
// qlang interface v1
// package: tool

// source: src/lib.ql
package demo.tool

pub fn exported() -> Int
",
    );
    std::thread::sleep(std::time::Duration::from_millis(1200));
    temp.write(
        "workspace-only/packages/tool/src/lib.ql",
        r#"
package demo.tool

pub fn exported() -> Int {
    return 1
}

pub fn newer() -> Int {
    return 2
}
"#,
    );
    let app_interface = app_root.join("app.qi");
    let tool_interface = tool_root.join("tool.qi");

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "emit-interface", "--changed-only"])
        .arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project emit-interface --changed-only` workspace-only manifest",
    );
    let (stdout, stderr) = expect_success(
        "project-interface-changed-only-workspace",
        "changed-only workspace interface emission",
        &output,
    )
    .expect("changed-only workspace interface emission should succeed");
    let normalized_stdout = stdout.replace('\\', "/");
    let normalized_app_interface = app_interface.display().to_string().replace('\\', "/");
    let normalized_tool_interface = tool_interface.display().to_string().replace('\\', "/");
    expect_stdout_contains_all(
        "project-interface-changed-only-workspace",
        &normalized_stdout,
        &[
            &format!("up-to-date interface: {normalized_app_interface}"),
            &format!("wrote interface: {normalized_tool_interface}"),
        ],
    )
    .expect("changed-only workspace interface emission should skip valid member and rewrite stale member");
    expect_snapshot_matches(
        "project-interface-changed-only-workspace",
        "changed-only workspace interface emission stderr",
        "",
        &stderr,
    )
    .expect("changed-only workspace interface emission should stay silent on stderr");
    let tool_actual = read_normalized_file(&tool_interface, "changed-only workspace tool qi");
    assert!(
        tool_actual.contains("pub fn newer() -> Int"),
        "expected stale workspace member interface to be regenerated, got:\n{tool_actual}"
    );
}

#[test]
fn project_emit_interface_changed_only_preserves_workspace_output_path_hints() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-interface-changed-only-workspace-output-path");
    let project_root = temp.path().join("workspace-only");
    let app_root = project_root.join("packages").join("app");
    let tool_root = project_root.join("packages").join("tool");
    std::fs::create_dir_all(app_root.join("src"))
        .expect("create app package source directory for changed-only workspace output-path test");
    std::fs::create_dir_all(tool_root.join("src"))
        .expect("create tool package source directory for changed-only workspace output-path test");
    temp.write(
        "workspace-only/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/tool"]
"#,
    );
    temp.write(
        "workspace-only/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace-only/packages/app/src/lib.ql",
        r#"
package demo.app

pub fn exported() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace-only/packages/app/app.qi",
        "\
// qlang interface v1
// package: app

// source: src/lib.ql
package demo.app

pub fn exported() -> Int
",
    );
    temp.write(
        "workspace-only/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );
    temp.write(
        "workspace-only/packages/tool/src/lib.ql",
        r#"
package demo.tool

pub fn exported() -> Int {
    return 1
}
"#,
    );
    let tool_interface = tool_root.join("tool.qi");
    std::fs::create_dir_all(&tool_interface)
        .expect("create blocking interface directory for changed-only workspace output-path test");
    let tool_manifest = tool_root.join("qlang.toml");
    let tool_manifest_display = tool_manifest.to_string_lossy().replace('\\', "/");
    let tool_interface_display = tool_interface.to_string_lossy().replace('\\', "/");
    let app_interface_display = app_root
        .join("app.qi")
        .display()
        .to_string()
        .replace('\\', "/");

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "emit-interface", "--changed-only"])
        .arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project emit-interface --changed-only` workspace-only manifest with blocked member output path",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-interface-changed-only-workspace-output-path",
        "changed-only workspace interface emission with blocked member output path",
        &output,
        1,
    )
    .expect(
        "changed-only workspace interface emission with blocked member output path should fail",
    );
    let normalized_stdout = stdout.replace('\\', "/");
    expect_stdout_contains_all(
        "project-interface-changed-only-workspace-output-path",
        &normalized_stdout,
        &[&format!("up-to-date interface: {app_interface_display}")],
    )
    .expect(
        "changed-only workspace output-path failures should still report skipped valid members",
    );
    let normalized_stderr = stderr.replace('\\', "/");
    let package_note = format!("note: failing package manifest: {tool_manifest_display}");
    let member_note = format!("note: failing workspace member manifest: {tool_manifest_display}");
    let output_note = format!("note: failing interface output path: {tool_interface_display}");
    let rerun_hint = format!(
        "hint: rerun `ql project emit-interface {} --changed-only` after fixing the interface output path",
        tool_manifest_display
    );
    let default_rerun_hint = format!(
        "hint: rerun `ql project emit-interface {}` after fixing the interface output path",
        tool_manifest_display
    );
    expect_stderr_contains(
        "project-interface-changed-only-workspace-output-path",
        "changed-only workspace interface emission with blocked member output path",
        &normalized_stderr,
        &rerun_hint,
    )
    .expect("changed-only workspace output-path failures should preserve `--changed-only`");
    assert!(
        !normalized_stderr.contains(&default_rerun_hint),
        "changed-only workspace output-path failures should not fall back to the default rerun hint, got:\n{stderr}"
    );
    let package_note_index = normalized_stderr.find(&package_note).expect(
        "changed-only workspace output-path failure should include the package manifest note",
    );
    let member_note_index = normalized_stderr.find(&member_note).expect(
        "changed-only workspace output-path failure should include the workspace member note",
    );
    let output_note_index = normalized_stderr
        .find(&output_note)
        .expect("changed-only workspace output-path failure should include the output-path note");
    let rerun_hint_index = normalized_stderr
        .find(&rerun_hint)
        .expect("changed-only workspace output-path failure should include the rerun hint");
    assert!(
        package_note_index < member_note_index
            && member_note_index < output_note_index
            && output_note_index < rerun_hint_index,
        "expected changed-only workspace output-path context before rerun hint, got:\n{stderr}"
    );
    expect_stderr_contains(
        "project-interface-changed-only-workspace-output-path",
        "changed-only workspace interface emission with blocked member output path",
        &stderr,
        "interface emission found 1 failing member(s)",
    )
    .expect("changed-only workspace output-path failures should still summarize failing members");
    expect_stderr_not_contains(
        "project-interface-changed-only-workspace-output-path",
        "changed-only workspace interface emission with blocked member output path",
        &normalized_stderr,
        "note: first failing member manifest:",
    )
    .expect("single failing changed-only workspace output-path failures should not repeat the manifest in the final summary");
    assert!(
        tool_interface.is_dir(),
        "changed-only workspace output-path failure should preserve `{}` as a directory",
        tool_interface.display()
    );
}

#[test]
fn project_emit_interface_check_rejects_stale_workspace_member_interface() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-interface-check-workspace");
    let project_root = temp.path().join("workspace-only");
    let app_root = project_root.join("packages").join("app");
    let tool_root = project_root.join("packages").join("tool");
    let broken_root = project_root.join("packages").join("broken");
    std::fs::create_dir_all(app_root.join("src"))
        .expect("create app package source directory for workspace interface check test");
    std::fs::create_dir_all(tool_root.join("src"))
        .expect("create tool package source directory for workspace interface check test");
    std::fs::create_dir_all(broken_root.join("src"))
        .expect("create broken package source directory for workspace interface check test");
    temp.write(
        "workspace-only/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/tool", "packages/broken"]
"#,
    );
    temp.write(
        "workspace-only/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace-only/packages/app/src/lib.ql",
        r#"
package demo.app

pub fn exported() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace-only/packages/app/app.qi",
        "\
// qlang interface v1
// package: app

// source: src/lib.ql
package demo.app

pub fn exported() -> Int
",
    );
    temp.write(
        "workspace-only/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );
    temp.write(
        "workspace-only/packages/tool/src/lib.ql",
        r#"
package demo.tool

pub fn exported() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace-only/packages/tool/tool.qi",
        "\
// qlang interface v1
// package: tool

// source: src/lib.ql
package demo.tool

pub fn exported() -> Int
",
    );
    temp.write(
        "workspace-only/packages/broken/qlang.toml",
        r#"
[package]
name = "broken"
"#,
    );
    temp.write(
        "workspace-only/packages/broken/src/lib.ql",
        r#"
package demo.broken

pub fn exported() -> Int {
    return 3
}
"#,
    );
    std::thread::sleep(std::time::Duration::from_millis(1200));
    temp.write(
        "workspace-only/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "emit-interface", "--check"])
        .arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project emit-interface --check` workspace-only manifest",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-interface-check-workspace",
        "workspace interface check with stale member",
        &output,
        1,
    )
    .expect("workspace interface check with stale member should fail");
    let normalized_stdout = stdout.replace('\\', "/");
    let normalized_stderr = stderr.replace('\\', "/");
    let normalized_app_interface = app_root
        .join("app.qi")
        .display()
        .to_string()
        .replace('\\', "/");
    expect_stdout_contains_all(
        "project-interface-check-workspace",
        &normalized_stdout,
        &[&format!("ok interface: {normalized_app_interface}")],
    )
    .expect("workspace interface check should still report valid members");
    expect_stderr_contains(
        "project-interface-check-workspace",
        "workspace interface check with stale member",
        &stderr,
        "is stale",
    )
    .expect("workspace interface check should surface stale member interface status");
    expect_stderr_contains(
        "project-interface-check-workspace",
        "workspace interface check with stale member",
        &stderr,
        "reason: manifest newer than artifact:",
    )
    .expect("workspace interface check should explain why the stale member interface is stale");
    expect_stderr_contains(
        "project-interface-check-workspace",
        "workspace interface check with stale member",
        &stderr,
        "is missing",
    )
    .expect("workspace interface check should also surface missing member interface status");
    expect_stderr_contains(
        "project-interface-check-workspace",
        "workspace interface check with stale member",
        &normalized_stderr,
        &format!(
            "note: failing package manifest: {}",
            tool_root
                .join("qlang.toml")
                .display()
                .to_string()
                .replace('\\', "/")
        ),
    )
    .expect("workspace interface check should point stale member failures at the package manifest");
    expect_stderr_contains(
        "project-interface-check-workspace",
        "workspace interface check with stale member",
        &normalized_stderr,
        &format!(
            "note: failing workspace member manifest: {}",
            tool_root
                .join("qlang.toml")
                .display()
                .to_string()
                .replace('\\', "/")
        ),
    )
    .expect("workspace interface check should also keep the stale member boundary visible");
    let package_note = format!(
        "note: failing package manifest: {}",
        tool_root
            .join("qlang.toml")
            .display()
            .to_string()
            .replace('\\', "/")
    );
    let member_note = format!(
        "note: failing workspace member manifest: {}",
        tool_root
            .join("qlang.toml")
            .display()
            .to_string()
            .replace('\\', "/")
    );
    let rerun_hint = format!(
        "hint: rerun `ql project emit-interface {} --changed-only` to regenerate it",
        tool_root
            .join("qlang.toml")
            .display()
            .to_string()
            .replace('\\', "/")
    );
    let default_rerun_hint = format!(
        "hint: rerun `ql project emit-interface {}` to regenerate it",
        tool_root
            .join("qlang.toml")
            .display()
            .to_string()
            .replace('\\', "/")
    );
    let package_note_index = normalized_stderr
        .find(&package_note)
        .expect("workspace stale member failure should include the package manifest note");
    let member_note_index = normalized_stderr
        .find(&member_note)
        .expect("workspace stale member failure should include the workspace member note");
    let rerun_hint_index = normalized_stderr
        .find(&rerun_hint)
        .expect("workspace stale member failure should include the rerun hint");
    assert!(
        package_note_index < member_note_index && member_note_index < rerun_hint_index,
        "expected workspace stale member context before hint, got:\n{stderr}"
    );
    assert!(
        !normalized_stderr.contains(&default_rerun_hint),
        "changed-only workspace interface check should not fall back to the default rerun hint, got:\n{stderr}"
    );
    expect_stderr_contains(
        "project-interface-check-workspace",
        "workspace interface check with stale member",
        &stderr,
        "found 2 failing member(s)",
    )
    .expect("workspace interface check should summarize all failing members");
    expect_stderr_contains(
        "project-interface-check-workspace",
        "workspace interface check with stale member",
        &normalized_stderr,
        &format!(
            "note: first failing member manifest: {}",
            tool_root
                .join("qlang.toml")
                .display()
                .to_string()
                .replace('\\', "/")
        ),
    )
    .expect("workspace interface check should point to the first failing member manifest");
}

#[test]
fn project_emit_interface_check_keeps_checking_other_workspace_members_when_one_member_manifest_is_invalid()
 {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-interface-check-workspace-invalid-member");
    let project_root = temp.path().join("workspace-only");
    let app_root = project_root.join("packages").join("app");
    let broken_root = project_root.join("packages").join("broken");
    std::fs::create_dir_all(app_root.join("src"))
        .expect("create app package source directory for invalid member workspace check test");
    std::fs::create_dir_all(&broken_root)
        .expect("create broken package directory for invalid member workspace check test");
    temp.write(
        "workspace-only/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/broken"]
"#,
    );
    temp.write(
        "workspace-only/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace-only/packages/app/src/lib.ql",
        r#"
package demo.app

pub fn exported() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace-only/packages/app/app.qi",
        "\
// qlang interface v1
// package: app

// source: src/lib.ql
package demo.app

pub fn exported() -> Int
",
    );
    temp.write(
        "workspace-only/packages/broken/qlang.toml",
        r#"
[package
name = "broken"
"#,
    );

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "emit-interface", "--check"])
        .arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project emit-interface --check` workspace manifest with invalid member",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-interface-check-workspace-invalid-member",
        "workspace interface check with invalid member manifest",
        &output,
        1,
    )
    .expect("workspace interface check with invalid member manifest should fail");
    let normalized_stdout = stdout.replace('\\', "/");
    let normalized_stderr = stderr.replace('\\', "/");
    let normalized_app_interface = app_root
        .join("app.qi")
        .display()
        .to_string()
        .replace('\\', "/");
    expect_stdout_contains_all(
        "project-interface-check-workspace-invalid-member",
        &normalized_stdout,
        &[&format!("ok interface: {normalized_app_interface}")],
    )
    .expect("workspace interface check should still report healthy members before failing");
    expect_stderr_contains(
        "project-interface-check-workspace-invalid-member",
        "workspace interface check with invalid member manifest",
        &stderr,
        "invalid manifest",
    )
    .expect("workspace interface check should surface the invalid member manifest");
    expect_stderr_contains(
        "project-interface-check-workspace-invalid-member",
        "workspace interface check with invalid member manifest",
        &normalized_stderr,
        &format!(
            "note: failing workspace member manifest: {}",
            broken_root
                .join("qlang.toml")
                .display()
                .to_string()
                .replace('\\', "/")
        ),
    )
    .expect("workspace interface check should point invalid member manifests locally");
    let rerun_hint = format!(
        "hint: rerun `ql project emit-interface {} --check` after fixing the workspace member manifest",
        broken_root
            .join("qlang.toml")
            .display()
            .to_string()
            .replace('\\', "/")
    );
    expect_stderr_contains(
        "project-interface-check-workspace-invalid-member",
        "workspace interface check with invalid member manifest",
        &normalized_stderr,
        &rerun_hint,
    )
    .expect("workspace interface check should suggest rerunning the broken member directly after repair");
    expect_stderr_contains(
        "project-interface-check-workspace-invalid-member",
        "workspace interface check with invalid member manifest",
        &stderr,
        "found 1 failing member(s)",
    )
    .expect("workspace interface check should summarize all failing members");
    expect_stderr_not_contains(
        "project-interface-check-workspace-invalid-member",
        "workspace interface check with invalid member manifest",
        &normalized_stderr,
        "note: first failing member manifest:",
    )
    .expect("single failing workspace members should not repeat the manifest in the final summary");
}

#[test]
fn project_emit_interface_check_changed_only_continues_after_workspace_non_package_member() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-interface-check-changed-only-workspace-not-package");
    let project_root = temp.path().join("workspace-only");
    let app_root = project_root.join("packages").join("app");
    let broken_root = project_root.join("packages").join("broken");
    let tool_root = project_root.join("packages").join("tool");
    std::fs::create_dir_all(app_root.join("src"))
        .expect("create app package source directory for changed-only workspace semantic-invalid member test");
    std::fs::create_dir_all(&broken_root).expect(
        "create broken package directory for changed-only workspace semantic-invalid member test",
    );
    std::fs::create_dir_all(tool_root.join("src"))
        .expect("create tool package source directory for changed-only workspace semantic-invalid member test");
    temp.write(
        "workspace-only/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/broken", "packages/tool"]
"#,
    );
    temp.write(
        "workspace-only/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace-only/packages/app/src/lib.ql",
        r#"
package demo.app

pub fn exported() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace-only/packages/app/app.qi",
        "\
// qlang interface v1
// package: app

// source: src/lib.ql
package demo.app

pub fn exported() -> Int
",
    );
    temp.write(
        "workspace-only/packages/broken/qlang.toml",
        r#"
[workspace]
members = []
"#,
    );
    temp.write(
        "workspace-only/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );
    temp.write(
        "workspace-only/packages/tool/src/lib.ql",
        r#"
package demo.tool

pub fn exported() -> Int {
    return 2
}
"#,
    );
    temp.write(
        "workspace-only/packages/tool/tool.qi",
        "\
// qlang interface v1
// package: tool

// source: src/lib.ql
package demo.tool

pub fn exported() -> Int
",
    );

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "emit-interface", "--changed-only", "--check"])
        .arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project emit-interface --changed-only --check` workspace manifest with non-package member",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-interface-check-changed-only-workspace-not-package",
        "changed-only workspace interface check with non-package member",
        &output,
        1,
    )
    .expect("changed-only workspace interface check with non-package member should fail");
    let normalized_stdout = stdout.replace('\\', "/");
    let normalized_stderr = stderr.replace('\\', "/");
    let app_interface = app_root
        .join("app.qi")
        .display()
        .to_string()
        .replace('\\', "/");
    let tool_interface = tool_root
        .join("tool.qi")
        .display()
        .to_string()
        .replace('\\', "/");
    expect_stdout_contains_all(
        "project-interface-check-changed-only-workspace-not-package",
        &normalized_stdout,
        &[
            &format!("up-to-date interface: {app_interface}"),
            &format!("up-to-date interface: {tool_interface}"),
        ],
    )
    .expect("changed-only workspace interface check should continue reporting later valid members");
    let broken_manifest = broken_root
        .join("qlang.toml")
        .display()
        .to_string()
        .replace('\\', "/");
    let error_line = format!(
        "error: `ql project emit-interface --changed-only --check` manifest `{broken_manifest}` does not declare `[package].name`"
    );
    let old_error_line = format!(
        "error: `ql project emit-interface --check` manifest `{broken_manifest}` does not declare `[package].name`"
    );
    let member_note = format!("note: failing workspace member manifest: {broken_manifest}");
    let rerun_hint = format!(
        "hint: rerun `ql project emit-interface {broken_manifest} --changed-only --check` after fixing the workspace member manifest"
    );
    expect_stderr_contains(
        "project-interface-check-changed-only-workspace-not-package",
        "changed-only workspace interface check with non-package member",
        &normalized_stderr,
        &error_line,
    )
    .expect("changed-only workspace semantic-invalid member errors should preserve the full command label");
    expect_stderr_not_contains(
        "project-interface-check-changed-only-workspace-not-package",
        "changed-only workspace interface check with non-package member",
        &normalized_stderr,
        &old_error_line,
    )
    .expect("changed-only workspace semantic-invalid member errors should not fall back to the default command label");
    let error_line_index = normalized_stderr
        .find(&error_line)
        .expect("changed-only workspace semantic-invalid member errors should include the full command label");
    let member_note_index = normalized_stderr
        .find(&member_note)
        .expect("changed-only workspace semantic-invalid member errors should include the local member note");
    let rerun_hint_index = normalized_stderr.find(&rerun_hint).expect(
        "changed-only workspace semantic-invalid member errors should include the rerun hint",
    );
    assert!(
        error_line_index < member_note_index && member_note_index < rerun_hint_index,
        "expected changed-only workspace semantic-invalid member context before rerun hint, got:\n{stderr}"
    );
    expect_stderr_contains(
        "project-interface-check-changed-only-workspace-not-package",
        "changed-only workspace interface check with non-package member",
        &stderr,
        "found 1 failing member(s)",
    )
    .expect("changed-only workspace semantic-invalid member failures should still summarize failing members");
    expect_stderr_not_contains(
        "project-interface-check-changed-only-workspace-not-package",
        "changed-only workspace interface check with non-package member",
        &normalized_stderr,
        "note: first failing member manifest:",
    )
    .expect("single changed-only workspace semantic-invalid member failures should not repeat the manifest in the final summary");
}

#[test]
fn project_emit_interface_check_changed_only_skips_valid_workspace_members() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-interface-check-changed-only-workspace");
    let project_root = temp.path().join("workspace-only");
    let app_root = project_root.join("packages").join("app");
    let tool_root = project_root.join("packages").join("tool");
    let broken_root = project_root.join("packages").join("broken");
    std::fs::create_dir_all(app_root.join("src")).expect(
        "create app package source directory for changed-only workspace interface check test",
    );
    std::fs::create_dir_all(tool_root.join("src")).expect(
        "create tool package source directory for changed-only workspace interface check test",
    );
    std::fs::create_dir_all(broken_root.join("src")).expect(
        "create broken package source directory for changed-only workspace interface check test",
    );
    temp.write(
        "workspace-only/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/tool", "packages/broken"]
"#,
    );
    temp.write(
        "workspace-only/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace-only/packages/app/src/lib.ql",
        r#"
package demo.app

pub fn exported() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace-only/packages/app/app.qi",
        "\
// qlang interface v1
// package: app

// source: src/lib.ql
package demo.app

pub fn exported() -> Int
",
    );
    temp.write(
        "workspace-only/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );
    temp.write(
        "workspace-only/packages/tool/src/lib.ql",
        r#"
package demo.tool

pub fn exported() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace-only/packages/tool/tool.qi",
        "\
// qlang interface v1
// package: tool

// source: src/lib.ql
package demo.tool

pub fn exported() -> Int
",
    );
    temp.write(
        "workspace-only/packages/broken/qlang.toml",
        r#"
[package]
name = "broken"
"#,
    );
    temp.write(
        "workspace-only/packages/broken/src/lib.ql",
        r#"
package demo.broken

pub fn exported() -> Int {
    return 3
}
"#,
    );
    let app_interface = app_root.join("app.qi");
    let app_metadata_before = std::fs::metadata(&app_interface)
        .expect("read app interface metadata before changed-only workspace check")
        .modified()
        .expect("read app interface modification time before changed-only workspace check");
    std::thread::sleep(std::time::Duration::from_millis(1200));
    temp.write(
        "workspace-only/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "emit-interface", "--changed-only", "--check"])
        .arg(&project_root);
    let output = run_command_capture(
        &mut command,
        "`ql project emit-interface --changed-only --check` workspace-only manifest",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-interface-check-changed-only-workspace",
        "changed-only workspace interface check with stale member",
        &output,
        1,
    )
    .expect("changed-only workspace interface check with stale member should fail");
    let normalized_stdout = stdout.replace('\\', "/");
    let normalized_app_interface = app_interface.display().to_string().replace('\\', "/");
    expect_snapshot_matches(
        "project-interface-check-changed-only-workspace",
        "changed-only workspace interface check stdout",
        &format!("up-to-date interface: {normalized_app_interface}\n"),
        &normalized_stdout,
    )
    .expect("changed-only workspace interface check should report valid members as up to date");
    expect_stderr_contains(
        "project-interface-check-changed-only-workspace",
        "changed-only workspace interface check with stale member",
        &stderr,
        "is stale",
    )
    .expect("changed-only workspace interface check should surface stale member interface status");
    expect_stderr_contains(
        "project-interface-check-changed-only-workspace",
        "changed-only workspace interface check with stale member",
        &stderr,
        "reason: manifest newer than artifact:",
    )
    .expect(
        "changed-only workspace interface check should explain why the stale member interface is stale",
    );
    expect_stderr_contains(
        "project-interface-check-changed-only-workspace",
        "changed-only workspace interface check with stale member",
        &stderr,
        "is missing",
    )
    .expect("changed-only workspace interface check should also surface missing member interface status");
    expect_stderr_contains(
        "project-interface-check-changed-only-workspace",
        "changed-only workspace interface check with stale member",
        &stderr,
        "found 2 failing member(s)",
    )
    .expect("changed-only workspace interface check should summarize all failing members");
    let app_metadata_after = std::fs::metadata(&app_interface)
        .expect("read app interface metadata after changed-only workspace check")
        .modified()
        .expect("read app interface modification time after changed-only workspace check");
    assert_eq!(
        app_metadata_before,
        app_metadata_after,
        "expected changed-only workspace interface check not to rewrite `{}`",
        app_interface.display()
    );
}

#[test]
fn project_emit_interface_rejects_output_path_for_workspace_only_manifest() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-project-interface-workspace-output");
    let project_root = temp.path().join("workspace-only");
    std::fs::create_dir_all(project_root.join("packages").join("app").join("src"))
        .expect("create workspace-only package directory");
    temp.write(
        "workspace-only/qlang.toml",
        r#"
[workspace]
members = ["packages/app"]
"#,
    );
    temp.write(
        "workspace-only/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace-only/packages/app/src/lib.ql",
        r#"
package demo.app

pub fn exported() -> Int {
    return 1
}
"#,
    );

    let mut command = ql_command(&workspace_root);
    command
        .args(["project", "emit-interface"])
        .arg(&project_root)
        .args(["--output", "workspace.qi"]);
    let output = run_command_capture(
        &mut command,
        "`ql project emit-interface --output` workspace-only manifest",
    );
    let (stdout, stderr) = expect_exit_code(
        "project-interface-workspace-output",
        "workspace-only interface emission with output",
        &output,
        1,
    )
    .expect("workspace-only interface emission with output should fail");
    expect_empty_stdout(
        "project-interface-workspace-output",
        "workspace-only interface emission with output",
        &stdout,
    )
    .expect("workspace-only interface emission with output should not print stdout");
    expect_stderr_contains(
        "project-interface-workspace-output",
        "workspace-only interface emission with output",
        &stderr,
        "--output` only supports package manifests",
    )
    .expect("workspace-only output rejection should explain the package-only constraint");
}

#[test]
fn build_with_emit_interface_writes_default_package_qi() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-build-emit-interface");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create project source directory for build interface test");
    let source_path = temp.write(
        "workspace/app/src/lib.ql",
        r#"
package demo.app

pub struct Buffer {
    value: Int,
}

pub fn exported(value: Int) -> Int {
    return value
}

fn main() -> Int {
    return exported(1)
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    let output_path = project_root.join("build").join("app.ll");
    let interface_path = project_root.join("app.qi");

    let mut command = ql_command(&workspace_root);
    command
        .arg("build")
        .arg(&source_path)
        .args(["--emit", "llvm-ir", "--output"])
        .arg(&output_path)
        .arg("--emit-interface");
    let output = run_command_capture(&mut command, "`ql build --emit-interface`");
    let (stdout, stderr) = expect_success(
        "build-emit-interface-success",
        "build with interface emission",
        &output,
    )
    .expect("build with interface emission should succeed");
    expect_stdout_contains_all(
        "build-emit-interface-success",
        &stdout,
        &[
            &format!("wrote llvm-ir: {}", output_path.display()),
            "wrote interface:",
            "app.qi",
        ],
    )
    .expect("build with interface emission should report both output artifacts");
    expect_snapshot_matches(
        "build-emit-interface-success",
        "build with interface emission stderr",
        "",
        &stderr,
    )
    .expect("successful build with interface emission should stay silent on stderr");
    expect_file_exists(
        "build-emit-interface-success",
        &output_path,
        "generated llvm ir",
        "build with interface emission",
    )
    .expect("build with interface emission should create the requested build artifact");
    expect_file_exists(
        "build-emit-interface-success",
        &interface_path,
        "generated interface",
        "build with interface emission",
    )
    .expect("build with interface emission should create the default package qi artifact");

    let expected = "\
// qlang interface v1
// package: app

// source: src/lib.ql
package demo.app

pub struct Buffer {
    value: Int,
}

pub fn exported(value: Int) -> Int
";
    let actual = read_normalized_file(&interface_path, "generated qi artifact");
    expect_snapshot_matches(
        "build-emit-interface-success",
        "generated qi artifact",
        expected,
        &actual,
    )
    .expect("generated qi artifact should match the build-side public interface snapshot");
}

#[test]
fn build_with_emit_interface_preserves_build_diagnostic_rerun_hint() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-build-emit-interface-build-diagnostics");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create project source directory for build-side diagnostics test");
    let source_path = temp.write(
        "workspace/app/src/lib.ql",
        r#"
package demo.app

pub fn broken(value: MissingType) -> Int {
    return value
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"
version = "0.1.0"
"#,
    );

    let manifest_path = project_root.join("qlang.toml");
    let output_path = project_root.join("build").join("app.lib");
    let header_path = project_root.join("include").join("app.h");
    let interface_path = project_root.join("app.qi");
    let manifest_display = manifest_path.display().to_string().replace('\\', "/");
    let source_display = source_path.display().to_string().replace('\\', "/");
    let output_display = output_path.display().to_string().replace('\\', "/");
    let header_display = header_path.display().to_string().replace('\\', "/");

    let mut command = ql_command(&workspace_root);
    command
        .arg("build")
        .arg(&source_path)
        .args(["--emit", "staticlib", "--release", "--output"])
        .arg(&output_path)
        .args(["--header-surface", "both", "--header-output"])
        .arg(&header_path)
        .arg("--emit-interface");
    let output = run_command_capture(
        &mut command,
        "`ql build --emit-interface` with build-side source diagnostics",
    );
    let (stdout, stderr) = expect_exit_code(
        "build-emit-interface-build-diagnostics",
        "build with source diagnostics before interface emission",
        &output,
        1,
    )
    .expect("build should fail when the requested source has diagnostics");
    expect_snapshot_matches(
        "build-emit-interface-build-diagnostics",
        "build with source diagnostics stdout",
        "",
        &stdout,
    )
    .expect("build-side source diagnostics should not report a successful build artifact");
    expect_stderr_contains(
        "build-emit-interface-build-diagnostics",
        "build with source diagnostics before interface emission",
        &stderr,
        "MissingType",
    )
    .expect("build-side source diagnostics should still surface the failing symbol");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stderr_contains(
        "build-emit-interface-build-diagnostics",
        "build with source diagnostics before interface emission",
        &normalized_stderr,
        &source_display,
    )
    .expect("build-side source diagnostics should point at the failing build source");
    expect_stderr_contains(
        "build-emit-interface-build-diagnostics",
        "build with source diagnostics before interface emission",
        &normalized_stderr,
        &format!("note: failing package manifest: {manifest_display}"),
    )
    .expect("build-side source diagnostics should point to the failing package manifest");
    expect_stderr_contains(
        "build-emit-interface-build-diagnostics",
        "build with source diagnostics before interface emission",
        &normalized_stderr,
        &format!(
            "hint: rerun `ql build {} --emit staticlib --release --output {} --header-surface both --header-output {} --emit-interface` after fixing the package sources",
            source_display, output_display, header_display
        ),
    )
    .expect("build-side source diagnostics should preserve the build rerun options");
    expect_stderr_not_contains(
        "build-emit-interface-build-diagnostics",
        "build with source diagnostics before interface emission",
        &normalized_stderr,
        "note: build artifact remains at `",
    )
    .expect("build-side source diagnostics should not claim that a build artifact was preserved");
    assert!(
        !output_path.is_file(),
        "build-side source diagnostics should not create `{}`",
        output_path.display()
    );
    assert!(
        !interface_path.is_file(),
        "build-side source diagnostics should not create `{}`",
        interface_path.display()
    );
    assert!(
        !header_path.is_file(),
        "build-side source diagnostics should not create `{}`",
        header_path.display()
    );
}

#[test]
fn build_with_emit_interface_preserves_header_configuration_rerun_hint() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-build-emit-interface-header-config");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create project source directory for header configuration test");
    let source_path = temp.write(
        "workspace/app/src/lib.ql",
        r#"
package demo.app

fn main() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"
version = "0.1.0"
"#,
    );

    let manifest_path = project_root.join("qlang.toml");
    let output_path = project_root.join("build").join("app.ll");
    let header_path = project_root.join("include").join("app.ffi.h");
    let interface_path = project_root.join("app.qi");
    let manifest_display = manifest_path.display().to_string().replace('\\', "/");
    let source_display = source_path.display().to_string().replace('\\', "/");
    let output_display = output_path.display().to_string().replace('\\', "/");
    let header_display = header_path.display().to_string().replace('\\', "/");

    let mut command = ql_command(&workspace_root);
    command
        .arg("build")
        .arg(&source_path)
        .args(["--emit", "llvm-ir", "--release", "--output"])
        .arg(&output_path)
        .args(["--header-surface", "both", "--header-output"])
        .arg(&header_path)
        .arg("--emit-interface");
    let output = run_command_capture(
        &mut command,
        "`ql build --emit-interface` with unsupported build header configuration",
    );
    let (stdout, stderr) = expect_exit_code(
        "build-emit-interface-header-config",
        "build with unsupported build header configuration",
        &output,
        1,
    )
    .expect("build should fail when build-side headers are requested for a non-library emit");
    expect_snapshot_matches(
        "build-emit-interface-header-config",
        "build with unsupported build header configuration stdout",
        "",
        &stdout,
    )
    .expect("unsupported build header configuration should not report a successful build artifact");
    expect_stderr_contains(
        "build-emit-interface-header-config",
        "build with unsupported build header configuration",
        &stderr,
        "only supports `dylib` and `staticlib`",
    )
    .expect("unsupported build header configuration should surface the compatibility error");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stderr_contains(
        "build-emit-interface-header-config",
        "build with unsupported build header configuration",
        &normalized_stderr,
        &format!("note: failing package manifest: {manifest_display}"),
    )
    .expect("unsupported build header configuration should point to the failing package manifest");
    expect_stderr_contains(
        "build-emit-interface-header-config",
        "build with unsupported build header configuration",
        &normalized_stderr,
        &format!(
            "hint: rerun `ql build {} --emit llvm-ir --release --output {} --header-surface both --header-output {} --emit-interface` after fixing the build header configuration",
            source_display, output_display, header_display
        ),
    )
    .expect("unsupported build header configuration should preserve the build rerun options");
    expect_stderr_not_contains(
        "build-emit-interface-header-config",
        "build with unsupported build header configuration",
        &normalized_stderr,
        "note: build artifact remains at `",
    )
    .expect("unsupported build header configuration should not claim that a build artifact was preserved");
    expect_stderr_not_contains(
        "build-emit-interface-header-config",
        "build with unsupported build header configuration",
        &normalized_stderr,
        "note: failing build header output path:",
    )
    .expect(
        "unsupported build header configuration should not be mislabeled as an output-path failure",
    );
    assert!(
        !output_path.is_file(),
        "unsupported build header configuration should not create `{}`",
        output_path.display()
    );
    assert!(
        !header_path.is_file(),
        "unsupported build header configuration should not create `{}`",
        header_path.display()
    );
    assert!(
        !interface_path.is_file(),
        "unsupported build header configuration should not create `{}`",
        interface_path.display()
    );
}

#[test]
fn build_with_emit_interface_preserves_dylib_export_rerun_hint() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-build-emit-interface-dylib-exports");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create project source directory for dylib export test");
    let source_path = temp.write(
        "workspace/app/src/lib.ql",
        r#"
package demo.app

fn main() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"
version = "0.1.0"
"#,
    );

    let manifest_path = project_root.join("qlang.toml");
    let output_path = project_root.join("build").join("app.dll");
    let interface_path = project_root.join("app.qi");
    let manifest_display = manifest_path.display().to_string().replace('\\', "/");
    let source_display = source_path.display().to_string().replace('\\', "/");
    let output_display = output_path.display().to_string().replace('\\', "/");

    let mut command = ql_command(&workspace_root);
    command
        .arg("build")
        .arg(&source_path)
        .args(["--emit", "dylib", "--release", "--output"])
        .arg(&output_path)
        .arg("--emit-interface");
    let output = run_command_capture(
        &mut command,
        "`ql build --emit-interface` with a dylib export configuration failure",
    );
    let (stdout, stderr) = expect_exit_code(
        "build-emit-interface-dylib-exports",
        "build with dylib export configuration failure",
        &output,
        1,
    )
    .expect("build should fail when dylib emission has no public extern exports");
    expect_snapshot_matches(
        "build-emit-interface-dylib-exports",
        "build with dylib export configuration failure stdout",
        "",
        &stdout,
    )
    .expect("dylib export configuration failure should not report a successful build artifact");
    expect_stderr_contains(
        "build-emit-interface-dylib-exports",
        "build with dylib export configuration failure",
        &stderr,
        "requires at least one public top-level `extern \"c\"` function definition",
    )
    .expect("dylib export configuration failure should surface the missing export requirement");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stderr_contains(
        "build-emit-interface-dylib-exports",
        "build with dylib export configuration failure",
        &normalized_stderr,
        &format!("note: failing package manifest: {manifest_display}"),
    )
    .expect("dylib export configuration failure should point to the failing package manifest");
    expect_stderr_contains(
        "build-emit-interface-dylib-exports",
        "build with dylib export configuration failure",
        &normalized_stderr,
        &format!(
            "hint: rerun `ql build {} --emit dylib --release --output {} --emit-interface` after fixing the dylib export surface",
            source_display, output_display
        ),
    )
    .expect("dylib export configuration failure should preserve the build rerun options");
    expect_stderr_not_contains(
        "build-emit-interface-dylib-exports",
        "build with dylib export configuration failure",
        &normalized_stderr,
        "note: build artifact remains at `",
    )
    .expect(
        "dylib export configuration failure should not claim that a build artifact was preserved",
    );
    assert!(
        !output_path.is_file(),
        "dylib export configuration failure should not create `{}`",
        output_path.display()
    );
    assert!(
        !interface_path.is_file(),
        "dylib export configuration failure should not create `{}`",
        interface_path.display()
    );
}

#[test]
fn build_with_emit_interface_preserves_header_import_surface_rerun_hint() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-build-emit-interface-header-imports");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create project source directory for header import surface test");
    let source_path = temp.write(
        "workspace/app/src/lib.ql",
        r#"
package demo.app

fn main() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"
version = "0.1.0"
"#,
    );

    let manifest_path = project_root.join("qlang.toml");
    let output_path = project_root.join("build").join("app.lib");
    let header_path = project_root.join("include").join("app.imports.h");
    let interface_path = project_root.join("app.qi");
    let manifest_display = manifest_path.display().to_string().replace('\\', "/");
    let source_display = source_path.display().to_string().replace('\\', "/");
    let output_display = output_path.display().to_string().replace('\\', "/");
    let header_display = header_path.display().to_string().replace('\\', "/");

    let mut command = ql_command(&workspace_root);
    command
        .arg("build")
        .arg(&source_path)
        .args(["--emit", "staticlib", "--release", "--output"])
        .arg(&output_path)
        .args(["--header-surface", "imports", "--header-output"])
        .arg(&header_path)
        .arg("--emit-interface");
    let output = run_command_capture(
        &mut command,
        "`ql build --emit-interface` with a build header import surface failure",
    );
    let (stdout, stderr) = expect_exit_code(
        "build-emit-interface-header-imports",
        "build with build header import surface failure",
        &output,
        1,
    )
    .expect("build should fail when the requested build header import surface is empty");
    expect_snapshot_matches(
        "build-emit-interface-header-imports",
        "build with build header import surface failure stdout",
        "",
        &stdout,
    )
    .expect("build header import surface failure should not report a successful build artifact");
    expect_stderr_contains(
        "build-emit-interface-header-imports",
        "build with build header import surface failure",
        &stderr,
        "does not define any imported `extern \"c\"` function declarations",
    )
    .expect("build header import surface failure should surface the missing import requirement");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stderr_contains(
        "build-emit-interface-header-imports",
        "build with build header import surface failure",
        &normalized_stderr,
        &format!("note: failing package manifest: {manifest_display}"),
    )
    .expect("build header import surface failure should point to the failing package manifest");
    expect_stderr_contains(
        "build-emit-interface-header-imports",
        "build with build header import surface failure",
        &normalized_stderr,
        &format!(
            "hint: rerun `ql build {} --emit staticlib --release --output {} --header-surface imports --header-output {} --emit-interface` after fixing the build header import surface",
            source_display, output_display, header_display
        ),
    )
    .expect("build header import surface failure should preserve the build rerun options");
    expect_stderr_not_contains(
        "build-emit-interface-header-imports",
        "build with build header import surface failure",
        &normalized_stderr,
        "note: build artifact remains at `",
    )
    .expect(
        "build header import surface failure should not claim that a build artifact was preserved",
    );
    expect_stderr_not_contains(
        "build-emit-interface-header-imports",
        "build with build header import surface failure",
        &normalized_stderr,
        "note: failing build header output path:",
    )
    .expect(
        "build header import surface failure should not be mislabeled as a header output-path failure",
    );
    assert!(
        !output_path.is_file(),
        "build header import surface failure should not create `{}`",
        output_path.display()
    );
    assert!(
        !header_path.is_file(),
        "build header import surface failure should not create `{}`",
        header_path.display()
    );
    assert!(
        !interface_path.is_file(),
        "build header import surface failure should not create `{}`",
        interface_path.display()
    );
}

#[test]
fn build_with_emit_interface_points_to_failing_build_input_path() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-build-emit-interface-input-path");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create project source directory for build input path test");
    temp.write(
        "workspace/app/src/lib.ql",
        r#"
package demo.app

extern "c" pub fn q_add(left: Int, right: Int) -> Int {
    return left + right
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"
version = "0.1.0"
"#,
    );

    let missing_source_path = project_root.join("src").join("missing.ql");
    let manifest_path = project_root.join("qlang.toml");
    let output_path = project_root.join("build").join("app.ll");
    let interface_path = project_root.join("app.qi");
    let manifest_display = manifest_path.display().to_string().replace('\\', "/");
    let source_display = missing_source_path.display().to_string().replace('\\', "/");
    let output_display = output_path.display().to_string().replace('\\', "/");

    let mut command = ql_command(&workspace_root);
    command
        .arg("build")
        .arg(&missing_source_path)
        .args(["--emit", "llvm-ir", "--release", "--output"])
        .arg(&output_path)
        .arg("--emit-interface");
    let output = run_command_capture(
        &mut command,
        "`ql build --emit-interface` with a missing build input path",
    );
    let (stdout, stderr) = expect_exit_code(
        "build-emit-interface-input-path",
        "build with missing build input path",
        &output,
        1,
    )
    .expect("build should fail when the requested build input path is not a file");
    expect_snapshot_matches(
        "build-emit-interface-input-path",
        "build with missing build input path stdout",
        "",
        &stdout,
    )
    .expect("missing build input path should not report a successful build artifact");
    expect_stderr_contains(
        "build-emit-interface-input-path",
        "build with missing build input path",
        &stderr,
        "is not a file",
    )
    .expect("missing build input path should surface the invalid input error");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stderr_contains(
        "build-emit-interface-input-path",
        "build with missing build input path",
        &normalized_stderr,
        &format!("note: failing package manifest: {manifest_display}"),
    )
    .expect("missing build input path should point to the failing package manifest");
    expect_stderr_contains(
        "build-emit-interface-input-path",
        "build with missing build input path",
        &normalized_stderr,
        &format!("note: failing build input path: {source_display}"),
    )
    .expect("missing build input path should point to the invalid source path");
    expect_stderr_contains(
        "build-emit-interface-input-path",
        "build with missing build input path",
        &normalized_stderr,
        &format!(
            "hint: rerun `ql build {} --emit llvm-ir --release --output {} --emit-interface` after fixing the build input path",
            source_display, output_display
        ),
    )
    .expect("missing build input path should preserve the build rerun options");
    expect_stderr_not_contains(
        "build-emit-interface-input-path",
        "build with missing build input path",
        &normalized_stderr,
        "note: build artifact remains at `",
    )
    .expect("missing build input path should not claim that a build artifact was preserved");
    assert!(
        !output_path.is_file(),
        "missing build input path should not create `{}`",
        output_path.display()
    );
    assert!(
        !interface_path.is_file(),
        "missing build input path should not create `{}`",
        interface_path.display()
    );
}

#[cfg(windows)]
#[test]
fn build_with_emit_interface_points_to_unreadable_build_input_path() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-build-emit-interface-input-read-failure");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create project source directory for unreadable build input test");
    let source_path = temp.write(
        "workspace/app/src/lib.ql",
        r#"
package demo.app

fn main() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"
version = "0.1.0"
"#,
    );

    let source_lock = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&source_path)
        .expect("open source file with an exclusive share mode");

    let manifest_path = project_root.join("qlang.toml");
    let output_path = project_root.join("build").join("app.ll");
    let interface_path = project_root.join("app.qi");
    let manifest_display = manifest_path.display().to_string().replace('\\', "/");
    let source_display = source_path.display().to_string().replace('\\', "/");
    let output_display = output_path.display().to_string().replace('\\', "/");

    let mut command = ql_command(&workspace_root);
    command
        .arg("build")
        .arg(&source_path)
        .args(["--emit", "llvm-ir", "--release", "--output"])
        .arg(&output_path)
        .arg("--emit-interface");
    let output = run_command_capture(
        &mut command,
        "`ql build --emit-interface` with an unreadable build input path",
    );
    drop(source_lock);

    let (stdout, stderr) = expect_exit_code(
        "build-emit-interface-input-read-failure",
        "build with unreadable build input path",
        &output,
        1,
    )
    .expect("build should fail when the build input file cannot be read");
    expect_snapshot_matches(
        "build-emit-interface-input-read-failure",
        "build with unreadable build input path stdout",
        "",
        &stdout,
    )
    .expect("unreadable build input path should not report a successful build artifact");
    expect_stderr_contains(
        "build-emit-interface-input-read-failure",
        "build with unreadable build input path",
        &stderr,
        "failed to access",
    )
    .expect("unreadable build input path should surface the access failure");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stderr_contains(
        "build-emit-interface-input-read-failure",
        "build with unreadable build input path",
        &normalized_stderr,
        &source_display,
    )
    .expect("unreadable build input path should surface the locked source path");
    expect_stderr_contains(
        "build-emit-interface-input-read-failure",
        "build with unreadable build input path",
        &normalized_stderr,
        &format!("note: failing package manifest: {manifest_display}"),
    )
    .expect("unreadable build input path should point to the failing package manifest");
    expect_stderr_contains(
        "build-emit-interface-input-read-failure",
        "build with unreadable build input path",
        &normalized_stderr,
        &format!("note: failing build input path: {source_display}"),
    )
    .expect("unreadable build input path should point to the unreadable source path");
    expect_stderr_contains(
        "build-emit-interface-input-read-failure",
        "build with unreadable build input path",
        &normalized_stderr,
        &format!(
            "hint: rerun `ql build {} --emit llvm-ir --release --output {} --emit-interface` after fixing the build input path",
            source_display, output_display
        ),
    )
    .expect("unreadable build input path should preserve the build rerun options");
    expect_stderr_not_contains(
        "build-emit-interface-input-read-failure",
        "build with unreadable build input path",
        &normalized_stderr,
        "note: build artifact remains at `",
    )
    .expect("unreadable build input path should not claim that a build artifact was preserved");
    assert!(
        !output_path.exists(),
        "unreadable build input path should not create `{}`",
        output_path.display()
    );
    assert!(
        !interface_path.is_file(),
        "unreadable build input path should not create `{}`",
        interface_path.display()
    );
}

#[test]
fn build_with_emit_interface_preserves_toolchain_rerun_hint() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-build-emit-interface-toolchain-failure");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create project source directory for build-side toolchain failure test");
    let source_path = temp.write(
        "workspace/app/src/lib.ql",
        r#"
package demo.app

extern "c" pub fn q_add(left: Int, right: Int) -> Int {
    return left + right
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"
version = "0.1.0"
"#,
    );

    let manifest_path = project_root.join("qlang.toml");
    let output_path = dynamic_library_output_path(&project_root.join("build"), "app");
    let header_path = project_root.join("include").join("app.h");
    let interface_path = project_root.join("app.qi");
    let manifest_display = manifest_path.display().to_string().replace('\\', "/");
    let source_display = source_path.display().to_string().replace('\\', "/");
    let output_display = output_path.display().to_string().replace('\\', "/");
    let header_display = header_path.display().to_string().replace('\\', "/");
    let clang_path = write_mock_clang_failure_script(&temp);

    let mut command = ql_command(&workspace_root);
    command
        .env("QLANG_CLANG", &clang_path)
        .arg("build")
        .arg(&source_path)
        .args(["--emit", "dylib", "--release", "--output"])
        .arg(&output_path)
        .args(["--header-surface", "both", "--header-output"])
        .arg(&header_path)
        .arg("--emit-interface");
    let output = run_command_capture(
        &mut command,
        "`ql build --emit-interface` with build-side toolchain failure",
    );
    let (stdout, stderr) = expect_exit_code(
        "build-emit-interface-toolchain-failure",
        "build with toolchain failure before interface emission",
        &output,
        1,
    )
    .expect("build should fail when the configured toolchain fails");
    expect_snapshot_matches(
        "build-emit-interface-toolchain-failure",
        "build with toolchain failure stdout",
        "",
        &stdout,
    )
    .expect("build-side toolchain failure should not report a successful build artifact");
    expect_stderr_contains(
        "build-emit-interface-toolchain-failure",
        "build with toolchain failure before interface emission",
        &stderr,
        "mock clang failure",
    )
    .expect("build-side toolchain failure should surface the toolchain stderr");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stderr_contains(
        "build-emit-interface-toolchain-failure",
        "build with toolchain failure before interface emission",
        &normalized_stderr,
        "note: preserved intermediate artifact at `",
    )
    .expect("build-side toolchain failure should report preserved intermediate artifacts");
    expect_stderr_contains(
        "build-emit-interface-toolchain-failure",
        "build with toolchain failure before interface emission",
        &normalized_stderr,
        ".codegen.ll",
    )
    .expect("build-side toolchain failure should preserve intermediate LLVM IR");
    expect_stderr_contains(
        "build-emit-interface-toolchain-failure",
        "build with toolchain failure before interface emission",
        &normalized_stderr,
        &format!("note: failing package manifest: {manifest_display}"),
    )
    .expect("build-side toolchain failure should point to the failing package manifest");
    expect_stderr_contains(
        "build-emit-interface-toolchain-failure",
        "build with toolchain failure before interface emission",
        &normalized_stderr,
        &format!(
            "hint: rerun `ql build {} --emit dylib --release --output {} --header-surface both --header-output {} --emit-interface` after fixing the build toolchain",
            source_display, output_display, header_display
        ),
    )
    .expect("build-side toolchain failure should preserve the build rerun options");
    expect_stderr_not_contains(
        "build-emit-interface-toolchain-failure",
        "build with toolchain failure before interface emission",
        &normalized_stderr,
        "note: build artifact remains at `",
    )
    .expect("build-side toolchain failure should not claim that a build artifact was preserved");
    assert!(
        !output_path.is_file(),
        "build-side toolchain failure should not create `{}`",
        output_path.display()
    );
    assert!(
        !header_path.is_file(),
        "build-side toolchain failure should not create `{}`",
        header_path.display()
    );
    assert!(
        !interface_path.is_file(),
        "build-side toolchain failure should not create `{}`",
        interface_path.display()
    );
}

#[test]
fn build_with_emit_interface_points_toolchain_output_failures_at_build_output_path() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-build-emit-interface-toolchain-output-path-failure");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create project source directory for toolchain output-path failure test");
    let source_path = temp.write(
        "workspace/app/src/lib.ql",
        r#"
package demo.app

fn main() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"
version = "0.1.0"
"#,
    );

    let manifest_path = project_root.join("qlang.toml");
    let output_path = project_root.join("build").join("app.obj");
    let interface_path = project_root.join("app.qi");
    let manifest_display = manifest_path.display().to_string().replace('\\', "/");
    let source_display = source_path.display().to_string().replace('\\', "/");
    let output_display = output_path.display().to_string().replace('\\', "/");
    let output_file_name = output_path
        .file_name()
        .and_then(|name| name.to_str())
        .expect("object output should have a UTF-8 filename");
    let clang_path = write_mock_clang_output_path_failure_script(&temp);

    let mut command = ql_command(&workspace_root);
    command
        .env("QLANG_CLANG", &clang_path)
        .arg("build")
        .arg(&source_path)
        .args(["--emit", "obj", "--release", "--output"])
        .arg(&output_path)
        .arg("--emit-interface");
    let output = run_command_capture(
        &mut command,
        "`ql build --emit-interface` with a toolchain output-path failure",
    );
    let (stdout, stderr) = expect_exit_code(
        "build-emit-interface-toolchain-output-path-failure",
        "build with toolchain output-path failure",
        &output,
        1,
    )
    .expect("build should fail when the toolchain cannot open the requested output path");
    expect_snapshot_matches(
        "build-emit-interface-toolchain-output-path-failure",
        "build with toolchain output-path failure stdout",
        "",
        &stdout,
    )
    .expect("toolchain output-path failure should not report a successful build artifact");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stderr_contains(
        "build-emit-interface-toolchain-output-path-failure",
        "build with toolchain output-path failure",
        &normalized_stderr,
        "unable to open output file '",
    )
    .expect("toolchain output-path failure should surface the blocked output file");
    expect_stderr_contains(
        "build-emit-interface-toolchain-output-path-failure",
        "build with toolchain output-path failure",
        &normalized_stderr,
        output_file_name,
    )
    .expect("toolchain output-path failure should still mention the requested output filename");
    expect_stderr_contains(
        "build-emit-interface-toolchain-output-path-failure",
        "build with toolchain output-path failure",
        &normalized_stderr,
        "Permission denied",
    )
    .expect("toolchain output-path failure should keep the output-path access reason");
    expect_stderr_contains(
        "build-emit-interface-toolchain-output-path-failure",
        "build with toolchain output-path failure",
        &normalized_stderr,
        "note: preserved intermediate artifact at `",
    )
    .expect("toolchain output-path failure should still report preserved intermediate artifacts");
    expect_stderr_contains(
        "build-emit-interface-toolchain-output-path-failure",
        "build with toolchain output-path failure",
        &normalized_stderr,
        ".codegen.ll",
    )
    .expect("toolchain output-path failure should preserve intermediate LLVM IR");
    expect_stderr_contains(
        "build-emit-interface-toolchain-output-path-failure",
        "build with toolchain output-path failure",
        &normalized_stderr,
        &format!("note: failing package manifest: {manifest_display}"),
    )
    .expect("toolchain output-path failure should point to the failing package manifest");
    expect_stderr_contains(
        "build-emit-interface-toolchain-output-path-failure",
        "build with toolchain output-path failure",
        &normalized_stderr,
        &format!("note: failing build output path: {output_display}"),
    )
    .expect("toolchain output-path failure should point to the requested build artifact path");
    expect_stderr_contains(
        "build-emit-interface-toolchain-output-path-failure",
        "build with toolchain output-path failure",
        &normalized_stderr,
        &format!(
            "hint: rerun `ql build {} --emit obj --release --output {} --emit-interface` after fixing the build output path",
            source_display, output_display
        ),
    )
    .expect("toolchain output-path failure should preserve the build rerun options");
    expect_stderr_not_contains(
        "build-emit-interface-toolchain-output-path-failure",
        "build with toolchain output-path failure",
        &normalized_stderr,
        &format!(
            "hint: rerun `ql build {} --emit obj --release --output {} --emit-interface` after fixing the build toolchain",
            source_display, output_display
        ),
    )
    .expect("toolchain output-path failure should not reuse the generic build-toolchain hint");
    expect_stderr_not_contains(
        "build-emit-interface-toolchain-output-path-failure",
        "build with toolchain output-path failure",
        &normalized_stderr,
        "note: build artifact remains at `",
    )
    .expect("toolchain output-path failure should not claim that a build artifact was preserved");
    assert!(
        !output_path.is_file(),
        "toolchain output-path failure should not create `{}`",
        output_path.display()
    );
    assert!(
        !interface_path.is_file(),
        "toolchain output-path failure should not create `{}`",
        interface_path.display()
    );
    let preserved_ir = std::fs::read_dir(
        output_path
            .parent()
            .expect("object output should have a parent"),
    )
    .expect("read preserved intermediate directory after toolchain output-path failure")
    .filter_map(Result::ok)
    .map(|entry| entry.path())
    .any(|path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.contains(".codegen.ll"))
    });
    assert!(
        preserved_ir,
        "toolchain output-path failure should preserve an intermediate LLVM IR file near `{}`",
        output_path.display()
    );
}

#[test]
fn build_with_emit_interface_points_archiver_output_failures_at_build_output_path() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-build-emit-interface-archiver-output-path-failure");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create project source directory for archiver output-path failure test");
    let source_path = temp.write(
        "workspace/app/src/lib.ql",
        r#"
package demo.app

extern "c" pub fn q_add(left: Int, right: Int) -> Int {
    return left + right
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"
version = "0.1.0"
"#,
    );

    let manifest_path = project_root.join("qlang.toml");
    let output_path = static_library_output_path(&project_root.join("build"), "app");
    let interface_path = project_root.join("app.qi");
    let manifest_display = manifest_path.display().to_string().replace('\\', "/");
    let source_display = source_path.display().to_string().replace('\\', "/");
    let output_display = output_path.display().to_string().replace('\\', "/");
    let output_file_name = output_path
        .file_name()
        .and_then(|name| name.to_str())
        .expect("static library output should have a UTF-8 filename");
    let clang_path = write_mock_clang_success_script(&temp);
    let archiver_path = write_mock_archiver_output_path_failure_script(&temp);

    let mut command = ql_command(&workspace_root);
    command
        .env("QLANG_CLANG", &clang_path)
        .env("QLANG_AR", &archiver_path)
        .env("QLANG_AR_STYLE", "lib")
        .arg("build")
        .arg(&source_path)
        .args(["--emit", "staticlib", "--release", "--output"])
        .arg(&output_path)
        .arg("--emit-interface");
    let output = run_command_capture(
        &mut command,
        "`ql build --emit-interface` with an archiver output-path failure",
    );
    let (stdout, stderr) = expect_exit_code(
        "build-emit-interface-archiver-output-path-failure",
        "build with archiver output-path failure",
        &output,
        1,
    )
    .expect("build should fail when the archiver cannot open the requested output path");
    expect_snapshot_matches(
        "build-emit-interface-archiver-output-path-failure",
        "build with archiver output-path failure stdout",
        "",
        &stdout,
    )
    .expect("archiver output-path failure should not report a successful build artifact");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stderr_contains(
        "build-emit-interface-archiver-output-path-failure",
        "build with archiver output-path failure",
        &normalized_stderr,
        "cannot open file '",
    )
    .expect("archiver output-path failure should surface the blocked archive file");
    expect_stderr_contains(
        "build-emit-interface-archiver-output-path-failure",
        "build with archiver output-path failure",
        &normalized_stderr,
        output_file_name,
    )
    .expect("archiver output-path failure should still mention the requested archive filename");
    expect_stderr_contains(
        "build-emit-interface-archiver-output-path-failure",
        "build with archiver output-path failure",
        &normalized_stderr,
        "Permission denied",
    )
    .expect("archiver output-path failure should keep the output-path access reason");
    expect_stderr_contains(
        "build-emit-interface-archiver-output-path-failure",
        "build with archiver output-path failure",
        &normalized_stderr,
        "note: preserved intermediate artifact at `",
    )
    .expect("archiver output-path failure should still report preserved intermediate artifacts");
    expect_stderr_contains(
        "build-emit-interface-archiver-output-path-failure",
        "build with archiver output-path failure",
        &normalized_stderr,
        ".codegen.obj",
    )
    .expect("archiver output-path failure should preserve the intermediate object");
    expect_stderr_contains(
        "build-emit-interface-archiver-output-path-failure",
        "build with archiver output-path failure",
        &normalized_stderr,
        &format!("note: failing package manifest: {manifest_display}"),
    )
    .expect("archiver output-path failure should point to the failing package manifest");
    expect_stderr_contains(
        "build-emit-interface-archiver-output-path-failure",
        "build with archiver output-path failure",
        &normalized_stderr,
        &format!("note: failing build output path: {output_display}"),
    )
    .expect("archiver output-path failure should point to the requested archive path");
    expect_stderr_contains(
        "build-emit-interface-archiver-output-path-failure",
        "build with archiver output-path failure",
        &normalized_stderr,
        &format!(
            "hint: rerun `ql build {} --emit staticlib --release --output {} --emit-interface` after fixing the build output path",
            source_display, output_display
        ),
    )
    .expect("archiver output-path failure should preserve the build rerun options");
    expect_stderr_not_contains(
        "build-emit-interface-archiver-output-path-failure",
        "build with archiver output-path failure",
        &normalized_stderr,
        &format!(
            "hint: rerun `ql build {} --emit staticlib --release --output {} --emit-interface` after fixing the build toolchain",
            source_display, output_display
        ),
    )
    .expect("archiver output-path failure should not reuse the generic build-toolchain hint");
    expect_stderr_not_contains(
        "build-emit-interface-archiver-output-path-failure",
        "build with archiver output-path failure",
        &normalized_stderr,
        "note: build artifact remains at `",
    )
    .expect("archiver output-path failure should not claim that a build artifact was preserved");
    assert!(
        !output_path.is_file(),
        "archiver output-path failure should not create `{}`",
        output_path.display()
    );
    assert!(
        !interface_path.is_file(),
        "archiver output-path failure should not create `{}`",
        interface_path.display()
    );
    let preserved_obj = std::fs::read_dir(
        output_path
            .parent()
            .expect("static library output should have a parent"),
    )
    .expect("read preserved intermediate directory after archiver output-path failure")
    .filter_map(Result::ok)
    .map(|entry| entry.path())
    .any(|path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.contains(".codegen.obj"))
    });
    assert!(
        preserved_obj,
        "archiver output-path failure should preserve an intermediate object near `{}`",
        output_path.display()
    );
}

#[test]
fn build_with_emit_interface_points_to_blocked_build_output_path() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-build-emit-interface-build-output-path-failure");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create project source directory for blocked build-output test");
    let source_path = temp.write(
        "workspace/app/src/lib.ql",
        r#"
package demo.app

pub fn exported(value: Int) -> Int {
    return value
}

fn main() -> Int {
    return exported(1)
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"
version = "0.1.0"
"#,
    );

    let manifest_path = project_root.join("qlang.toml");
    let output_path = project_root.join("build").join("blocked.ll");
    std::fs::create_dir_all(&output_path)
        .expect("create blocking directory at the build output path");
    let interface_path = project_root.join("app.qi");
    let manifest_display = manifest_path.display().to_string().replace('\\', "/");
    let source_display = source_path.display().to_string().replace('\\', "/");
    let output_display = output_path.display().to_string().replace('\\', "/");

    let mut command = ql_command(&workspace_root);
    command
        .arg("build")
        .arg(&source_path)
        .args(["--emit", "llvm-ir", "--release", "--output"])
        .arg(&output_path)
        .arg("--emit-interface");
    let output = run_command_capture(
        &mut command,
        "`ql build --emit-interface` with blocked build output path",
    );
    let (stdout, stderr) = expect_exit_code(
        "build-emit-interface-build-output-path-failure",
        "build with blocked build output path",
        &output,
        1,
    )
    .expect("build should fail when the primary build output path is blocked");
    expect_snapshot_matches(
        "build-emit-interface-build-output-path-failure",
        "build with blocked build output path stdout",
        "",
        &stdout,
    )
    .expect("blocked build output path should not report a successful build artifact");
    expect_stderr_contains(
        "build-emit-interface-build-output-path-failure",
        "build with blocked build output path",
        &stderr,
        "failed to access",
    )
    .expect("blocked build output path should surface the access failure");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stderr_contains(
        "build-emit-interface-build-output-path-failure",
        "build with blocked build output path",
        &normalized_stderr,
        &format!("note: failing package manifest: {manifest_display}"),
    )
    .expect("blocked build output path should point to the failing package manifest");
    expect_stderr_contains(
        "build-emit-interface-build-output-path-failure",
        "build with blocked build output path",
        &normalized_stderr,
        &format!("note: failing build output path: {output_display}"),
    )
    .expect("blocked build output path should point to the blocked build artifact path");
    expect_stderr_contains(
        "build-emit-interface-build-output-path-failure",
        "build with blocked build output path",
        &normalized_stderr,
        &format!(
            "hint: rerun `ql build {} --emit llvm-ir --release --output {} --emit-interface` after fixing the build output path",
            source_display, output_display
        ),
    )
    .expect("blocked build output path should preserve the build rerun options");
    expect_stderr_not_contains(
        "build-emit-interface-build-output-path-failure",
        "build with blocked build output path",
        &normalized_stderr,
        "note: build artifact remains at `",
    )
    .expect("blocked build output path should not claim that a build artifact was preserved");
    assert!(
        output_path.is_dir(),
        "blocked build output path test should preserve `{}` as a directory",
        output_path.display()
    );
    assert!(
        !interface_path.is_file(),
        "blocked build output path should not create `{}`",
        interface_path.display()
    );
}

#[test]
fn build_with_emit_interface_points_to_blocked_build_output_parent_path() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-build-emit-interface-build-output-parent-path-failure");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create project source directory for blocked build-output parent test");
    let source_path = temp.write(
        "workspace/app/src/lib.ql",
        r#"
package demo.app

fn main() -> Int {
    return 1
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"
version = "0.1.0"
"#,
    );

    let manifest_path = project_root.join("qlang.toml");
    let blocked_parent = project_root.join("blocked");
    temp.write("workspace/app/blocked", "not-a-directory");
    let output_path = blocked_parent.join("app.ll");
    let interface_path = project_root.join("app.qi");
    let manifest_display = manifest_path.display().to_string().replace('\\', "/");
    let source_display = source_path.display().to_string().replace('\\', "/");
    let output_display = output_path.display().to_string().replace('\\', "/");
    let parent_display = blocked_parent.display().to_string().replace('\\', "/");

    let mut command = ql_command(&workspace_root);
    command
        .arg("build")
        .arg(&source_path)
        .args(["--emit", "llvm-ir", "--release", "--output"])
        .arg(&output_path)
        .arg("--emit-interface");
    let output = run_command_capture(
        &mut command,
        "`ql build --emit-interface` with blocked build output parent path",
    );
    let (stdout, stderr) = expect_exit_code(
        "build-emit-interface-build-output-parent-path-failure",
        "build with blocked build output parent path",
        &output,
        1,
    )
    .expect("build should fail when the build output parent path is blocked");
    expect_snapshot_matches(
        "build-emit-interface-build-output-parent-path-failure",
        "build with blocked build output parent path stdout",
        "",
        &stdout,
    )
    .expect("blocked build output parent path should not report a successful build artifact");
    expect_stderr_contains(
        "build-emit-interface-build-output-parent-path-failure",
        "build with blocked build output parent path",
        &stderr,
        "failed to access",
    )
    .expect("blocked build output parent path should surface the access failure");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stderr_contains(
        "build-emit-interface-build-output-parent-path-failure",
        "build with blocked build output parent path",
        &normalized_stderr,
        &parent_display,
    )
    .expect("blocked build output parent path should surface the blocked parent path");
    expect_stderr_contains(
        "build-emit-interface-build-output-parent-path-failure",
        "build with blocked build output parent path",
        &normalized_stderr,
        &format!("note: failing package manifest: {manifest_display}"),
    )
    .expect("blocked build output parent path should point to the failing package manifest");
    expect_stderr_contains(
        "build-emit-interface-build-output-parent-path-failure",
        "build with blocked build output parent path",
        &normalized_stderr,
        &format!("note: failing build output path: {output_display}"),
    )
    .expect(
        "blocked build output parent path should still point to the requested build artifact path",
    );
    expect_stderr_contains(
        "build-emit-interface-build-output-parent-path-failure",
        "build with blocked build output parent path",
        &normalized_stderr,
        &format!(
            "hint: rerun `ql build {} --emit llvm-ir --release --output {} --emit-interface` after fixing the build output path",
            source_display, output_display
        ),
    )
    .expect("blocked build output parent path should preserve the build rerun options");
    expect_stderr_not_contains(
        "build-emit-interface-build-output-parent-path-failure",
        "build with blocked build output parent path",
        &normalized_stderr,
        "note: build artifact remains at `",
    )
    .expect(
        "blocked build output parent path should not claim that a build artifact was preserved",
    );
    assert!(
        blocked_parent.is_file(),
        "blocked build output parent path test should preserve `{}` as a file",
        blocked_parent.display()
    );
    assert!(
        !output_path.exists(),
        "blocked build output parent path should not create `{}`",
        output_path.display()
    );
    assert!(
        !interface_path.is_file(),
        "blocked build output parent path should not create `{}`",
        interface_path.display()
    );
}

#[test]
fn build_with_emit_interface_points_to_blocked_build_header_output_path() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-build-emit-interface-build-header-output-path-failure");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create project source directory for blocked build-header test");
    let source_path = temp.write(
        "workspace/app/src/lib.ql",
        r#"
package demo.app

extern "c" pub fn q_add(left: Int, right: Int) -> Int {
    return left + right
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"
version = "0.1.0"
"#,
    );

    let manifest_path = project_root.join("qlang.toml");
    let output_path = dynamic_library_output_path(&project_root.join("build"), "app");
    let header_path = project_root.join("include").join("app.h");
    std::fs::create_dir_all(&header_path)
        .expect("create blocking directory at the build header output path");
    let interface_path = project_root.join("app.qi");
    let manifest_display = manifest_path.display().to_string().replace('\\', "/");
    let source_display = source_path.display().to_string().replace('\\', "/");
    let output_display = output_path.display().to_string().replace('\\', "/");
    let header_display = header_path.display().to_string().replace('\\', "/");
    let clang_path = write_mock_clang_success_script(&temp);

    let mut command = ql_command(&workspace_root);
    command
        .env("QLANG_CLANG", &clang_path)
        .arg("build")
        .arg(&source_path)
        .args(["--emit", "dylib", "--release", "--output"])
        .arg(&output_path)
        .args(["--header-surface", "both", "--header-output"])
        .arg(&header_path)
        .arg("--emit-interface");
    let output = run_command_capture(
        &mut command,
        "`ql build --emit-interface` with blocked build header output path",
    );
    let (stdout, stderr) = expect_exit_code(
        "build-emit-interface-build-header-output-path-failure",
        "build with blocked build header output path",
        &output,
        1,
    )
    .expect("build should fail when the build header output path is blocked");
    expect_snapshot_matches(
        "build-emit-interface-build-header-output-path-failure",
        "build with blocked build header output path stdout",
        "",
        &stdout,
    )
    .expect("blocked build header output path should not report a successful build artifact");
    expect_stderr_contains(
        "build-emit-interface-build-header-output-path-failure",
        "build with blocked build header output path",
        &stderr,
        "failed to access",
    )
    .expect("blocked build header output path should surface the access failure");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stderr_contains(
        "build-emit-interface-build-header-output-path-failure",
        "build with blocked build header output path",
        &normalized_stderr,
        &format!("note: failing package manifest: {manifest_display}"),
    )
    .expect("blocked build header output path should point to the failing package manifest");
    expect_stderr_contains(
        "build-emit-interface-build-header-output-path-failure",
        "build with blocked build header output path",
        &normalized_stderr,
        &format!("note: failing build header output path: {header_display}"),
    )
    .expect("blocked build header output path should point to the blocked header target");
    expect_stderr_contains(
        "build-emit-interface-build-header-output-path-failure",
        "build with blocked build header output path",
        &normalized_stderr,
        &format!(
            "hint: rerun `ql build {} --emit dylib --release --output {} --header-surface both --header-output {} --emit-interface` after fixing the build header output path",
            source_display, output_display, header_display
        ),
    )
    .expect("blocked build header output path should preserve the build rerun options");
    expect_stderr_not_contains(
        "build-emit-interface-build-header-output-path-failure",
        "build with blocked build header output path",
        &normalized_stderr,
        "note: build artifact remains at `",
    )
    .expect(
        "blocked build header output path should not claim that a build artifact was preserved",
    );
    expect_stderr_not_contains(
        "build-emit-interface-build-header-output-path-failure",
        "build with blocked build header output path",
        &normalized_stderr,
        "note: failing build output path:",
    )
    .expect("blocked build header output path should not be mislabeled as the primary build output path");
    assert!(
        !output_path.is_file(),
        "blocked build header output path should remove `{}` after header failure",
        output_path.display()
    );
    assert!(
        header_path.is_dir(),
        "blocked build header output path test should preserve `{}` as a directory",
        header_path.display()
    );
    assert!(
        !interface_path.is_file(),
        "blocked build header output path should not create `{}`",
        interface_path.display()
    );
}

#[test]
fn build_with_emit_interface_points_to_blocked_build_header_output_parent_path() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-build-emit-interface-build-header-output-parent-path-failure");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create project source directory for blocked build-header parent test");
    let source_path = temp.write(
        "workspace/app/src/lib.ql",
        r#"
package demo.app

extern "c" pub fn q_add(left: Int, right: Int) -> Int {
    return left + right
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"
version = "0.1.0"
"#,
    );

    let manifest_path = project_root.join("qlang.toml");
    let output_path = dynamic_library_output_path(&project_root.join("build"), "app");
    let blocked_parent = project_root.join("blocked-include");
    temp.write("workspace/app/blocked-include", "not-a-directory");
    let header_path = blocked_parent.join("app.h");
    let interface_path = project_root.join("app.qi");
    let manifest_display = manifest_path.display().to_string().replace('\\', "/");
    let source_display = source_path.display().to_string().replace('\\', "/");
    let output_display = output_path.display().to_string().replace('\\', "/");
    let header_display = header_path.display().to_string().replace('\\', "/");
    let parent_display = blocked_parent.display().to_string().replace('\\', "/");
    let clang_path = write_mock_clang_success_script(&temp);

    let mut command = ql_command(&workspace_root);
    command
        .env("QLANG_CLANG", &clang_path)
        .arg("build")
        .arg(&source_path)
        .args(["--emit", "dylib", "--release", "--output"])
        .arg(&output_path)
        .args(["--header-surface", "both", "--header-output"])
        .arg(&header_path)
        .arg("--emit-interface");
    let output = run_command_capture(
        &mut command,
        "`ql build --emit-interface` with blocked build header output parent path",
    );
    let (stdout, stderr) = expect_exit_code(
        "build-emit-interface-build-header-output-parent-path-failure",
        "build with blocked build header output parent path",
        &output,
        1,
    )
    .expect("build should fail when the build header output parent path is blocked");
    expect_snapshot_matches(
        "build-emit-interface-build-header-output-parent-path-failure",
        "build with blocked build header output parent path stdout",
        "",
        &stdout,
    )
    .expect(
        "blocked build header output parent path should not report a successful build artifact",
    );
    expect_stderr_contains(
        "build-emit-interface-build-header-output-parent-path-failure",
        "build with blocked build header output parent path",
        &stderr,
        "failed to access",
    )
    .expect("blocked build header output parent path should surface the access failure");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stderr_contains(
        "build-emit-interface-build-header-output-parent-path-failure",
        "build with blocked build header output parent path",
        &normalized_stderr,
        &parent_display,
    )
    .expect("blocked build header output parent path should surface the blocked parent path");
    expect_stderr_contains(
        "build-emit-interface-build-header-output-parent-path-failure",
        "build with blocked build header output parent path",
        &normalized_stderr,
        &format!("note: failing package manifest: {manifest_display}"),
    )
    .expect("blocked build header output parent path should point to the failing package manifest");
    expect_stderr_contains(
        "build-emit-interface-build-header-output-parent-path-failure",
        "build with blocked build header output parent path",
        &normalized_stderr,
        &format!("note: failing build header output path: {header_display}"),
    )
    .expect(
        "blocked build header output parent path should still point to the requested header path",
    );
    expect_stderr_contains(
        "build-emit-interface-build-header-output-parent-path-failure",
        "build with blocked build header output parent path",
        &normalized_stderr,
        &format!(
            "hint: rerun `ql build {} --emit dylib --release --output {} --header-surface both --header-output {} --emit-interface` after fixing the build header output path",
            source_display, output_display, header_display
        ),
    )
    .expect("blocked build header output parent path should preserve the build rerun options");
    expect_stderr_not_contains(
        "build-emit-interface-build-header-output-parent-path-failure",
        "build with blocked build header output parent path",
        &normalized_stderr,
        "note: build artifact remains at `",
    )
    .expect(
        "blocked build header output parent path should not claim that a build artifact was preserved",
    );
    expect_stderr_not_contains(
        "build-emit-interface-build-header-output-parent-path-failure",
        "build with blocked build header output parent path",
        &normalized_stderr,
        "note: failing build output path:",
    )
    .expect(
        "blocked build header output parent path should not be mislabeled as the primary build output path",
    );
    assert!(
        !output_path.is_file(),
        "blocked build header output parent path should remove `{}` after header failure",
        output_path.display()
    );
    assert!(
        blocked_parent.is_file(),
        "blocked build header output parent path test should preserve `{}` as a file",
        blocked_parent.display()
    );
    assert!(
        !header_path.exists(),
        "blocked build header output parent path should not create `{}`",
        header_path.display()
    );
    assert!(
        !interface_path.is_file(),
        "blocked build header output parent path should not create `{}`",
        interface_path.display()
    );
}

#[test]
fn build_with_emit_interface_points_to_colliding_build_header_output_path() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-build-emit-interface-build-header-output-collision");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create project source directory for header collision test");
    let source_path = temp.write(
        "workspace/app/src/lib.ql",
        r#"
package demo.app

extern "c" pub fn q_add(left: Int, right: Int) -> Int {
    return left + right
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"
version = "0.1.0"
"#,
    );

    let manifest_path = project_root.join("qlang.toml");
    let output_path = project_root.join("build").join("app.lib");
    let interface_path = project_root.join("app.qi");
    let manifest_display = manifest_path.display().to_string().replace('\\', "/");
    let source_display = source_path.display().to_string().replace('\\', "/");
    let output_display = output_path.display().to_string().replace('\\', "/");

    let mut command = ql_command(&workspace_root);
    command
        .arg("build")
        .arg(&source_path)
        .args(["--emit", "staticlib", "--release", "--output"])
        .arg(&output_path)
        .args(["--header-output"])
        .arg(&output_path)
        .arg("--emit-interface");
    let output = run_command_capture(
        &mut command,
        "`ql build --emit-interface` with colliding build header output path",
    );
    let (stdout, stderr) = expect_exit_code(
        "build-emit-interface-build-header-output-collision",
        "build with colliding build header output path",
        &output,
        1,
    )
    .expect(
        "build should fail when the build header output path collides with the primary artifact",
    );
    expect_snapshot_matches(
        "build-emit-interface-build-header-output-collision",
        "build with colliding build header output path stdout",
        "",
        &stdout,
    )
    .expect("colliding build header output path should not report a successful build artifact");
    expect_stderr_contains(
        "build-emit-interface-build-header-output-collision",
        "build with colliding build header output path",
        &stderr,
        "must differ from the primary artifact output",
    )
    .expect("colliding build header output path should surface the collision message");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stderr_contains(
        "build-emit-interface-build-header-output-collision",
        "build with colliding build header output path",
        &normalized_stderr,
        &format!("note: failing package manifest: {manifest_display}"),
    )
    .expect("colliding build header output path should point to the failing package manifest");
    expect_stderr_contains(
        "build-emit-interface-build-header-output-collision",
        "build with colliding build header output path",
        &normalized_stderr,
        &format!("note: failing build header output path: {output_display}"),
    )
    .expect("colliding build header output path should point to the colliding header target");
    expect_stderr_contains(
        "build-emit-interface-build-header-output-collision",
        "build with colliding build header output path",
        &normalized_stderr,
        &format!(
            "hint: rerun `ql build {} --emit staticlib --release --output {} --header-output {} --emit-interface` after fixing the build header output path",
            source_display, output_display, output_display
        ),
    )
    .expect("colliding build header output path should preserve the build rerun options");
    expect_stderr_not_contains(
        "build-emit-interface-build-header-output-collision",
        "build with colliding build header output path",
        &normalized_stderr,
        "note: build artifact remains at `",
    )
    .expect(
        "colliding build header output path should not claim that a build artifact was preserved",
    );
    assert!(
        !output_path.is_file(),
        "colliding build header output path should not create `{}`",
        output_path.display()
    );
    assert!(
        !interface_path.is_file(),
        "colliding build header output path should not create `{}`",
        interface_path.display()
    );
}

#[test]
fn build_with_emit_interface_points_to_build_side_package_manifest_failure() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-build-emit-interface-package-manifest");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create project source directory for build-side package manifest failure test");
    let source_path = temp.write(
        "workspace/app/src/lib.ql",
        r#"
pub fn exported(value: Int) -> Int {
    return value
}

fn main() -> Int {
    return exported(1)
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[workspace]
members = []
"#,
    );
    let output_path = project_root.join("build").join("app.ll");
    let manifest_path = project_root.join("qlang.toml");
    let manifest_display = manifest_path.to_string_lossy().replace('\\', "/");
    let source_display = source_path.to_string_lossy().replace('\\', "/");
    let output_display = output_path.to_string_lossy().replace('\\', "/");

    let mut command = ql_command(&workspace_root);
    command
        .arg("build")
        .arg(&source_path)
        .args(["--emit", "llvm-ir", "--output"])
        .arg(&output_path)
        .arg("--emit-interface");
    let output = run_command_capture(
        &mut command,
        "`ql build --emit-interface` with build-side package manifest failure",
    );
    let (stdout, stderr) = expect_exit_code(
        "build-emit-interface-package-manifest-failure",
        "build with missing package manifest metadata",
        &output,
        1,
    )
    .expect("build should fail when build-side interface emission cannot resolve package metadata");
    expect_stdout_contains_all(
        "build-emit-interface-package-manifest-failure",
        &stdout,
        &[&format!("wrote llvm-ir: {}", output_path.display())],
    )
    .expect(
        "build-side package manifest failure should still report the successful build artifact",
    );
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stderr_contains(
        "build-emit-interface-package-manifest-failure",
        "build with missing package manifest metadata",
        &normalized_stderr,
        &format!(
            "error: `ql build --emit-interface` manifest `{}` does not declare `[package].name`",
            manifest_display
        ),
    )
    .expect("build-side package manifest failure should preserve the build command label");
    expect_stderr_contains(
        "build-emit-interface-package-manifest-failure",
        "build with missing package manifest metadata",
        &normalized_stderr,
        &format!("note: failing package manifest: {}", manifest_display),
    )
    .expect("build-side package manifest failure should point to the failing manifest");
    expect_stderr_contains(
        "build-emit-interface-package-manifest-failure",
        "build with missing package manifest metadata",
        &normalized_stderr,
        &format!(
            "hint: rerun `ql build {} --emit llvm-ir --output {} --emit-interface` after fixing the package manifest",
            source_display, output_display
        ),
    )
    .expect("build-side package manifest failure should preserve the original build rerun options");
    expect_stderr_not_contains(
        "build-emit-interface-package-manifest-failure",
        "build with missing package manifest metadata",
        &normalized_stderr,
        "after fixing the package interface error",
    )
    .expect("build-side package manifest failure should not fall back to a generic interface-error hint");
    expect_stderr_contains(
        "build-emit-interface-package-manifest-failure",
        "build with missing package manifest metadata",
        &normalized_stderr,
        &format!("note: build artifact remains at `{}`", output_display),
    )
    .expect(
        "build-side package manifest failure should confirm that the build artifact was preserved",
    );
    expect_file_exists(
        "build-emit-interface-package-manifest-failure",
        &output_path,
        "generated llvm ir",
        "build with missing package manifest metadata",
    )
    .expect("build-side package manifest failure should keep the successful build artifact");
}

#[test]
fn build_with_emit_interface_points_to_build_side_package_source_root_failure() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-build-emit-interface-package-source-root");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(&project_root)
        .expect("create project root for build-side package source root failure test");
    let source_path = temp.write(
        "workspace/app/entry.ql",
        r#"
pub fn exported(value: Int) -> Int {
    return value
}

fn main() -> Int {
    return exported(1)
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    let output_path = project_root.join("build").join("app.ll");
    let manifest_path = project_root.join("qlang.toml");
    let source_root = project_root.join("src");
    let interface_path = project_root.join("app.qi");
    let manifest_display = manifest_path.to_string_lossy().replace('\\', "/");
    let source_root_display = source_root.to_string_lossy().replace('\\', "/");
    let source_display = source_path.to_string_lossy().replace('\\', "/");
    let output_display = output_path.to_string_lossy().replace('\\', "/");

    let mut command = ql_command(&workspace_root);
    command
        .arg("build")
        .arg(&source_path)
        .args(["--emit", "llvm-ir", "--output"])
        .arg(&output_path)
        .arg("--emit-interface");
    let output = run_command_capture(
        &mut command,
        "`ql build --emit-interface` with build-side package source root failure",
    );
    let (stdout, stderr) = expect_exit_code(
        "build-emit-interface-package-source-root-failure",
        "build with missing package source root",
        &output,
        1,
    )
    .expect(
        "build should fail when build-side interface emission cannot find the package source root",
    );
    expect_stdout_contains_all(
        "build-emit-interface-package-source-root-failure",
        &stdout,
        &[&format!("wrote llvm-ir: {}", output_path.display())],
    )
    .expect(
        "build-side package source root failure should still report the successful build artifact",
    );
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stderr_contains(
        "build-emit-interface-package-source-root-failure",
        "build with missing package source root",
        &normalized_stderr,
        &format!(
            "error: `ql build --emit-interface` package source directory `{}` does not exist",
            source_root_display
        ),
    )
    .expect("build-side package source root failure should preserve the build command label");
    expect_stderr_contains(
        "build-emit-interface-package-source-root-failure",
        "build with missing package source root",
        &normalized_stderr,
        &format!("note: failing package manifest: {}", manifest_display),
    )
    .expect("build-side package source root failure should point to the failing manifest");
    expect_stderr_contains(
        "build-emit-interface-package-source-root-failure",
        "build with missing package source root",
        &normalized_stderr,
        &format!("note: failing package source root: {}", source_root_display),
    )
    .expect("build-side package source root failure should point to the missing source root");
    expect_stderr_contains(
        "build-emit-interface-package-source-root-failure",
        "build with missing package source root",
        &normalized_stderr,
        &format!(
            "hint: rerun `ql build {} --emit llvm-ir --output {} --emit-interface` after fixing the package source root",
            source_display, output_display
        ),
    )
    .expect("build-side package source root failure should preserve the original build rerun options");
    expect_stderr_not_contains(
        "build-emit-interface-package-source-root-failure",
        "build with missing package source root",
        &normalized_stderr,
        "after fixing the package interface error",
    )
    .expect("build-side package source root failure should not fall back to a generic interface-error hint");
    expect_stderr_contains(
        "build-emit-interface-package-source-root-failure",
        "build with missing package source root",
        &normalized_stderr,
        &format!("note: build artifact remains at `{}`", output_display),
    )
    .expect("build-side package source root failure should confirm that the build artifact was preserved");
    expect_file_exists(
        "build-emit-interface-package-source-root-failure",
        &output_path,
        "generated llvm ir",
        "build with missing package source root",
    )
    .expect("build-side package source root failure should keep the successful build artifact");
    assert!(
        !interface_path.is_file(),
        "build-side package source root failure should not leave behind a partial interface artifact"
    );
}

#[test]
fn build_with_emit_interface_points_to_empty_build_side_package_source_root() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-build-emit-interface-empty-package-source-root");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create empty package source root for build-side empty-source-root test");
    let source_path = temp.write(
        "workspace/app/entry.ql",
        r#"
pub fn exported(value: Int) -> Int {
    return value
}

fn main() -> Int {
    return exported(1)
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    let output_path = project_root.join("build").join("app.ll");
    let manifest_path = project_root.join("qlang.toml");
    let source_root = project_root.join("src");
    let interface_path = project_root.join("app.qi");
    let manifest_display = manifest_path.to_string_lossy().replace('\\', "/");
    let source_root_display = source_root.to_string_lossy().replace('\\', "/");
    let source_display = source_path.to_string_lossy().replace('\\', "/");
    let output_display = output_path.to_string_lossy().replace('\\', "/");

    let mut command = ql_command(&workspace_root);
    command
        .arg("build")
        .arg(&source_path)
        .args(["--emit", "llvm-ir", "--output"])
        .arg(&output_path)
        .arg("--emit-interface");
    let output = run_command_capture(
        &mut command,
        "`ql build --emit-interface` with empty build-side package source root",
    );
    let (stdout, stderr) = expect_exit_code(
        "build-emit-interface-empty-package-source-root",
        "build with empty package source root",
        &output,
        1,
    )
    .expect(
        "build should fail when build-side interface emission sees an empty package source root",
    );
    expect_stdout_contains_all(
        "build-emit-interface-empty-package-source-root",
        &stdout,
        &[&format!("wrote llvm-ir: {}", output_path.display())],
    )
    .expect(
        "build-side empty package source root should still report the successful build artifact",
    );
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stderr_contains(
        "build-emit-interface-empty-package-source-root",
        "build with empty package source root",
        &normalized_stderr,
        &format!(
            "error: `ql build --emit-interface` no `.ql` files found under `{}`",
            source_root_display
        ),
    )
    .expect("build-side empty package source root should be reported as a no-source failure");
    expect_stderr_contains(
        "build-emit-interface-empty-package-source-root",
        "build with empty package source root",
        &normalized_stderr,
        &format!("note: failing package manifest: {}", manifest_display),
    )
    .expect("build-side empty package source root should point to the failing manifest");
    expect_stderr_contains(
        "build-emit-interface-empty-package-source-root",
        "build with empty package source root",
        &normalized_stderr,
        &format!("note: failing package source root: {}", source_root_display),
    )
    .expect("build-side empty package source root should point to the empty source root");
    expect_stderr_contains(
        "build-emit-interface-empty-package-source-root",
        "build with empty package source root",
        &normalized_stderr,
        &format!(
            "hint: rerun `ql build {} --emit llvm-ir --output {} --emit-interface` after adding package source files",
            source_display, output_display
        ),
    )
    .expect("build-side empty package source root should preserve the original build rerun options");
    expect_stderr_not_contains(
        "build-emit-interface-empty-package-source-root",
        "build with empty package source root",
        &normalized_stderr,
        "after fixing the package interface error",
    )
    .expect("build-side empty package source root should not fall back to a generic interface-error hint");
    expect_stderr_contains(
        "build-emit-interface-empty-package-source-root",
        "build with empty package source root",
        &normalized_stderr,
        &format!("note: build artifact remains at `{}`", output_display),
    )
    .expect(
        "build-side empty package source root should confirm that the build artifact was preserved",
    );
    expect_file_exists(
        "build-emit-interface-empty-package-source-root",
        &output_path,
        "generated llvm ir",
        "build with empty package source root",
    )
    .expect("build-side empty package source root should keep the successful build artifact");
    assert!(
        !interface_path.is_file(),
        "build-side empty package source root should not leave behind a partial interface artifact"
    );
}

#[test]
fn build_with_emit_interface_points_to_failing_package_manifest() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-build-emit-interface-failure");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create project source directory for build-side interface failure test");
    let source_path = temp.write(
        "workspace/app/src/lib.ql",
        r#"
package demo.app

pub fn exported(value: Int) -> Int {
    return value
}

fn main() -> Int {
    return exported(1)
}
"#,
    );
    let first_failure = temp.write(
        "workspace/app/src/a_broken.ql",
        r#"
package demo.app

pub fn broken_first(value: MissingFirst) -> Int {
    return value
}
"#,
    );
    temp.write(
        "workspace/app/src/z_broken.ql",
        r#"
package demo.app

pub fn broken_second(value: MissingSecond) -> Int {
    return value
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    let output_path = project_root.join("build").join("app.ll");
    let manifest_path = project_root.join("qlang.toml");
    let interface_path = project_root.join("app.qi");
    let manifest_display = manifest_path.to_string_lossy().replace('\\', "/");
    let source_display = source_path.to_string_lossy().replace('\\', "/");
    let output_display = output_path.to_string_lossy().replace('\\', "/");
    let first_failure_display = first_failure.to_string_lossy().replace('\\', "/");

    let mut command = ql_command(&workspace_root);
    command
        .arg("build")
        .arg(&source_path)
        .args(["--emit", "llvm-ir", "--output"])
        .arg(&output_path)
        .arg("--emit-interface");
    let output = run_command_capture(
        &mut command,
        "`ql build --emit-interface` with failing package interface emission",
    );
    let (stdout, stderr) = expect_exit_code(
        "build-emit-interface-failure",
        "build with failing interface emission",
        &output,
        1,
    )
    .expect("build should fail when package interface emission fails");
    expect_stdout_contains_all(
        "build-emit-interface-failure",
        &stdout,
        &[&format!("wrote llvm-ir: {}", output_path.display())],
    )
    .expect("build-side interface failure should still report the successful build artifact");
    expect_stderr_contains(
        "build-emit-interface-failure",
        "build with failing interface emission",
        &stderr,
        "a_broken.ql",
    )
    .expect("build-side interface failure should still surface the failing package source");
    expect_stderr_contains(
        "build-emit-interface-failure",
        "build with failing interface emission",
        &stderr,
        "z_broken.ql",
    )
    .expect("build-side interface failure should continue surfacing later package source failures");
    expect_stderr_contains(
        "build-emit-interface-failure",
        "build with failing interface emission",
        &stderr,
        "interface emission found 2 failing source file(s)",
    )
    .expect("build-side interface failure should summarize all failing package sources");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stderr_contains(
        "build-emit-interface-failure",
        "build with failing interface emission",
        &normalized_stderr,
        &format!("note: first failing source file: {first_failure_display}"),
    )
    .expect("build-side interface failure should point to the first failing package source");
    expect_stderr_contains(
        "build-emit-interface-failure",
        "build with failing interface emission",
        &normalized_stderr,
        &format!("note: failing package manifest: {}", manifest_display),
    )
    .expect("build-side interface failure should point to the failing package manifest");
    expect_stderr_contains(
        "build-emit-interface-failure",
        "build with failing interface emission",
        &normalized_stderr,
        &format!(
            "hint: rerun `ql build {} --emit llvm-ir --output {} --emit-interface` after fixing the package interface error",
            source_display, output_display
        ),
    )
    .expect("build-side interface failure should preserve the original build rerun options");
    expect_stderr_not_contains(
        "build-emit-interface-failure",
        "build with failing interface emission",
        &normalized_stderr,
        &format!(
            "hint: rerun `ql project emit-interface {}` after fixing the package interface error",
            manifest_display
        ),
    )
    .expect("build-side interface failure should not fall back to a project-only rerun hint");
    expect_stderr_contains(
        "build-emit-interface-failure",
        "build with failing interface emission",
        &normalized_stderr,
        &format!("note: build artifact remains at `{}`", output_display),
    )
    .expect("build-side interface failure should confirm that the build artifact was preserved");
    expect_file_exists(
        "build-emit-interface-failure",
        &output_path,
        "generated llvm ir",
        "build with failing interface emission",
    )
    .expect("build-side interface failure should keep the successful build artifact");
    assert!(
        !interface_path.is_file(),
        "build-side interface failure should not leave behind a partial interface artifact"
    );
}

#[test]
fn build_with_emit_interface_dedupes_single_failing_source_summary() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-build-emit-interface-single-failure");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create project source directory for single build-side interface failure test");
    let source_path = temp.write(
        "workspace/app/src/lib.ql",
        r#"
package demo.app

pub fn exported(value: Int) -> Int {
    return value
}

fn main() -> Int {
    return exported(1)
}
"#,
    );
    let broken_source = temp.write(
        "workspace/app/src/broken.ql",
        r#"
package demo.app

pub fn broken(value: MissingType) -> Int {
    return value
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    let output_path = project_root.join("build").join("app.ll");
    let manifest_path = project_root.join("qlang.toml");
    let interface_path = project_root.join("app.qi");
    let manifest_display = manifest_path.to_string_lossy().replace('\\', "/");
    let source_display = source_path.to_string_lossy().replace('\\', "/");
    let output_display = output_path.to_string_lossy().replace('\\', "/");
    let broken_source_display = broken_source.to_string_lossy().replace('\\', "/");

    let mut command = ql_command(&workspace_root);
    command
        .arg("build")
        .arg(&source_path)
        .args(["--emit", "llvm-ir", "--output"])
        .arg(&output_path)
        .arg("--emit-interface");
    let output = run_command_capture(
        &mut command,
        "`ql build --emit-interface` with single failing package interface source",
    );
    let (stdout, stderr) = expect_exit_code(
        "build-emit-interface-single-failure",
        "build with single failing interface source",
        &output,
        1,
    )
    .expect("build should fail when package interface emission has a single failing source");
    expect_stdout_contains_all(
        "build-emit-interface-single-failure",
        &stdout,
        &[&format!("wrote llvm-ir: {}", output_path.display())],
    )
    .expect("build-side single source failure should still report the successful build artifact");
    expect_stderr_contains(
        "build-emit-interface-single-failure",
        "build with single failing interface source",
        &stderr,
        "broken.ql",
    )
    .expect("build-side single source failure should surface the broken source file");
    expect_stderr_contains(
        "build-emit-interface-single-failure",
        "build with single failing interface source",
        &stderr,
        "interface emission found 1 failing source file(s)",
    )
    .expect("build-side single source failure should summarize the single failing source");
    let normalized_stderr = stderr.replace('\\', "/");
    expect_stderr_not_contains(
        "build-emit-interface-single-failure",
        "build with single failing interface source",
        &normalized_stderr,
        "note: first failing source file:",
    )
    .expect(
        "single failing build-side sources should not repeat the source path in the final summary",
    );
    expect_stderr_contains(
        "build-emit-interface-single-failure",
        "build with single failing interface source",
        &normalized_stderr,
        &broken_source_display,
    )
    .expect("build-side single source failure should still surface the broken source path locally");
    expect_stderr_contains(
        "build-emit-interface-single-failure",
        "build with single failing interface source",
        &normalized_stderr,
        &format!("note: failing package manifest: {}", manifest_display),
    )
    .expect("build-side single source failure should still point to the failing package manifest");
    expect_stderr_contains(
        "build-emit-interface-single-failure",
        "build with single failing interface source",
        &normalized_stderr,
        &format!(
            "hint: rerun `ql build {} --emit llvm-ir --output {} --emit-interface` after fixing the package interface error",
            source_display, output_display
        ),
    )
    .expect("build-side single source failure should preserve the original build rerun options");
    expect_stderr_not_contains(
        "build-emit-interface-single-failure",
        "build with single failing interface source",
        &normalized_stderr,
        &format!(
            "hint: rerun `ql project emit-interface {}` after fixing the package interface error",
            manifest_display
        ),
    )
    .expect("build-side single source failure should not fall back to a project-only rerun hint");
    expect_stderr_contains(
        "build-emit-interface-single-failure",
        "build with single failing interface source",
        &normalized_stderr,
        &format!("note: build artifact remains at `{}`", output_display),
    )
    .expect(
        "build-side single source failure should confirm that the build artifact was preserved",
    );
    expect_file_exists(
        "build-emit-interface-single-failure",
        &output_path,
        "generated llvm ir",
        "build with single failing interface source",
    )
    .expect("build-side single source failure should keep the successful build artifact");
    assert!(
        !interface_path.is_file(),
        "build-side single source failure should not leave behind a partial interface artifact"
    );
}

#[test]
fn build_with_emit_interface_points_blocked_output_paths_at_interface_target() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-build-emit-interface-output-path-failure");
    let project_root = temp.path().join("workspace").join("app");
    std::fs::create_dir_all(project_root.join("src"))
        .expect("create project source directory for blocked build-side interface output test");
    let source_path = temp.write(
        "workspace/app/src/lib.ql",
        r#"
package demo.app

pub fn exported(value: Int) -> Int {
    return value
}

fn main() -> Int {
    return exported(1)
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    let interface_path = project_root.join("app.qi");
    std::fs::create_dir_all(&interface_path)
        .expect("create blocking interface directory for build-side emit-interface test");
    let output_path = project_root.join("build").join("app.ll");
    let manifest_path = project_root.join("qlang.toml");
    let manifest_display = manifest_path.to_string_lossy().replace('\\', "/");
    let interface_display = interface_path.to_string_lossy().replace('\\', "/");
    let output_display = output_path.to_string_lossy().replace('\\', "/");
    let source_display = source_path.to_string_lossy().replace('\\', "/");

    let mut command = ql_command(&workspace_root);
    command
        .arg("build")
        .arg(&source_path)
        .args(["--emit", "llvm-ir", "--output"])
        .arg(&output_path)
        .arg("--emit-interface");
    let output = run_command_capture(
        &mut command,
        "`ql build --emit-interface` with blocked interface output path",
    );
    let (stdout, stderr) = expect_exit_code(
        "build-emit-interface-output-path-failure",
        "build with blocked interface output path",
        &output,
        1,
    )
    .expect("build should fail when `--emit-interface` cannot write the default `.qi` path");
    expect_stdout_contains_all(
        "build-emit-interface-output-path-failure",
        &stdout,
        &[&format!("wrote llvm-ir: {}", output_path.display())],
    )
    .expect("build-side blocked output path should still report the successful build artifact");
    expect_stderr_contains(
        "build-emit-interface-output-path-failure",
        "build with blocked interface output path",
        &stderr,
        "failed to write interface",
    )
    .expect("build-side blocked output path should surface a write failure");
    let normalized_stderr = stderr.replace('\\', "/");
    let package_note = format!("note: failing package manifest: {manifest_display}");
    let output_note = format!("note: failing interface output path: {interface_display}");
    let rerun_hint = format!(
        "hint: rerun `ql build {} --emit llvm-ir --output {} --emit-interface` after fixing the interface output path",
        source_display, output_display
    );
    let old_hint = format!(
        "hint: rerun `ql project emit-interface {}` after fixing the package interface error",
        manifest_display
    );
    expect_stderr_contains(
        "build-emit-interface-output-path-failure",
        "build with blocked interface output path",
        &normalized_stderr,
        &package_note,
    )
    .expect("build-side blocked output path should still point to the package manifest");
    expect_stderr_contains(
        "build-emit-interface-output-path-failure",
        "build with blocked interface output path",
        &normalized_stderr,
        &output_note,
    )
    .expect("build-side blocked output path should point to the failing interface target");
    expect_stderr_contains(
        "build-emit-interface-output-path-failure",
        "build with blocked interface output path",
        &normalized_stderr,
        &rerun_hint,
    )
    .expect("build-side blocked output path should suggest fixing the interface output path");
    expect_stderr_not_contains(
        "build-emit-interface-output-path-failure",
        "build with blocked interface output path",
        &normalized_stderr,
        &format!(
            "hint: rerun `ql project emit-interface {}` after fixing the interface output path",
            manifest_display
        ),
    )
    .expect("build-side blocked output path should not fall back to a project-only rerun hint");
    expect_stderr_not_contains(
        "build-emit-interface-output-path-failure",
        "build with blocked interface output path",
        &normalized_stderr,
        &old_hint,
    )
    .expect("build-side blocked output path should not reuse the package-source failure hint");
    expect_stderr_contains(
        "build-emit-interface-output-path-failure",
        "build with blocked interface output path",
        &normalized_stderr,
        &format!("note: build artifact remains at `{}`", output_display),
    )
    .expect("build-side blocked output path should confirm that the build artifact was preserved");
    expect_file_exists(
        "build-emit-interface-output-path-failure",
        &output_path,
        "generated llvm ir",
        "build with blocked interface output path",
    )
    .expect("build-side blocked output path should keep the successful build artifact");
    assert!(
        interface_path.is_dir(),
        "build-side blocked output path test should preserve `{}` as a directory",
        interface_path.display()
    );
}
