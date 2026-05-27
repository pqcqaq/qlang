use super::*;
use std::io;
use std::path::PathBuf;

#[test]
fn lock_error_message_reports_lock_path() {
    let message = build_output_lock_error_message(BuildError::Io {
        path: PathBuf::from("target/ql/debug/app.exe.lock"),
        error: io::Error::new(io::ErrorKind::PermissionDenied, "busy"),
    });

    assert!(message.contains("failed to acquire build output lock"));
    assert!(message.contains("busy"));
    assert!(message.contains("app.exe.lock"));
}
