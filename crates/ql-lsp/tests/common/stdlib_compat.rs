#![allow(dead_code)]

use std::path::{Path, PathBuf};

use ql_analysis::{PackageAnalysis, analyze_package_dependencies};
use tower_lsp::lsp_types::{
    CompletionItem as LspCompletionItem, CompletionItemTag, CompletionResponse, Documentation, Url,
};

use crate::common::request::TempDir;

pub trait StdlibCompatTempDir {
    fn path(&self) -> &Path;

    fn write(&self, relative: &str, contents: &str) -> PathBuf;
}

impl StdlibCompatTempDir for TempDir {
    fn path(&self) -> &Path {
        self.path()
    }

    fn write(&self, relative: &str, contents: &str) -> PathBuf {
        self.write(relative, contents)
    }
}

pub struct StdlibCompatWorkspace {
    pub app_root: PathBuf,
    pub app_uri: Url,
}

pub fn write_stdlib_compat_workspace(temp: &TempDir, app_source: &str) -> Url {
    write_stdlib_compat_workspace_paths(temp, app_source).app_uri
}

pub fn write_stdlib_compat_package_workspace<T: StdlibCompatTempDir>(
    temp: &T,
    app_source: &str,
) -> (PathBuf, PackageAnalysis) {
    let workspace = write_stdlib_compat_workspace_paths(temp, app_source);
    let package = analyze_package_dependencies(&workspace.app_root)
        .expect("dependency-only package analysis should succeed");
    (workspace.app_root, package)
}

pub fn write_stdlib_compat_workspace_paths<T: StdlibCompatTempDir>(
    temp: &T,
    app_source: &str,
) -> StdlibCompatWorkspace {
    let app_root = temp.path().join("workspace").join("app");
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
    let app_uri = Url::from_file_path(app_path).expect("app path should convert to URI");
    StdlibCompatWorkspace { app_root, app_uri }
}

pub fn completion_items(completion: CompletionResponse) -> Vec<LspCompletionItem> {
    match completion {
        CompletionResponse::Array(items) => items,
        CompletionResponse::List(list) => list.items,
    }
}

pub fn completion_item<'a>(items: &'a [LspCompletionItem], label: &str) -> &'a LspCompletionItem {
    items
        .iter()
        .find(|item| item.label == label)
        .unwrap_or_else(|| panic!("completion item `{label}` should exist"))
}

fn documentation_value(item: &LspCompletionItem) -> &str {
    match item
        .documentation
        .as_ref()
        .expect("completion item should include documentation")
    {
        Documentation::String(value) => value,
        Documentation::MarkupContent(markup) => markup.value.as_str(),
    }
}

#[allow(deprecated)]
pub fn assert_recommended_completion(item: &LspCompletionItem) {
    assert_eq!(item.tags, None);
    assert_eq!(item.deprecated, None);
    assert_eq!(item.sort_text, None);
}

#[allow(deprecated)]
pub fn assert_compat_completion(item: &LspCompletionItem) {
    assert_eq!(item.tags, Some(vec![CompletionItemTag::DEPRECATED]));
    assert_eq!(item.deprecated, Some(true));
    assert!(
        item.sort_text
            .as_deref()
            .is_some_and(|text| text.starts_with("zz_")),
        "compatibility completion should sort after recommended APIs: {item:#?}",
    );
    assert!(
        documentation_value(item).contains("Compatibility API"),
        "compatibility completion should document the migration path: {item:#?}",
    );
}
