use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[test]
fn ui_diagnostics_snapshots_match() {
    let workspace_root = workspace_root();
    let fixture_root = workspace_root.join("tests/ui");
    let fixtures = collect_ui_fixtures(&fixture_root);
    assert!(
        !fixtures.is_empty(),
        "expected at least one UI fixture under `{}`",
        fixture_root.display()
    );

    let mut failures = Vec::new();
    for fixture in fixtures {
        let relative = fixture
            .strip_prefix(&workspace_root)
            .expect("fixture should be inside workspace root")
            .to_string_lossy()
            .replace('\\', "/");
        let expected_path = fixture.with_extension("stderr");
        let expected =
            normalize(&fs::read_to_string(&expected_path).unwrap_or_else(|_| {
                panic!("read expected snapshot `{}`", expected_path.display())
            }));

        let output = Command::new(env!("CARGO_BIN_EXE_ql"))
            .current_dir(&workspace_root)
            .args(["check", &relative])
            .output()
            .unwrap_or_else(|_| panic!("run `ql check {relative}`"));

        let stdout = normalize(&String::from_utf8_lossy(&output.stdout));
        let stderr = normalize(&String::from_utf8_lossy(&output.stderr));

        if output.status.code().is_none_or(|code| code != 1) {
            failures.push(format!(
                "[{relative}] expected exit code 1, got {:?}\nstdout:\n{stdout}\nstderr:\n{stderr}",
                output.status.code()
            ));
            continue;
        }

        if !stdout.trim().is_empty() {
            failures.push(format!(
                "[{relative}] expected no stdout for failing fixture\nstdout:\n{stdout}"
            ));
        }

        if stderr != expected {
            failures.push(format!(
                "[{relative}] stderr snapshot mismatch\n--- expected ---\n{expected}\n--- actual ---\n{stderr}"
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "UI snapshot regressions:\n\n{}",
        failures.join("\n\n")
    );
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

fn collect_ui_fixtures(root: &Path) -> Vec<PathBuf> {
    let mut fixtures = Vec::new();
    collect_ui_fixtures_recursive(root, &mut fixtures);
    fixtures.sort();
    fixtures
}

fn collect_ui_fixtures_recursive(root: &Path, fixtures: &mut Vec<PathBuf>) {
    for entry in
        fs::read_dir(root).unwrap_or_else(|_| panic!("read fixture dir `{}`", root.display()))
    {
        let entry = entry.expect("read fixture directory entry");
        let path = entry.path();
        if path.is_dir() {
            collect_ui_fixtures_recursive(&path, fixtures);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("ql") {
            fixtures.push(path);
        }
    }
}

fn normalize(text: &str) -> String {
    text.replace("\r\n", "\n")
}
