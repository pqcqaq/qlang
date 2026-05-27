use super::*;

fn unique_temp_file(name: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "ql-cli-{name}-{}-{}.tmp",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after unix epoch")
            .as_nanos()
    ))
}

#[test]
fn lock_check_status_accepts_equivalent_line_endings() {
    let path = unique_temp_file("lock-check-line-endings");
    fs::write(&path, "one\r\ntwo\r\n").expect("write temp lockfile");

    let status = project_lockfile_check_status(&path, "one\ntwo\n");

    let _ = fs::remove_file(&path);
    assert_eq!(status, ProjectLockCheckStatus::UpToDate);
}

#[test]
fn lock_check_status_reports_stale_and_missing_files() {
    let stale_path = unique_temp_file("lock-check-stale");
    let missing_path = unique_temp_file("lock-check-missing");
    fs::write(&stale_path, "old\n").expect("write temp lockfile");

    let stale_status = project_lockfile_check_status(&stale_path, "new\n");
    let missing_status = project_lockfile_check_status(&missing_path, "new\n");

    let _ = fs::remove_file(&stale_path);
    assert_eq!(stale_status, ProjectLockCheckStatus::Stale);
    assert_eq!(missing_status, ProjectLockCheckStatus::Missing);
}
