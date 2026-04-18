use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ql_analysis::{
    RenameEdit, RenameResult, RenameTarget, SymbolKind, analyze_package,
    analyze_package_dependencies,
};
use ql_span::Span;

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

#[test]
fn package_analysis_supports_dependency_import_alias_rename() {
    let temp = TempDir::new("ql-analysis-package-rename");
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

pub fn make() -> Cfg {
    let value: Cfg = Cfg { value: 1 }
    return value
}
"#;
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let use_position = nth_offset(source, "Cfg", 2);

    assert_eq!(
        package.dependency_prepare_rename_in_source_at(source, use_position),
        Some(RenameTarget {
            kind: SymbolKind::Import,
            name: "Cfg".to_owned(),
            span: Span::new(use_position, use_position + "Cfg".len()),
        })
    );
    assert_eq!(
        package.dependency_rename_in_source_at(source, use_position, "Settings"),
        Ok(Some(RenameResult {
            kind: SymbolKind::Import,
            old_name: "Cfg".to_owned(),
            new_name: "Settings".to_owned(),
            edits: vec![
                RenameEdit {
                    span: Span::new(
                        nth_offset(source, "Cfg", 1),
                        nth_offset(source, "Cfg", 1) + 3
                    ),
                    replacement: "Settings".to_owned(),
                },
                RenameEdit {
                    span: Span::new(
                        nth_offset(source, "Cfg", 2),
                        nth_offset(source, "Cfg", 2) + 3
                    ),
                    replacement: "Settings".to_owned(),
                },
                RenameEdit {
                    span: Span::new(
                        nth_offset(source, "Cfg", 3),
                        nth_offset(source, "Cfg", 3) + 3
                    ),
                    replacement: "Settings".to_owned(),
                },
                RenameEdit {
                    span: Span::new(
                        nth_offset(source, "Cfg", 4),
                        nth_offset(source, "Cfg", 4) + 3
                    ),
                    replacement: "Settings".to_owned(),
                },
            ],
        }))
    );
}

#[test]
fn package_analysis_supports_direct_dependency_import_rename() {
    let temp = TempDir::new("ql-analysis-package-direct-import-rename");
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
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let use_position = nth_offset(source, "Config", 2);

    assert_eq!(
        package.dependency_prepare_rename_in_source_at(source, use_position),
        Some(RenameTarget {
            kind: SymbolKind::Import,
            name: "Config".to_owned(),
            span: Span::new(use_position, use_position + "Config".len()),
        })
    );
    assert_eq!(
        package.dependency_rename_in_source_at(source, use_position, "Settings"),
        Ok(Some(RenameResult {
            kind: SymbolKind::Import,
            old_name: "Config".to_owned(),
            new_name: "Settings".to_owned(),
            edits: vec![
                RenameEdit {
                    span: Span::new(
                        nth_offset(source, "Config", 1),
                        nth_offset(source, "Config", 1) + "Config".len(),
                    ),
                    replacement: "Config as Settings".to_owned(),
                },
                RenameEdit {
                    span: Span::new(
                        nth_offset(source, "Config", 2),
                        nth_offset(source, "Config", 2) + "Config".len(),
                    ),
                    replacement: "Settings".to_owned(),
                },
                RenameEdit {
                    span: Span::new(
                        nth_offset(source, "Config", 3),
                        nth_offset(source, "Config", 3) + "Config".len(),
                    ),
                    replacement: "Settings".to_owned(),
                },
                RenameEdit {
                    span: Span::new(
                        nth_offset(source, "Config", 4),
                        nth_offset(source, "Config", 4) + "Config".len(),
                    ),
                    replacement: "Settings".to_owned(),
                },
            ],
        }))
    );
}

#[test]
fn package_analysis_supports_dependency_value_root_rename() {
    let temp = TempDir::new("ql-analysis-package-value-root-rename");
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
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let use_position = nth_offset(source, "current", 2);

    assert_eq!(
        package.dependency_prepare_rename_in_source_at(source, use_position),
        Some(RenameTarget {
            kind: SymbolKind::Local,
            name: "current".to_owned(),
            span: Span::new(use_position, use_position + "current".len()),
        })
    );
    assert_eq!(
        package.dependency_rename_in_source_at(source, use_position, "selected"),
        Ok(Some(RenameResult {
            kind: SymbolKind::Local,
            old_name: "current".to_owned(),
            new_name: "selected".to_owned(),
            edits: vec![
                RenameEdit {
                    span: Span::new(
                        nth_offset(source, "current", 1),
                        nth_offset(source, "current", 1) + "current".len(),
                    ),
                    replacement: "selected".to_owned(),
                },
                RenameEdit {
                    span: Span::new(
                        nth_offset(source, "current", 2),
                        nth_offset(source, "current", 2) + "current".len(),
                    ),
                    replacement: "selected".to_owned(),
                },
                RenameEdit {
                    span: Span::new(
                        nth_offset(source, "current", 3),
                        nth_offset(source, "current", 3) + "current".len(),
                    ),
                    replacement: "selected".to_owned(),
                },
                RenameEdit {
                    span: Span::new(
                        nth_offset(source, "current", 4),
                        nth_offset(source, "current", 4) + "current".len(),
                    ),
                    replacement: "selected".to_owned(),
                },
            ],
        }))
    );
}

#[test]
fn package_analysis_preserves_dependency_value_root_rename_in_broken_source() {
    let temp = TempDir::new("ql-analysis-package-value-root-rename-parse-errors");
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
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should survive parse errors");
    let use_position = nth_offset(source, "current", 2);

    assert_eq!(
        package.dependency_prepare_rename_in_source_at(source, use_position),
        Some(RenameTarget {
            kind: SymbolKind::Local,
            name: "current".to_owned(),
            span: Span::new(use_position, use_position + "current".len()),
        })
    );
    assert_eq!(
        package.dependency_rename_in_source_at(source, use_position, "selected"),
        Ok(Some(RenameResult {
            kind: SymbolKind::Local,
            old_name: "current".to_owned(),
            new_name: "selected".to_owned(),
            edits: vec![
                RenameEdit {
                    span: Span::new(
                        nth_offset(source, "current", 1),
                        nth_offset(source, "current", 1) + "current".len(),
                    ),
                    replacement: "selected".to_owned(),
                },
                RenameEdit {
                    span: Span::new(
                        nth_offset(source, "current", 2),
                        nth_offset(source, "current", 2) + "current".len(),
                    ),
                    replacement: "selected".to_owned(),
                },
                RenameEdit {
                    span: Span::new(
                        nth_offset(source, "current", 3),
                        nth_offset(source, "current", 3) + "current".len(),
                    ),
                    replacement: "selected".to_owned(),
                },
                RenameEdit {
                    span: Span::new(
                        nth_offset(source, "current", 4),
                        nth_offset(source, "current", 4) + "current".len(),
                    ),
                    replacement: "selected".to_owned(),
                },
            ],
        }))
    );
}

#[test]
fn package_analysis_preserves_dependency_structured_root_indexed_member_rename_in_broken_source() {
    let temp = TempDir::new("ql-analysis-package-structured-root-indexed-member-rename");
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
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should survive parse errors");

    let field_use = nth_offset(source, "leaf", 1);
    assert_eq!(
        package.dependency_prepare_rename_in_source_at(source, field_use),
        Some(RenameTarget {
            kind: SymbolKind::Field,
            name: "leaf".to_owned(),
            span: Span::new(field_use, field_use + "leaf".len()),
        })
    );
    assert_eq!(
        package.dependency_rename_in_source_at(source, nth_offset(source, "leaf", 2), "branch"),
        Ok(Some(RenameResult {
            kind: SymbolKind::Field,
            old_name: "leaf".to_owned(),
            new_name: "branch".to_owned(),
            edits: vec![
                RenameEdit {
                    span: Span::new(
                        nth_offset(source, "leaf", 1),
                        nth_offset(source, "leaf", 1) + "leaf".len(),
                    ),
                    replacement: "branch".to_owned(),
                },
                RenameEdit {
                    span: Span::new(
                        nth_offset(source, "leaf", 2),
                        nth_offset(source, "leaf", 2) + "leaf".len(),
                    ),
                    replacement: "branch".to_owned(),
                },
            ],
        }))
    );

    let method_use = nth_offset(source, "leaf", 3);
    assert_eq!(
        package.dependency_prepare_rename_in_source_at(source, method_use),
        Some(RenameTarget {
            kind: SymbolKind::Method,
            name: "leaf".to_owned(),
            span: Span::new(method_use, method_use + "leaf".len()),
        })
    );
    assert_eq!(
        package.dependency_rename_in_source_at(source, nth_offset(source, "leaf", 4), "tip"),
        Ok(Some(RenameResult {
            kind: SymbolKind::Method,
            old_name: "leaf".to_owned(),
            new_name: "tip".to_owned(),
            edits: vec![
                RenameEdit {
                    span: Span::new(
                        nth_offset(source, "leaf", 3),
                        nth_offset(source, "leaf", 3) + "leaf".len(),
                    ),
                    replacement: "tip".to_owned(),
                },
                RenameEdit {
                    span: Span::new(
                        nth_offset(source, "leaf", 4),
                        nth_offset(source, "leaf", 4) + "leaf".len(),
                    ),
                    replacement: "tip".to_owned(),
                },
            ],
        }))
    );
}

#[test]
fn package_analysis_preserves_dependency_direct_question_unwrapped_member_rename_in_broken_source()
{
    let temp = TempDir::new("ql-analysis-package-direct-question-member-rename");
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
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should survive parse errors");

    let field_use = nth_offset(source, "leaf", 1);
    assert_eq!(
        package.dependency_prepare_rename_in_source_at(source, field_use),
        Some(RenameTarget {
            kind: SymbolKind::Field,
            name: "leaf".to_owned(),
            span: Span::new(field_use, field_use + "leaf".len()),
        })
    );
    assert_eq!(
        package.dependency_rename_in_source_at(source, nth_offset(source, "leaf", 2), "branch"),
        Ok(Some(RenameResult {
            kind: SymbolKind::Field,
            old_name: "leaf".to_owned(),
            new_name: "branch".to_owned(),
            edits: vec![
                RenameEdit {
                    span: Span::new(
                        nth_offset(source, "leaf", 1),
                        nth_offset(source, "leaf", 1) + "leaf".len(),
                    ),
                    replacement: "branch".to_owned(),
                },
                RenameEdit {
                    span: Span::new(
                        nth_offset(source, "leaf", 2),
                        nth_offset(source, "leaf", 2) + "leaf".len(),
                    ),
                    replacement: "branch".to_owned(),
                },
            ],
        }))
    );

    let method_use = nth_offset(source, "leaf", 3);
    assert_eq!(
        package.dependency_prepare_rename_in_source_at(source, method_use),
        Some(RenameTarget {
            kind: SymbolKind::Method,
            name: "leaf".to_owned(),
            span: Span::new(method_use, method_use + "leaf".len()),
        })
    );
    assert_eq!(
        package.dependency_rename_in_source_at(source, nth_offset(source, "leaf", 4), "tip"),
        Ok(Some(RenameResult {
            kind: SymbolKind::Method,
            old_name: "leaf".to_owned(),
            new_name: "tip".to_owned(),
            edits: vec![
                RenameEdit {
                    span: Span::new(
                        nth_offset(source, "leaf", 3),
                        nth_offset(source, "leaf", 3) + "leaf".len(),
                    ),
                    replacement: "tip".to_owned(),
                },
                RenameEdit {
                    span: Span::new(
                        nth_offset(source, "leaf", 4),
                        nth_offset(source, "leaf", 4) + "leaf".len(),
                    ),
                    replacement: "tip".to_owned(),
                },
            ],
        }))
    );
}

#[test]
fn package_analysis_preserves_dependency_local_method_result_value_root_rename_in_broken_source() {
    let temp = TempDir::new("ql-analysis-package-local-method-result-value-root-rename");
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
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should survive parse errors");
    let use_position = nth_offset(source, "current", 2);

    assert_eq!(
        package.dependency_prepare_rename_in_source_at(source, use_position),
        Some(RenameTarget {
            kind: SymbolKind::Local,
            name: "current".to_owned(),
            span: Span::new(use_position, use_position + "current".len()),
        })
    );
    assert_eq!(
        package.dependency_rename_in_source_at(source, use_position, "selected"),
        Ok(Some(RenameResult {
            kind: SymbolKind::Local,
            old_name: "current".to_owned(),
            new_name: "selected".to_owned(),
            edits: vec![
                RenameEdit {
                    span: Span::new(
                        nth_offset(source, "current", 1),
                        nth_offset(source, "current", 1) + "current".len(),
                    ),
                    replacement: "selected".to_owned(),
                },
                RenameEdit {
                    span: Span::new(
                        nth_offset(source, "current", 2),
                        nth_offset(source, "current", 2) + "current".len(),
                    ),
                    replacement: "selected".to_owned(),
                },
                RenameEdit {
                    span: Span::new(
                        nth_offset(source, "current", 3),
                        nth_offset(source, "current", 3) + "current".len(),
                    ),
                    replacement: "selected".to_owned(),
                },
                RenameEdit {
                    span: Span::new(
                        nth_offset(source, "current", 4),
                        nth_offset(source, "current", 4) + "current".len(),
                    ),
                    replacement: "selected".to_owned(),
                },
            ],
        }))
    );
}

#[test]
fn package_analysis_preserves_dependency_question_unwrapped_method_result_value_root_rename_in_broken_source()
 {
    let temp =
        TempDir::new("ql-analysis-package-question-unwrapped-method-result-value-root-rename");
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
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should survive parse errors");
    let use_position = nth_offset(source, "current", 2);

    assert_eq!(
        package.dependency_prepare_rename_in_source_at(source, use_position),
        Some(RenameTarget {
            kind: SymbolKind::Local,
            name: "current".to_owned(),
            span: Span::new(use_position, use_position + "current".len()),
        })
    );
    assert_eq!(
        package.dependency_rename_in_source_at(source, use_position, "selected"),
        Ok(Some(RenameResult {
            kind: SymbolKind::Local,
            old_name: "current".to_owned(),
            new_name: "selected".to_owned(),
            edits: vec![
                RenameEdit {
                    span: Span::new(
                        nth_offset(source, "current", 1),
                        nth_offset(source, "current", 1) + "current".len(),
                    ),
                    replacement: "selected".to_owned(),
                },
                RenameEdit {
                    span: Span::new(
                        nth_offset(source, "current", 2),
                        nth_offset(source, "current", 2) + "current".len(),
                    ),
                    replacement: "selected".to_owned(),
                },
                RenameEdit {
                    span: Span::new(
                        nth_offset(source, "current", 3),
                        nth_offset(source, "current", 3) + "current".len(),
                    ),
                    replacement: "selected".to_owned(),
                },
                RenameEdit {
                    span: Span::new(
                        nth_offset(source, "current", 4),
                        nth_offset(source, "current", 4) + "current".len(),
                    ),
                    replacement: "selected".to_owned(),
                },
            ],
        }))
    );
}

#[test]
fn package_analysis_preserves_dependency_question_unwrapped_method_result_member_rename_in_broken_source()
 {
    let temp = TempDir::new("ql-analysis-package-question-unwrapped-method-result-member-rename");
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
    temp.write("workspace/app/src/lib.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should survive parse errors");

    let use_position = nth_offset(source, "value", 1);
    assert_eq!(
        package.dependency_prepare_rename_in_source_at(source, use_position),
        Some(RenameTarget {
            kind: SymbolKind::Field,
            name: "value".to_owned(),
            span: Span::new(use_position, use_position + "value".len()),
        })
    );
    assert_eq!(
        package.dependency_rename_in_source_at(source, nth_offset(source, "value", 2), "count"),
        Ok(Some(RenameResult {
            kind: SymbolKind::Field,
            old_name: "value".to_owned(),
            new_name: "count".to_owned(),
            edits: vec![
                RenameEdit {
                    span: Span::new(
                        nth_offset(source, "value", 1),
                        nth_offset(source, "value", 1) + "value".len(),
                    ),
                    replacement: "count".to_owned(),
                },
                RenameEdit {
                    span: Span::new(
                        nth_offset(source, "value", 2),
                        nth_offset(source, "value", 2) + "value".len(),
                    ),
                    replacement: "count".to_owned(),
                },
            ],
        }))
    );
}

#[test]
fn package_analysis_rewrites_dependency_destructured_local_rename_definitions() {
    let temp = TempDir::new("ql-analysis-package-destructured-value-root-rename");
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
    temp.write("workspace/app/src/lib.ql", source);

    let package = analyze_package(&app_root).expect("package analysis should succeed");
    let use_position = nth_offset(source, "child", 2);

    assert_eq!(
        package.dependency_prepare_rename_in_source_at(source, use_position),
        Some(RenameTarget {
            kind: SymbolKind::Local,
            name: "child".to_owned(),
            span: Span::new(use_position, use_position + "child".len()),
        })
    );
    assert_eq!(
        package.dependency_rename_in_source_at(source, use_position, "current"),
        Ok(Some(RenameResult {
            kind: SymbolKind::Local,
            old_name: "child".to_owned(),
            new_name: "current".to_owned(),
            edits: vec![
                RenameEdit {
                    span: Span::new(
                        nth_offset(source, "child", 1),
                        nth_offset(source, "child", 1) + "child".len(),
                    ),
                    replacement: "child: current".to_owned(),
                },
                RenameEdit {
                    span: Span::new(
                        nth_offset(source, "child", 2),
                        nth_offset(source, "child", 2) + "child".len(),
                    ),
                    replacement: "current".to_owned(),
                },
                RenameEdit {
                    span: Span::new(
                        nth_offset(source, "child", 3),
                        nth_offset(source, "child", 3) + "child".len(),
                    ),
                    replacement: "current".to_owned(),
                },
            ],
        }))
    );
}

#[test]
fn package_analysis_preserves_same_named_local_dependency_variant_rename_in_broken_source() {
    let temp = TempDir::new("ql-analysis-package-same-named-local-dependency-variant-rename");
    let app_root = temp.path().join("workspace").join("packages").join("app");

    temp.write(
        "workspace/qlang.toml",
        r#"
[workspace]
members = ["packages/app"]
"#,
    );
    temp.write(
        "workspace/packages/app/qlang.toml",
        r#"
[package]
name = "app"

[dependencies]
alpha = { path = "../../vendor/alpha" }
beta = { path = "../../vendor/beta" }
"#,
    );
    temp.write(
        "workspace/vendor/alpha/qlang.toml",
        r#"
[package]
name = "core"
"#,
    );
    temp.write(
        "workspace/vendor/beta/qlang.toml",
        r#"
[package]
name = "core"
"#,
    );
    temp.write(
        "workspace/vendor/alpha/core.qi",
        r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.shared.alpha

pub enum Command {
    Retry(Int),
}
"#,
    );
    temp.write(
        "workspace/vendor/beta/core.qi",
        r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.shared.beta

pub enum Command {
    Retry(Int),
}
"#,
    );
    let source = r#"
package demo.app

use demo.shared.alpha.Command as Cmd
use demo.shared.beta.Command as OtherCmd

pub fn main() -> Int {
    let first = Cmd.Retry(1)
    let second = Cmd.Retry(2)
    let third = OtherCmd.Retry(
"#;
    temp.write("workspace/packages/app/src/main.ql", source);

    assert!(analyze_package(&app_root).is_err());
    let package = analyze_package_dependencies(&app_root)
        .expect("dependency-only package analysis should survive parse errors");

    let use_position = nth_offset(source, "Retry", 2);
    assert_eq!(
        package.dependency_prepare_rename_in_source_at(source, use_position),
        Some(RenameTarget {
            kind: SymbolKind::Variant,
            name: "Retry".to_owned(),
            span: Span::new(use_position, use_position + "Retry".len()),
        })
    );
    assert_eq!(
        package.dependency_rename_in_source_at(source, use_position, "Repeat"),
        Ok(Some(RenameResult {
            kind: SymbolKind::Variant,
            old_name: "Retry".to_owned(),
            new_name: "Repeat".to_owned(),
            edits: vec![
                RenameEdit {
                    span: Span::new(
                        nth_offset(source, "Retry", 1),
                        nth_offset(source, "Retry", 1) + "Retry".len(),
                    ),
                    replacement: "Repeat".to_owned(),
                },
                RenameEdit {
                    span: Span::new(
                        nth_offset(source, "Retry", 2),
                        nth_offset(source, "Retry", 2) + "Retry".len(),
                    ),
                    replacement: "Repeat".to_owned(),
                },
            ],
        }))
    );
}
