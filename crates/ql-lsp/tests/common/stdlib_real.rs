#![allow(dead_code)]

use std::path::{Path, PathBuf};

use tower_lsp::lsp_types::Url;

use crate::common::request::TempDir;

pub struct RealStdlibWorkspace {
    pub app_root: PathBuf,
    pub app_uri: Url,
    pub stdlib_root: PathBuf,
}

pub fn write_real_stdlib_workspace(temp: &TempDir, app_source: &str) -> RealStdlibWorkspace {
    write_real_stdlib_packages(temp);

    let app_root = temp.path().join("workspace").join("app");
    let app_path = temp.write("workspace/app/src/main.ql", app_source);
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[dependencies]
"std.core" = "../stdlib/packages/core"
"std.option" = "../stdlib/packages/option"
"std.result" = "../stdlib/packages/result"
"std.array" = "../stdlib/packages/array"
"std.test" = "../stdlib/packages/test"
"#,
    );

    RealStdlibWorkspace {
        app_root,
        app_uri: Url::from_file_path(app_path).expect("app path should convert to URI"),
        stdlib_root: temp.path().join("workspace").join("stdlib"),
    }
}

pub fn real_stdlib_interface_path(stdlib_root: &Path, package_dir: &str) -> PathBuf {
    stdlib_root
        .join("packages")
        .join(package_dir)
        .join(format!("std.{package_dir}.qi"))
}

fn write_real_stdlib_packages(temp: &TempDir) {
    write_package(
        temp,
        "core",
        include_str!("../../../../stdlib/packages/core/qlang.toml"),
        include_str!("../../../../stdlib/packages/core/std.core.qi"),
        "std.core.qi",
    );
    write_package(
        temp,
        "option",
        include_str!("../../../../stdlib/packages/option/qlang.toml"),
        include_str!("../../../../stdlib/packages/option/std.option.qi"),
        "std.option.qi",
    );
    write_package(
        temp,
        "result",
        include_str!("../../../../stdlib/packages/result/qlang.toml"),
        include_str!("../../../../stdlib/packages/result/std.result.qi"),
        "std.result.qi",
    );
    write_package(
        temp,
        "array",
        include_str!("../../../../stdlib/packages/array/qlang.toml"),
        include_str!("../../../../stdlib/packages/array/std.array.qi"),
        "std.array.qi",
    );
    write_package(
        temp,
        "test",
        include_str!("../../../../stdlib/packages/test/qlang.toml"),
        include_str!("../../../../stdlib/packages/test/std.test.qi"),
        "std.test.qi",
    );
}

fn write_package(
    temp: &TempDir,
    package_dir: &str,
    manifest: &str,
    interface: &str,
    interface_name: &str,
) {
    let base = format!("workspace/stdlib/packages/{package_dir}");
    temp.write(&format!("{base}/qlang.toml"), manifest);
    temp.write(&format!("{base}/{interface_name}"), interface);
}
