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
fn run_package_path_supports_direct_dependency_public_struct_functions() {
    if !toolchain_available("`ql run` dependency public struct function test") {
        return;
    }

    let fixture = write_dependency_run_project(
        "ql-project-run-dependency-public-struct-function",
        "pub struct Box { value: Int }\npub fn make_box() -> Box { return Box { value: 7 } }\n",
        "use dep.make_box as make\n\nfn main() -> Int {\n    let value = make()\n    return value.value\n}\n",
    );
    expect_dependency_run_project_exits(
        "project-run-dependency-public-struct-function",
        "package path run with dependency public struct function",
        "`ql run` dependency public struct function",
        &fixture,
        7,
    );
}

#[test]
fn run_package_path_supports_direct_dependency_public_type_alias_functions() {
    if !toolchain_available("`ql run` dependency public type alias function test") {
        return;
    }

    let fixture = write_dependency_run_project(
        "ql-project-run-dependency-public-type-alias-function",
        "pub type Count = Int\npub type Score = Count\npub fn make_score(value: Count) -> Score { return value + 2 }\npub fn unwrap_score(value: Score) -> Int { return value }\n",
        "use dep.make_score as make_score\nuse dep.unwrap_score as unwrap_score\n\nfn main() -> Int {\n    return unwrap_score(make_score(5))\n}\n",
    );
    expect_dependency_run_project_exits(
        "project-run-dependency-public-type-alias-function",
        "package path run with dependency public type alias function",
        "`ql run` dependency public type alias function",
        &fixture,
        7,
    );
}

#[test]
fn run_package_path_supports_direct_dependency_public_struct_methods() {
    if !toolchain_available("`ql run` dependency public struct method test") {
        return;
    }

    let fixture = write_dependency_run_project(
        "ql-project-run-dependency-public-struct-method",
        "pub struct Box { value: Int }\n\nimpl Box {\n    pub fn read(self) -> Int {\n        return self.value\n    }\n}\n\npub fn make_box() -> Box { return Box { value: 7 } }\n",
        "use dep.make_box as make\n\nfn main() -> Int {\n    let value = make()\n    return value.read()\n}\n",
    );
    expect_dependency_run_project_exits(
        "project-run-dependency-public-struct-method",
        "package path run with dependency public struct method",
        "`ql run` dependency public struct method",
        &fixture,
        7,
    );
}

#[test]
fn run_package_path_supports_direct_dependency_public_struct_method_values() {
    if !toolchain_available("`ql run` dependency public struct method value test") {
        return;
    }

    let fixture = write_dependency_run_project(
        "ql-project-run-dependency-public-struct-method-value",
        "pub struct Box { value: Int }\n\nimpl Box {\n    pub fn add(self, delta: Int) -> Int {\n        return self.value + delta\n    }\n}\n\npub fn make_box() -> Box { return Box { value: 7 } }\n",
        "use dep.make_box as make\n\nfn main() -> Int {\n    let value = make()\n    let add = value.add\n    return add(5)\n}\n",
    );
    expect_dependency_run_project_exits(
        "project-run-dependency-public-struct-method-value",
        "package path run with dependency public struct method value",
        "`ql run` dependency public struct method value",
        &fixture,
        12,
    );
}

#[test]
fn run_package_path_supports_direct_dependency_public_enums() {
    if !toolchain_available("`ql run` dependency public enum test") {
        return;
    }

    let fixture = write_dependency_run_project(
        "ql-project-run-dependency-public-enum",
        "pub struct Issue { code: Int }\n\npub enum Status {\n    Ready,\n    Failed {\n        issue: Issue,\n    },\n}\n\npub fn load_status() -> Status {\n    return Status.Failed { issue: Issue { code: 3 } }\n}\n",
        "use dep.load_status as load\n\nfn main() -> Int {\n    return match load() {\n        Status.Ready => 1,\n        Status.Failed { issue } => issue.code + 4,\n    }\n}\n",
    );
    expect_dependency_run_project_exits(
        "project-run-dependency-public-enum",
        "package path run with dependency public enum",
        "`ql run` dependency public enum",
        &fixture,
        7,
    );
}

#[test]
fn run_package_path_supports_direct_dependency_public_tuple_enums() {
    if !toolchain_available("`ql run` dependency public tuple enum test") {
        return;
    }

    let fixture = write_dependency_run_project(
        "ql-project-run-dependency-public-tuple-enum",
        "pub struct Issue { code: Int }\n\npub enum Status {\n    Ready,\n    Failed(Issue),\n}\n\npub fn load_status() -> Status {\n    return Status.Failed(Issue { code: 3 })\n}\n",
        "use dep.load_status as load\n\nfn main() -> Int {\n    return match load() {\n        Status.Ready => 1,\n        Status.Failed(issue) => issue.code + 4,\n    }\n}\n",
    );
    expect_dependency_run_project_exits(
        "project-run-dependency-public-tuple-enum",
        "package path run with dependency public tuple enum",
        "`ql run` dependency public tuple enum",
        &fixture,
        7,
    );
}

#[test]
fn run_package_path_supports_direct_dependency_public_trait_methods() {
    if !toolchain_available("`ql run` dependency public trait method test") {
        return;
    }

    let fixture = write_dependency_run_project(
        "ql-project-run-dependency-public-trait-method",
        "pub trait Reader {\n    fn read(self) -> Int\n}\n\npub struct Box { value: Int }\n\nimpl Reader for Box {\n    pub fn read(self) -> Int {\n        return self.value\n    }\n}\n\npub fn make_box() -> Box { return Box { value: 7 } }\n",
        "use dep.make_box as make\n\nfn main() -> Int {\n    let value = make()\n    return value.read()\n}\n",
    );
    expect_dependency_run_project_exits(
        "project-run-dependency-public-trait-method",
        "package path run with dependency public trait method",
        "`ql run` dependency public trait method",
        &fixture,
        7,
    );
}
