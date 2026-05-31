mod support;

use ql_driver::{ToolchainOptions, discover_toolchain};
use support::project_run_dependencies::{
    expect_dependency_run_project_exits, write_dependency_run_project,
};

fn toolchain_available(context: &str) -> bool {
    let Ok(_toolchain) = discover_toolchain(&ToolchainOptions::default()) else {
        eprintln!(
            "skipping {context}: no clang-style compiler found via ql-driver toolchain discovery"
        );
        return false;
    };
    true
}

#[test]
fn run_package_path_syncs_dependency_interfaces_without_polluting_program_output() {
    if !toolchain_available("`ql run` dependency sync test") {
        return;
    }

    let fixture = write_dependency_run_project(
        "ql-project-run-dependency-sync",
        "extern \"c\" pub fn q_add(left: Int, right: Int) -> Int { return left + right }\n",
        "use dep.q_add as add\n\nfn main() -> Int { return add(6, 7) }\n",
    );
    expect_dependency_run_project_exits(
        "project-run-dependency-sync",
        "package path run with dependency sync",
        "`ql run` dependency sync",
        &fixture,
        13,
    );
}
