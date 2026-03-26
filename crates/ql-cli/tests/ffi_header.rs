use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn ffi_header_snapshot_matches() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-ffi-header");
    let output_path = temp.path().join("extern_c_export.h");
    let expected_path = workspace_root.join("tests/codegen/pass/extern_c_export.h");
    let expected = normalize(
        &fs::read_to_string(&expected_path)
            .unwrap_or_else(|_| panic!("read expected snapshot `{}`", expected_path.display())),
    );

    let output = Command::new(env!("CARGO_BIN_EXE_ql"))
        .current_dir(&workspace_root)
        .args([
            "ffi",
            "header",
            "tests/ffi/pass/extern_c_export.ql",
            "--output",
            &output_path.to_string_lossy(),
        ])
        .output()
        .expect("run `ql ffi header tests/ffi/pass/extern_c_export.ql`");
    let stdout = normalize(&String::from_utf8_lossy(&output.stdout));
    let stderr = normalize(&String::from_utf8_lossy(&output.stderr));

    assert_eq!(
        output.status.code(),
        Some(0),
        "expected exit code 0\nstdout:\n{}\nstderr:\n{}",
        stdout,
        stderr
    );
    assert!(
        stderr.trim().is_empty(),
        "expected no stderr for successful header generation\nstderr:\n{}",
        stderr
    );

    let actual = normalize(
        &fs::read_to_string(&output_path)
            .unwrap_or_else(|_| panic!("read generated header `{}`", output_path.display())),
    );
    assert_eq!(actual, expected, "generated header snapshot mismatch");
}

#[test]
fn ffi_header_rejects_unsupported_export_signature() {
    let workspace_root = workspace_root();
    let temp = TempDir::new("ql-ffi-header-fail");
    let source = temp.write(
        "unsupported.ql",
        r#"
extern "c" pub fn q_print(message: String) -> Void {
}
"#,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_ql"))
        .current_dir(&workspace_root)
        .args(["ffi", "header", &source.to_string_lossy()])
        .output()
        .expect("run `ql ffi header` on unsupported signature");
    let stdout = normalize(&String::from_utf8_lossy(&output.stdout));
    let stderr = normalize(&String::from_utf8_lossy(&output.stderr));

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected exit code 1\nstdout:\n{}\nstderr:\n{}",
        stdout,
        stderr
    );
    assert!(
        stdout.trim().is_empty(),
        "expected no stdout for failing header generation\nstdout:\n{}",
        stdout
    );
    assert!(
        stderr.contains("C header generation does not support parameter type `String` yet"),
        "expected unsupported-type diagnostic in stderr\nstderr:\n{}",
        stderr
    );
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
        fs::create_dir_all(&path).expect("create temporary ffi header test directory");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn write(&self, relative: &str, contents: &str) -> PathBuf {
        let path = self.path.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent directory for temp source");
        }
        fs::write(&path, contents).expect("write temp ql source");
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
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

fn normalize(text: &str) -> String {
    text.replace("\r\n", "\n")
}
