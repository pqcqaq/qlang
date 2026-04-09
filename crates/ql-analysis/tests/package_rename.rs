use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use ql_analysis::{RenameEdit, RenameResult, RenameTarget, SymbolKind, analyze_package};
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
fn package_analysis_keeps_direct_dependency_import_rename_closed() {
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
        None
    );
    assert_eq!(
        package.dependency_rename_in_source_at(source, use_position, "Settings"),
        Ok(None)
    );
}
