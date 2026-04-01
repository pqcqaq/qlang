mod support;

use std::fs;
use std::path::{Path, PathBuf};

use support::{
    expect_empty_stdout, expect_exit_code, expect_snapshot_matches, ql_command,
    read_normalized_file, run_command_capture, workspace_root,
};

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
        let expected = read_normalized_file(&expected_path, "expected snapshot");

        let mut command = ql_command(&workspace_root);
        command.args(["check", &relative]);
        let output = run_command_capture(&mut command, format!("`ql check {relative}`"));
        let (stdout, stderr) = match expect_exit_code(&relative, "failing fixture", &output, 1) {
            Ok(output) => output,
            Err(message) => {
                failures.push(message);
                continue;
            }
        };

        if let Err(message) = expect_empty_stdout(&relative, "failing fixture", &stdout) {
            failures.push(message);
        }

        if let Err(message) =
            expect_snapshot_matches(&relative, "stderr snapshot", &expected, &stderr)
        {
            failures.push(message);
        }
    }

    assert!(
        failures.is_empty(),
        "UI snapshot regressions:\n\n{}",
        failures.join("\n\n")
    );
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
