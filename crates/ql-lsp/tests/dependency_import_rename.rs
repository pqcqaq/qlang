use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ql_analysis::{analyze_package, analyze_package_dependencies};
use ql_lsp::bridge::{
    prepare_rename_for_dependency_imports, rename_for_dependency_imports, span_to_range,
};
use ql_span::Span;
use tower_lsp::lsp_types::{Position, PrepareRenameResponse, Url, WorkspaceEdit};

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(prefix: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();
        let path = env::temp_dir().join(format!("{prefix}-{unique}"));
        fs::create_dir_all(&path).expect("create temporary test directory");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn write(&self, relative: &str, contents: &str) -> PathBuf {
        let path = self.path.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent directory for temp file");
        }
        fs::write(&path, contents).expect("write temp file");
        path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn nth_offset(source: &str, needle: &str, occurrence: usize) -> usize {
    source
        .match_indices(needle)
        .nth(occurrence.saturating_sub(1))
        .map(|(start, _)| start)
        .expect("needle occurrence should exist")
}

fn offset_to_position(source: &str, offset: usize) -> Position {
    let prefix = &source[..offset];
    let line = prefix.bytes().filter(|byte| *byte == b'\n').count() as u32;
    let line_start = prefix.rfind('\n').map(|index| index + 1).unwrap_or(0);
    Position::new(line, prefix[line_start..].chars().count() as u32)
}

fn assert_workspace_edit(edit: WorkspaceEdit, uri: &Url, source: &str, expected: &[(Span, &str)]) {
    let changes = edit
        .changes
        .expect("workspace edit should contain direct changes");
    let edits = changes
        .get(uri)
        .expect("workspace edit should target source uri");
    assert_eq!(edits.len(), expected.len());
    for (edit, (span, replacement)) in edits.iter().zip(expected.iter()) {
        assert_eq!(edit.range, span_to_range(source, *span));
        assert_eq!(edit.new_text, *replacement);
    }
}

#[test]
fn dependency_direct_import_rename_bridge_supports_package_analysis() {
    let temp = TempDir::new("ql-lsp-dependency-import-rename");
    let app_root = temp.path().join("workspace").join("app");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Config {
    value: Int,
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    let source = r#"
package demo.app

use demo.dep.Config

pub fn make() -> Config {
    let value: Config = Config { value: 1 }
    return value
}
"#;
    let app_source = temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let use_offset = nth_offset(source, "Config", 2);
    let use_position = offset_to_position(source, use_offset);

    assert_eq!(
        prepare_rename_for_dependency_imports(source, &package, use_position),
        Some(PrepareRenameResponse::RangeWithPlaceholder {
            range: span_to_range(source, Span::new(use_offset, use_offset + 6)),
            placeholder: "Config".to_owned(),
        })
    );

    let uri = Url::from_file_path(&app_source).expect("source path should convert to file uri");
    let edit = rename_for_dependency_imports(&uri, source, &package, use_position, "Settings")
        .expect("rename should validate")
        .expect("dependency direct import rename should produce edits");
    assert_workspace_edit(
        edit,
        &uri,
        source,
        &[
            (
                Span::new(
                    nth_offset(source, "Config", 1),
                    nth_offset(source, "Config", 1) + 6,
                ),
                "Config as Settings",
            ),
            (
                Span::new(
                    nth_offset(source, "Config", 2),
                    nth_offset(source, "Config", 2) + 6,
                ),
                "Settings",
            ),
            (
                Span::new(
                    nth_offset(source, "Config", 3),
                    nth_offset(source, "Config", 3) + 6,
                ),
                "Settings",
            ),
            (
                Span::new(
                    nth_offset(source, "Config", 4),
                    nth_offset(source, "Config", 4) + 6,
                ),
                "Settings",
            ),
        ],
    );
}

#[test]
fn dependency_direct_import_rename_bridge_survives_parse_errors() {
    let temp = TempDir::new("ql-lsp-dependency-import-rename-parse-errors");
    let app_root = temp.path().join("workspace").join("app");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Config {
    value: Int,
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    let source = r#"
package demo.app

use demo.dep.Config

pub fn make() -> Config {
    let value: Config = Config { value: 1
    return value
}
"#;
    let app_source = temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should survive parse errors");
    let use_offset = nth_offset(source, "Config", 2);
    let use_position = offset_to_position(source, use_offset);

    assert_eq!(
        prepare_rename_for_dependency_imports(source, &package, use_position),
        Some(PrepareRenameResponse::RangeWithPlaceholder {
            range: span_to_range(source, Span::new(use_offset, use_offset + 6)),
            placeholder: "Config".to_owned(),
        })
    );

    let uri = Url::from_file_path(&app_source).expect("source path should convert to file uri");
    let edit = rename_for_dependency_imports(&uri, source, &package, use_position, "Settings")
        .expect("rename should validate")
        .expect("broken-source dependency direct import rename should produce edits");
    assert_workspace_edit(
        edit,
        &uri,
        source,
        &[
            (
                Span::new(
                    nth_offset(source, "Config", 1),
                    nth_offset(source, "Config", 1) + 6,
                ),
                "Config as Settings",
            ),
            (
                Span::new(
                    nth_offset(source, "Config", 2),
                    nth_offset(source, "Config", 2) + 6,
                ),
                "Settings",
            ),
            (
                Span::new(
                    nth_offset(source, "Config", 3),
                    nth_offset(source, "Config", 3) + 6,
                ),
                "Settings",
            ),
            (
                Span::new(
                    nth_offset(source, "Config", 4),
                    nth_offset(source, "Config", 4) + 6,
                ),
                "Settings",
            ),
        ],
    );
}

#[test]
fn dependency_grouped_direct_import_rename_bridge_survives_semantic_errors() {
    let temp = TempDir::new("ql-lsp-dependency-grouped-import-rename");
    let app_root = temp.path().join("workspace").join("app");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Config {
    value: Int,
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    let source = r#"
package demo.app

use demo.dep.{Config}

pub fn make() -> Config {
    let value: Config = Config { value: 1 }
    return 0
}
"#;
    let app_source = temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should survive semantic errors");
    let use_offset = nth_offset(source, "Config", 3);
    let use_position = offset_to_position(source, use_offset);

    assert_eq!(
        prepare_rename_for_dependency_imports(source, &package, use_position),
        Some(PrepareRenameResponse::RangeWithPlaceholder {
            range: span_to_range(source, Span::new(use_offset, use_offset + 6)),
            placeholder: "Config".to_owned(),
        })
    );

    let uri = Url::from_file_path(&app_source).expect("source path should convert to file uri");
    let edit = rename_for_dependency_imports(&uri, source, &package, use_position, "Settings")
        .expect("rename should validate")
        .expect("broken-source dependency direct import rename should produce edits");
    assert_workspace_edit(
        edit,
        &uri,
        source,
        &[
            (
                Span::new(
                    nth_offset(source, "Config", 1),
                    nth_offset(source, "Config", 1) + 6,
                ),
                "Config as Settings",
            ),
            (
                Span::new(
                    nth_offset(source, "Config", 2),
                    nth_offset(source, "Config", 2) + 6,
                ),
                "Settings",
            ),
            (
                Span::new(
                    nth_offset(source, "Config", 3),
                    nth_offset(source, "Config", 3) + 6,
                ),
                "Settings",
            ),
            (
                Span::new(
                    nth_offset(source, "Config", 4),
                    nth_offset(source, "Config", 4) + 6,
                ),
                "Settings",
            ),
        ],
    );
}

#[test]
fn dependency_value_root_rename_bridge_supports_package_analysis() {
    let temp = TempDir::new("ql-lsp-dependency-value-root-rename");
    let app_root = temp.path().join("workspace").join("app");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Config {
    value: Int,
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    let source = r#"
package demo.app

use demo.dep.Config as Cfg

pub fn read(config: Cfg) -> Int {
    let current = config
    return current.value + current.value
}
"#;
    let app_source = temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let use_offset = nth_offset(source, "current", 2);
    let use_position = offset_to_position(source, use_offset);

    assert_eq!(
        prepare_rename_for_dependency_imports(source, &package, use_position),
        Some(PrepareRenameResponse::RangeWithPlaceholder {
            range: span_to_range(source, Span::new(use_offset, use_offset + "current".len()),),
            placeholder: "current".to_owned(),
        })
    );

    let uri = Url::from_file_path(&app_source).expect("source path should convert to file uri");
    let edit = rename_for_dependency_imports(&uri, source, &package, use_position, "selected")
        .expect("rename should validate")
        .expect("dependency value root rename should produce edits");
    assert_workspace_edit(
        edit,
        &uri,
        source,
        &[
            (
                Span::new(
                    nth_offset(source, "current", 1),
                    nth_offset(source, "current", 1) + "current".len(),
                ),
                "selected",
            ),
            (
                Span::new(
                    nth_offset(source, "current", 2),
                    nth_offset(source, "current", 2) + "current".len(),
                ),
                "selected",
            ),
            (
                Span::new(
                    nth_offset(source, "current", 3),
                    nth_offset(source, "current", 3) + "current".len(),
                ),
                "selected",
            ),
            (
                Span::new(
                    nth_offset(source, "current", 4),
                    nth_offset(source, "current", 4) + "current".len(),
                ),
                "selected",
            ),
        ],
    );
}

#[test]
fn dependency_value_root_rename_bridge_survives_parse_errors() {
    let temp = TempDir::new("ql-lsp-dependency-value-root-rename-parse-errors");
    let app_root = temp.path().join("workspace").join("app");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Config {
    value: Int,
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    let source = r#"
package demo.app

use demo.dep.Config as Cfg

pub fn read(config: Cfg) -> Int {
    let current = config
    return current.value + current.value
"#;
    let app_source = temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should survive parse errors");
    let use_offset = nth_offset(source, "current", 2);
    let use_position = offset_to_position(source, use_offset);

    assert_eq!(
        prepare_rename_for_dependency_imports(source, &package, use_position),
        Some(PrepareRenameResponse::RangeWithPlaceholder {
            range: span_to_range(source, Span::new(use_offset, use_offset + "current".len())),
            placeholder: "current".to_owned(),
        })
    );

    let uri = Url::from_file_path(&app_source).expect("source path should convert to file uri");
    let edit = rename_for_dependency_imports(&uri, source, &package, use_position, "selected")
        .expect("rename should validate")
        .expect("broken-source dependency value root rename should produce edits");
    assert_workspace_edit(
        edit,
        &uri,
        source,
        &[
            (
                Span::new(
                    nth_offset(source, "current", 1),
                    nth_offset(source, "current", 1) + "current".len(),
                ),
                "selected",
            ),
            (
                Span::new(
                    nth_offset(source, "current", 2),
                    nth_offset(source, "current", 2) + "current".len(),
                ),
                "selected",
            ),
            (
                Span::new(
                    nth_offset(source, "current", 3),
                    nth_offset(source, "current", 3) + "current".len(),
                ),
                "selected",
            ),
            (
                Span::new(
                    nth_offset(source, "current", 4),
                    nth_offset(source, "current", 4) + "current".len(),
                ),
                "selected",
            ),
        ],
    );
}

#[test]
fn dependency_structured_root_indexed_member_rename_bridge_survives_parse_errors() {
    let temp = TempDir::new("ql-lsp-dependency-structured-root-indexed-member-rename");
    let app_root = temp.path().join("workspace").join("app");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Leaf {
    value: Int,
}

pub struct Child {
    leaf: Leaf,
}

impl Child {
    pub fn leaf(self) -> Leaf
}

pub fn maybe_children() -> Option[[Child; 2]]
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    let source = r#"
package demo.app

use demo.dep.maybe_children

pub fn read(flag: Bool) -> Int {
    let first = (if flag { maybe_children()? } else { maybe_children()? })[0].leaf.value
    let second = (match flag { true => maybe_children()?, false => maybe_children()? })[1].leaf.value
    let third = (if flag { maybe_children()? } else { maybe_children()? })[1].leaf()
    let fourth = (match flag { true => maybe_children()?, false => maybe_children()? })[0].leaf()
    let broken = maybe_children(
"#;
    let app_source = temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should survive parse errors");
    let uri = Url::from_file_path(&app_source).expect("source path should convert to file uri");

    let field_offset = nth_offset(source, "leaf", 1);
    let field_position = offset_to_position(source, field_offset);
    assert_eq!(
        prepare_rename_for_dependency_imports(source, &package, field_position),
        Some(PrepareRenameResponse::RangeWithPlaceholder {
            range: span_to_range(source, Span::new(field_offset, field_offset + "leaf".len())),
            placeholder: "leaf".to_owned(),
        })
    );

    let field_edit =
        rename_for_dependency_imports(&uri, source, &package, field_position, "branch")
            .expect("rename should validate")
            .expect("broken-source dependency field rename should produce edits");
    assert_workspace_edit(
        field_edit,
        &uri,
        source,
        &[
            (
                Span::new(
                    nth_offset(source, "leaf", 1),
                    nth_offset(source, "leaf", 1) + "leaf".len(),
                ),
                "branch",
            ),
            (
                Span::new(
                    nth_offset(source, "leaf", 2),
                    nth_offset(source, "leaf", 2) + "leaf".len(),
                ),
                "branch",
            ),
        ],
    );

    let method_offset = nth_offset(source, "leaf", 3);
    let method_position = offset_to_position(source, method_offset);
    assert_eq!(
        prepare_rename_for_dependency_imports(source, &package, method_position),
        Some(PrepareRenameResponse::RangeWithPlaceholder {
            range: span_to_range(
                source,
                Span::new(method_offset, method_offset + "leaf".len()),
            ),
            placeholder: "leaf".to_owned(),
        })
    );

    let method_edit = rename_for_dependency_imports(&uri, source, &package, method_position, "tip")
        .expect("rename should validate")
        .expect("broken-source dependency method rename should produce edits");
    assert_workspace_edit(
        method_edit,
        &uri,
        source,
        &[
            (
                Span::new(
                    nth_offset(source, "leaf", 3),
                    nth_offset(source, "leaf", 3) + "leaf".len(),
                ),
                "tip",
            ),
            (
                Span::new(
                    nth_offset(source, "leaf", 4),
                    nth_offset(source, "leaf", 4) + "leaf".len(),
                ),
                "tip",
            ),
        ],
    );
}

#[test]
fn dependency_direct_question_unwrapped_member_rename_bridge_survives_parse_errors() {
    let temp = TempDir::new("ql-lsp-dependency-direct-question-member-rename");
    let app_root = temp.path().join("workspace").join("app");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Leaf {
    value: Int,
}

pub struct Child {
    leaf: Leaf,
}

pub struct ErrInfo {
    code: Int,
}

pub struct Config {}

impl Config {
    pub fn child(self) -> Result[Child, ErrInfo]
}

impl Child {
    pub fn leaf(self) -> Leaf
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    let source = r#"
package demo.app

use demo.dep.Config as Cfg

pub fn read(config: Cfg) -> Int {
    let first = config.child()?.leaf.value
    let second = config.child()?.leaf.value
    let third = config.child()?.leaf()
    let fourth = config.child()?.leaf()
    let broken = config.child(
"#;
    let app_source = temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should survive parse errors");
    let uri = Url::from_file_path(&app_source).expect("source path should convert to file uri");

    let field_offset = nth_offset(source, "leaf", 1);
    let field_position = offset_to_position(source, field_offset);
    assert_eq!(
        prepare_rename_for_dependency_imports(source, &package, field_position),
        Some(PrepareRenameResponse::RangeWithPlaceholder {
            range: span_to_range(source, Span::new(field_offset, field_offset + "leaf".len())),
            placeholder: "leaf".to_owned(),
        })
    );

    let field_edit =
        rename_for_dependency_imports(&uri, source, &package, field_position, "branch")
            .expect("rename should validate")
            .expect("broken-source dependency field rename should produce edits");
    assert_workspace_edit(
        field_edit,
        &uri,
        source,
        &[
            (
                Span::new(
                    nth_offset(source, "leaf", 1),
                    nth_offset(source, "leaf", 1) + "leaf".len(),
                ),
                "branch",
            ),
            (
                Span::new(
                    nth_offset(source, "leaf", 2),
                    nth_offset(source, "leaf", 2) + "leaf".len(),
                ),
                "branch",
            ),
        ],
    );

    let method_offset = nth_offset(source, "leaf", 3);
    let method_position = offset_to_position(source, method_offset);
    assert_eq!(
        prepare_rename_for_dependency_imports(source, &package, method_position),
        Some(PrepareRenameResponse::RangeWithPlaceholder {
            range: span_to_range(
                source,
                Span::new(method_offset, method_offset + "leaf".len()),
            ),
            placeholder: "leaf".to_owned(),
        })
    );

    let method_edit = rename_for_dependency_imports(&uri, source, &package, method_position, "tip")
        .expect("rename should validate")
        .expect("broken-source dependency method rename should produce edits");
    assert_workspace_edit(
        method_edit,
        &uri,
        source,
        &[
            (
                Span::new(
                    nth_offset(source, "leaf", 3),
                    nth_offset(source, "leaf", 3) + "leaf".len(),
                ),
                "tip",
            ),
            (
                Span::new(
                    nth_offset(source, "leaf", 4),
                    nth_offset(source, "leaf", 4) + "leaf".len(),
                ),
                "tip",
            ),
        ],
    );
}

#[test]
fn dependency_local_method_result_value_root_rename_bridge_survives_parse_errors() {
    let temp = TempDir::new("ql-lsp-dependency-local-method-result-value-root-rename");
    let app_root = temp.path().join("workspace").join("app");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Child {
    value: Int,
}

pub struct ErrInfo {
    code: Int,
}

pub struct Config {}

impl Config {
    pub fn child(self) -> Result[Child, ErrInfo]
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    let source = r#"
package demo.app

use demo.dep.Config as Cfg

pub fn read(config: Cfg) -> Int {
    let current = config.child()?
    return current.value + current.value + current.ge(
"#;
    let app_source = temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should survive parse errors");
    let use_offset = nth_offset(source, "current", 2);
    let use_position = offset_to_position(source, use_offset);

    assert_eq!(
        prepare_rename_for_dependency_imports(source, &package, use_position),
        Some(PrepareRenameResponse::RangeWithPlaceholder {
            range: span_to_range(source, Span::new(use_offset, use_offset + "current".len()),),
            placeholder: "current".to_owned(),
        })
    );

    let uri = Url::from_file_path(&app_source).expect("source path should convert to file uri");
    let edit = rename_for_dependency_imports(&uri, source, &package, use_position, "selected")
        .expect("rename should validate")
        .expect("broken-source dependency local method-result rename should produce edits");
    assert_workspace_edit(
        edit,
        &uri,
        source,
        &[
            (
                Span::new(
                    nth_offset(source, "current", 1),
                    nth_offset(source, "current", 1) + "current".len(),
                ),
                "selected",
            ),
            (
                Span::new(
                    nth_offset(source, "current", 2),
                    nth_offset(source, "current", 2) + "current".len(),
                ),
                "selected",
            ),
            (
                Span::new(
                    nth_offset(source, "current", 3),
                    nth_offset(source, "current", 3) + "current".len(),
                ),
                "selected",
            ),
            (
                Span::new(
                    nth_offset(source, "current", 4),
                    nth_offset(source, "current", 4) + "current".len(),
                ),
                "selected",
            ),
        ],
    );
}

#[test]
fn dependency_question_unwrapped_method_result_value_root_rename_bridge_survives_parse_errors() {
    let temp = TempDir::new("ql-lsp-dependency-question-unwrapped-method-result-value-root-rename");
    let app_root = temp.path().join("workspace").join("app");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Leaf {
    value: Int,
}

pub struct Child {
    leaf: Leaf,
}

pub struct ErrInfo {
    code: Int,
}

pub struct Config {}

impl Config {
    pub fn child(self) -> Result[Child, ErrInfo]
}

impl Child {
    pub fn leaf(self) -> Leaf
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    let source = r#"
package demo.app

use demo.dep.Config as Cfg

pub fn read(config: Cfg) -> Int {
    let current = config.child()?.leaf()
    return current.value + current.value + current.ge(
"#;
    let app_source = temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should survive parse errors");
    let use_offset = nth_offset(source, "current", 2);
    let use_position = offset_to_position(source, use_offset);

    assert_eq!(
        prepare_rename_for_dependency_imports(source, &package, use_position),
        Some(PrepareRenameResponse::RangeWithPlaceholder {
            range: span_to_range(source, Span::new(use_offset, use_offset + "current".len())),
            placeholder: "current".to_owned(),
        })
    );

    let uri = Url::from_file_path(&app_source).expect("source path should convert to file uri");
    let edit = rename_for_dependency_imports(&uri, source, &package, use_position, "selected")
        .expect("rename should validate")
        .expect(
            "broken-source dependency question-unwrapped method-result rename should produce edits",
        );
    assert_workspace_edit(
        edit,
        &uri,
        source,
        &[
            (
                Span::new(
                    nth_offset(source, "current", 1),
                    nth_offset(source, "current", 1) + "current".len(),
                ),
                "selected",
            ),
            (
                Span::new(
                    nth_offset(source, "current", 2),
                    nth_offset(source, "current", 2) + "current".len(),
                ),
                "selected",
            ),
            (
                Span::new(
                    nth_offset(source, "current", 3),
                    nth_offset(source, "current", 3) + "current".len(),
                ),
                "selected",
            ),
            (
                Span::new(
                    nth_offset(source, "current", 4),
                    nth_offset(source, "current", 4) + "current".len(),
                ),
                "selected",
            ),
        ],
    );
}

#[test]
fn dependency_question_unwrapped_method_result_member_rename_bridge_survives_parse_errors() {
    let temp = TempDir::new("ql-lsp-dependency-question-unwrapped-method-result-member-rename");
    let app_root = temp.path().join("workspace").join("app");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Leaf {
    value: Int,
}

pub struct Child {
    leaf: Leaf,
}

pub struct ErrInfo {
    code: Int,
}

pub struct Config {}

impl Config {
    pub fn child(self) -> Result[Child, ErrInfo]
}

impl Child {
    pub fn leaf(self) -> Leaf
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    let source = r#"
package demo.app

use demo.dep.Config as Cfg

pub fn read(config: Cfg) -> Int {
    let first = config.child()?.leaf().value
    let second = config.child()?.leaf().value
    let broken = config.child()?.leaf(
"#;
    let app_source = temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should survive parse errors");
    let use_offset = nth_offset(source, "value", 1);
    let use_position = offset_to_position(source, use_offset);

    assert_eq!(
        prepare_rename_for_dependency_imports(source, &package, use_position),
        Some(PrepareRenameResponse::RangeWithPlaceholder {
            range: span_to_range(source, Span::new(use_offset, use_offset + "value".len())),
            placeholder: "value".to_owned(),
        })
    );

    let uri = Url::from_file_path(&app_source).expect("source path should convert to file uri");
    let edit = rename_for_dependency_imports(&uri, source, &package, use_position, "count")
        .expect("rename should validate")
        .expect(
            "broken-source dependency question-unwrapped method-result member rename should produce edits",
        );
    assert_workspace_edit(
        edit,
        &uri,
        source,
        &[
            (
                Span::new(
                    nth_offset(source, "value", 1),
                    nth_offset(source, "value", 1) + "value".len(),
                ),
                "count",
            ),
            (
                Span::new(
                    nth_offset(source, "value", 2),
                    nth_offset(source, "value", 2) + "value".len(),
                ),
                "count",
            ),
        ],
    );
}

#[test]
fn dependency_destructured_value_root_rename_rewrites_shorthand_definitions() {
    let temp = TempDir::new("ql-lsp-dependency-destructured-value-root-rename");
    let app_root = temp.path().join("workspace").join("app");

    temp.write(
        "workspace/dep/qlang.toml",
        r#"
[package]
name = "dep"
"#,
    );
    temp.write(
        "workspace/dep/dep.qi",
        r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Child {
    value: Int,
}

pub struct Config {
    child: Child,
}
"#,
    );
    temp.write(
        "workspace/app/qlang.toml",
        r#"
[package]
name = "app"

[references]
packages = ["../dep"]
"#,
    );
    let source = r#"
package demo.app

use demo.dep.Config as Cfg

pub fn read(config: Cfg) -> Int {
    let Cfg { child } = config
    return child.value + child.value
}
"#;
    let app_source = temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let use_offset = nth_offset(source, "child", 2);
    let use_position = offset_to_position(source, use_offset);

    assert_eq!(
        prepare_rename_for_dependency_imports(source, &package, use_position),
        Some(PrepareRenameResponse::RangeWithPlaceholder {
            range: span_to_range(source, Span::new(use_offset, use_offset + "child".len())),
            placeholder: "child".to_owned(),
        })
    );

    let uri = Url::from_file_path(&app_source).expect("source path should convert to file uri");
    let edit = rename_for_dependency_imports(&uri, source, &package, use_position, "current")
        .expect("rename should validate")
        .expect("dependency destructured value root rename should produce edits");
    assert_workspace_edit(
        edit,
        &uri,
        source,
        &[
            (
                Span::new(
                    nth_offset(source, "child", 1),
                    nth_offset(source, "child", 1) + "child".len(),
                ),
                "child: current",
            ),
            (
                Span::new(
                    nth_offset(source, "child", 2),
                    nth_offset(source, "child", 2) + "child".len(),
                ),
                "current",
            ),
            (
                Span::new(
                    nth_offset(source, "child", 3),
                    nth_offset(source, "child", 3) + "child".len(),
                ),
                "current",
            ),
        ],
    );
}
