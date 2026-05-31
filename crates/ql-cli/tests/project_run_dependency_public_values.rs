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
fn run_package_path_supports_direct_dependency_public_functions() {
    if !toolchain_available("`ql run` dependency public function test") {
        return;
    }

    let fixture = write_dependency_run_project(
        "ql-project-run-dependency-public-function",
        "pub fn add(left: Int, right: Int) -> Int { return left + right }\n",
        "use dep.add as sum\n\nfn main() -> Int { return sum(9, 4) }\n",
    );
    expect_dependency_run_project_exits(
        "project-run-dependency-public-function",
        "package path run with dependency public function",
        "`ql run` dependency public function",
        &fixture,
        13,
    );
}

#[test]
fn run_package_path_supports_direct_dependency_public_values() {
    if !toolchain_available("`ql run` dependency public value test") {
        return;
    }

    let fixture = write_dependency_run_project(
        "ql-project-run-dependency-public-value",
        "pub const VALUE: Int = 7\npub static READY: Bool = true\npub static VALUES: [Int; 3] = [1, 3, 5]\n",
        r#"
use dep.VALUE as THRESHOLD
use dep.READY as ENABLED
use dep.VALUES as ITEMS

fn main() -> Int {
    if ENABLED {
        return THRESHOLD + ITEMS[1]
    }
    return 0
}
"#,
    );
    expect_dependency_run_project_exits(
        "project-run-dependency-public-value",
        "package path run with dependency public value",
        "`ql run` dependency public value",
        &fixture,
        10,
    );
}

#[test]
fn run_package_path_supports_dependency_public_values_with_function_initializers() {
    if !toolchain_available("`ql run` dependency public value initializer test") {
        return;
    }

    let fixture = write_dependency_run_project(
        "ql-project-run-dependency-public-value-function-initializers",
        "pub fn add_one(value: Int) -> Int { return value + 1 }\npub fn make_value() -> Int { return add_one(6) }\npub const VALUE: Int = make_value()\npub const APPLY: (Int) -> Int = add_one\n",
        "use dep.VALUE as VALUE_ALIAS\nuse dep.APPLY as RUN\n\nfn main() -> Int { return VALUE_ALIAS + RUN(3) }\n",
    );
    expect_dependency_run_project_exits(
        "project-run-dependency-public-value-function-initializers",
        "package path run with dependency public value function initializers",
        "`ql run` dependency public value initializer",
        &fixture,
        11,
    );
}
