#![allow(dead_code)]

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

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

pub fn run_ql_build_capture(
    workspace_root: &Path,
    relative_ql: &str,
    emit: &str,
    output_path: &Path,
    extra_args: &[String],
) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_ql"));
    command.current_dir(workspace_root).args([
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
    command
        .output()
        .unwrap_or_else(|_| panic!("run `ql build {relative_ql} --emit {emit}`"))
}
