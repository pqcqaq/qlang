#![allow(dead_code)]

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    pub fn new(prefix: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let path = env::temp_dir().join(format!("{prefix}-{unique}"));
        fs::create_dir_all(&path).expect("create temporary test directory");
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn write(&self, relative: &str, contents: &str) -> PathBuf {
        let path = self.path.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent directory for temp file");
        }
        fs::write(&path, contents).expect("write temp file");
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

pub fn workspace_root() -> PathBuf {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let crates_dir = crate_dir
        .parent()
        .expect("ql-cli crate should have a parent directory");
    crates_dir
        .parent()
        .expect("workspace root should exist")
        .to_path_buf()
}

pub fn executable_output_path(root: &Path, stem: &str) -> PathBuf {
    if cfg!(windows) {
        root.join(format!("{stem}.exe"))
    } else {
        root.join(stem)
    }
}

pub fn static_library_output_path(root: &Path, stem: &str) -> PathBuf {
    if cfg!(windows) {
        root.join(format!("{stem}.lib"))
    } else {
        root.join(format!("lib{stem}.a"))
    }
}

pub fn dynamic_library_output_path(root: &Path, stem: &str) -> PathBuf {
    if cfg!(windows) {
        root.join(format!("{stem}.dll"))
    } else if cfg!(target_os = "macos") {
        root.join(format!("lib{stem}.dylib"))
    } else {
        root.join(format!("lib{stem}.so"))
    }
}

pub fn normalize(text: &str) -> String {
    text.replace("\r\n", "\n")
}

pub fn normalize_trimmed(text: &str) -> String {
    normalize(text).trim_end().to_owned()
}

pub fn read_normalized_file(path: &Path, subject: &str) -> String {
    normalize(
        &fs::read_to_string(path).unwrap_or_else(|_| panic!("read {subject} `{}`", path.display())),
    )
}

pub fn read_normalized_trimmed_file(path: &Path, subject: &str) -> String {
    normalize_trimmed(
        &fs::read_to_string(path).unwrap_or_else(|_| panic!("read {subject} `{}`", path.display())),
    )
}

pub fn ql_command(workspace_root: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ql"));
    command.current_dir(workspace_root);
    command
}

pub fn run_command_capture(command: &mut Command, description: impl Into<String>) -> Output {
    let description = description.into();
    command
        .output()
        .unwrap_or_else(|_| panic!("run {description}"))
}

pub fn normalized_output(output: &Output) -> (String, String) {
    (
        normalize(&String::from_utf8_lossy(&output.stdout)),
        normalize(&String::from_utf8_lossy(&output.stderr)),
    )
}

pub fn expect_exit_code(
    case_name: &str,
    action: &str,
    output: &Output,
    expected_code: i32,
) -> Result<(String, String), String> {
    let (stdout, stderr) = normalized_output(output);
    if output.status.code() != Some(expected_code) {
        return Err(format!(
            "[{case_name}] expected {action} to exit with {expected_code}, got {:?}\nstdout:\n{stdout}\nstderr:\n{stderr}",
            output.status.code()
        ));
    }
    Ok((stdout, stderr))
}

pub fn expect_success(
    case_name: &str,
    action: &str,
    output: &Output,
) -> Result<(String, String), String> {
    expect_exit_code(case_name, action, output, 0)
}

pub fn expect_file_exists(
    case_name: &str,
    path: &Path,
    subject: &str,
    action: &str,
) -> Result<(), String> {
    if !path.is_file() {
        return Err(format!(
            "[{case_name}] expected {subject} `{}` to exist after {action}",
            path.display()
        ));
    }
    Ok(())
}

pub fn assert_no_build_lock_directories(case_name: &str, root: &Path) {
    let mut pending = vec![root.to_path_buf()];
    while let Some(path) = pending.pop() {
        let Ok(entries) = fs::read_dir(&path) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let file_name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("");
            assert!(
                !file_name.ends_with(".ql-build.lock"),
                "[{case_name}] build output lock directory leaked at `{}`",
                path.display()
            );
            if path.is_dir() {
                pending.push(path);
            }
        }
    }
}

pub fn wait_for_path_exists(
    case_name: &str,
    action: &str,
    path: &Path,
    timeout: Duration,
) -> Result<(), String> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if path.exists() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(20));
    }
    Err(format!(
        "[{case_name}] expected {action} to create `{}` within {} ms",
        path.display(),
        timeout.as_millis()
    ))
}

pub fn sleep_program_source(milliseconds: u64) -> String {
    if cfg!(windows) {
        format!(
            r#"
extern "c" {{
    fn Sleep(milliseconds: Int)
}}

fn main() -> Int {{
    Sleep({milliseconds})
    return 0
}}
"#
        )
    } else {
        let microseconds = milliseconds * 1000;
        format!(
            r#"
extern "c" {{
    fn usleep(microseconds: Int) -> Int
}}

fn main() -> Int {{
    return usleep({microseconds})
}}
"#
        )
    }
}

pub fn expect_empty_stderr(case_name: &str, action: &str, stderr: &str) -> Result<(), String> {
    if !stderr.trim().is_empty() {
        return Err(format!(
            "[{case_name}] expected {action} stderr to be empty, got:\n{stderr}"
        ));
    }
    Ok(())
}

pub fn expect_empty_stdout(case_name: &str, action: &str, stdout: &str) -> Result<(), String> {
    if !stdout.trim().is_empty() {
        return Err(format!(
            "[{case_name}] expected {action} stdout to be empty, got:\n{stdout}"
        ));
    }
    Ok(())
}

pub fn expect_silent_output(
    case_name: &str,
    action: &str,
    stdout: &str,
    stderr: &str,
) -> Result<(), String> {
    if !stdout.trim().is_empty() || !stderr.trim().is_empty() {
        return Err(format!(
            "[{case_name}] expected {action} to be silent\nstdout:\n{stdout}\nstderr:\n{stderr}"
        ));
    }
    Ok(())
}

pub fn expect_stderr_contains(
    case_name: &str,
    action: &str,
    stderr: &str,
    fragment: &str,
) -> Result<(), String> {
    if !stderr.contains(fragment) {
        return Err(format!(
            "[{case_name}] expected {action} stderr to contain `{fragment}`, got:\n{stderr}"
        ));
    }
    Ok(())
}

pub fn expect_stderr_not_contains(
    case_name: &str,
    action: &str,
    stderr: &str,
    fragment: &str,
) -> Result<(), String> {
    if stderr.contains(fragment) {
        return Err(format!(
            "[{case_name}] expected {action} stderr not to contain `{fragment}`, got:\n{stderr}"
        ));
    }
    Ok(())
}

pub fn expect_stdout_contains_all(
    case_name: &str,
    stdout: &str,
    expected_fragments: &[&str],
) -> Result<(), String> {
    for fragment in expected_fragments {
        if !stdout.contains(fragment) {
            return Err(format!(
                "[{case_name}] expected stdout to contain `{fragment}`, got:\n{stdout}"
            ));
        }
    }
    Ok(())
}

pub fn expect_snapshot_matches(
    case_name: &str,
    subject: &str,
    expected: &str,
    actual: &str,
) -> Result<(), String> {
    if actual != expected {
        return Err(format!(
            "[{case_name}] {subject} mismatch\n--- expected ---\n{expected}\n--- actual ---\n{actual}"
        ));
    }
    Ok(())
}

pub fn run_ql_build_capture(
    workspace_root: &Path,
    relative_ql: &str,
    emit: &str,
    output_path: &Path,
    extra_args: &[String],
) -> Output {
    let mut command = ql_command(workspace_root);
    command.args([
        "build",
        relative_ql,
        "--emit",
        emit,
        "--output",
        &output_path.to_string_lossy(),
    ]);
    for arg in extra_args {
        command.arg(arg);
    }
    run_command_capture(
        &mut command,
        format!("`ql build {relative_ql} --emit {emit}`"),
    )
}
