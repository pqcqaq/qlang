use tower_lsp::lsp_types::Url;

use crate::common::request::TempDir;

pub fn write_stdlib_compat_workspace(temp: &TempDir, app_source: &str) -> Url {
    let app_path = temp.write("workspace/app/src/main.ql", app_source);
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../option", "../result", "../array"]
"#,
    );
    temp.write(
        "workspace/option/qlang.toml",
        r#"
[package]
name = "std.option"
"#,
    );
    temp.write(
        "workspace/option/std.option.qi",
        r#"
// qlang interface v1
// package: std.option

// source: src/lib.ql
package std.option

pub enum Option[T] {
    Some(T),
    None,
}
pub enum IntOption {
    Some(Int),
    None,
}
pub fn some[T](value: T) -> Option[T]
pub fn some_int(value: Int) -> IntOption
"#,
    );
    temp.write(
        "workspace/result/qlang.toml",
        r#"
[package]
name = "std.result"
"#,
    );
    temp.write(
        "workspace/result/std.result.qi",
        r#"
// qlang interface v1
// package: std.result

// source: src/lib.ql
package std.result

pub enum Result[T, E] {
    Ok(T),
    Err(E),
}
pub enum IntResult {
    Ok(Int),
    Err(Int),
}
pub fn ok[T, E](value: T) -> Result[T, E]
pub fn ok_int(value: Int) -> IntResult
"#,
    );
    temp.write(
        "workspace/array/qlang.toml",
        r#"
[package]
name = "std.array"
"#,
    );
    temp.write(
        "workspace/array/std.array.qi",
        r#"
// qlang interface v1
// package: std.array

// source: src/lib.ql
package std.array

pub fn sum_int_array[N](values: [Int; N]) -> Int
pub fn sum3_int_array(values: [Int; 3]) -> Int
pub fn repeat3_array[T](value: T) -> [T; 3]
"#,
    );
    Url::from_file_path(app_path).expect("app path should convert to URI")
}
