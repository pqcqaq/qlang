use std::fs;
use std::path::PathBuf;

use serde_json::Value as JsonValue;

use crate::support::{TempDir, executable_output_path};

pub fn parse_json_output(case_name: &str, stdout: &str) -> JsonValue {
    serde_json::from_str(&stdout.replace("\r\n", "\n"))
        .unwrap_or_else(|error| panic!("[{case_name}] parse json stdout: {error}\n{stdout}"))
}

pub struct WorkspaceTestPackageSelectorProject {
    pub temp: TempDir,
    pub project_root: PathBuf,
    pub app_root: PathBuf,
    pub selected_smoke_output: PathBuf,
    pub selected_extra_output: PathBuf,
    pub unselected_smoke_output: PathBuf,
}

pub fn write_workspace_test_package_selector_project(
    prefix: &str,
) -> WorkspaceTestPackageSelectorProject {
    let temp = TempDir::new(prefix);
    let project_root = temp.path().join("workspace");
    let app_root = project_root.join("packages").join("app");
    fs::create_dir_all(project_root.join("packages/app/src"))
        .expect("create app package source tree");
    fs::create_dir_all(project_root.join("packages/tool/src"))
        .expect("create tool package source tree");

    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app", "packages/tool"]
"#,
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"
"#,
    );
    temp.write(
        "workspace/packages/tool/qlang.toml",
        r#"
[package]
name = "tool"
"#,
    );
    temp.write(
        "workspace/packages/app/src/lib.ql",
        "pub fn helper() -> Int { return 1 }\n",
    );
    temp.write(
        "workspace/packages/tool/src/lib.ql",
        "pub fn helper() -> Int { return 2 }\n",
    );
    temp.write(
        "workspace/packages/app/tests/app_only.ql",
        "fn main() -> Int { return 0 }\n",
    );
    temp.write(
        "workspace/packages/app/tests/api/extra.ql",
        "fn main() -> Int { return 0 }\n",
    );
    temp.write(
        "workspace/packages/tool/tests/tool_only.ql",
        "fn main() -> Int { return 0 }\n",
    );

    let selected_extra_output = executable_output_path(
        &project_root.join("packages/app/target/ql/debug/tests/api"),
        "extra",
    );
    let selected_smoke_output = executable_output_path(
        &project_root.join("packages/app/target/ql/debug/tests"),
        "app_only",
    );
    let unselected_smoke_output = executable_output_path(
        &project_root.join("packages/tool/target/ql/debug/tests"),
        "tool_only",
    );

    WorkspaceTestPackageSelectorProject {
        temp,
        project_root,
        app_root,
        selected_smoke_output,
        selected_extra_output,
        unselected_smoke_output,
    }
}
