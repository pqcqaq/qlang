    use super::{
        GotoTypeDefinitionResponse, auto_import_code_actions_for_source,
        completion_for_dependency_member_fields, completion_for_dependency_methods,
        completion_for_dependency_struct_fields, completion_for_dependency_variants,
        completion_options, dependency_definition_target_at, document_formatting_edits,
        document_highlights_for_analysis_at, fallback_document_highlights_for_package_at,
        fallback_document_highlights_for_package_at_with_open_docs, file_open_documents,
        import_missing_dependency_code_actions_for_position,
        local_source_dependency_target_with_analysis, package_analysis_for_path,
        prepare_rename_for_dependency_imports,
        prepare_rename_for_workspace_import_in_broken_source,
        prepare_rename_for_workspace_source_root_symbol_from_import,
        prepare_rename_for_workspace_source_root_symbol_from_import_in_broken_source,
        prepare_rename_for_workspace_source_root_symbol_from_import_in_broken_source_with_open_docs,
        prepare_rename_for_workspace_source_root_symbol_from_import_with_open_docs,
        rename_for_dependency_imports, rename_for_local_source_dependency_with_open_docs,
        rename_for_workspace_import_in_broken_source,
        rename_for_workspace_import_in_broken_source_with_open_docs,
        rename_for_workspace_source_dependency_with_open_docs,
        rename_for_workspace_source_root_symbol_from_import_in_broken_source_with_open_docs,
        rename_for_workspace_source_root_symbol_from_import_with_open_docs,
        rename_for_workspace_source_root_symbol_with_open_docs, same_dependency_definition_target,
        semantic_tokens_for_workspace_dependency_fallback,
        semantic_tokens_for_workspace_dependency_fallback_with_open_docs,
        semantic_tokens_for_workspace_package_analysis,
        semantic_tokens_for_workspace_package_analysis_with_open_docs,
        workspace_dependency_document_highlights,
        workspace_dependency_reference_locations_with_open_docs,
        workspace_import_document_highlights, workspace_import_document_highlights_with_open_docs,
        workspace_source_definition_for_dependency,
        workspace_source_definition_for_dependency_with_open_docs,
        workspace_source_definition_for_import,
        workspace_source_definition_for_import_in_broken_source,
        workspace_source_definition_for_import_in_broken_source_with_open_docs,
        workspace_source_definition_for_import_with_open_docs,
        workspace_source_dependency_prepare_rename_with_open_docs,
        workspace_source_hover_for_dependency,
        workspace_source_hover_for_dependency_with_open_docs, workspace_source_hover_for_import,
        workspace_source_hover_for_import_in_broken_source,
        workspace_source_hover_for_import_in_broken_source_with_open_docs,
        workspace_source_hover_for_import_with_open_docs,
        workspace_source_implementation_for_dependency_with_open_docs,
        workspace_source_member_field_completions, workspace_source_method_completions,
        workspace_source_method_completions_with_open_docs,
        workspace_source_method_implementation_for_dependency_with_open_docs,
        workspace_source_method_implementation_for_broken_source_with_open_docs,
        workspace_source_method_implementation_for_local_source_in_broken_source_with_open_docs,
        workspace_source_method_implementation_for_local_source_with_open_docs,
        workspace_source_references_for_dependency,
        workspace_source_references_for_dependency_in_broken_source,
        workspace_source_references_for_dependency_in_broken_source_with_open_docs,
        workspace_source_references_for_dependency_with_open_docs,
        workspace_source_references_for_import,
        workspace_source_references_for_import_in_broken_source,
        workspace_source_references_for_import_in_broken_source_with_open_docs,
        workspace_source_references_for_import_with_open_docs,
        workspace_source_references_for_root_symbol_with_open_docs,
        workspace_source_root_implementation_in_broken_source_with_open_docs,
        workspace_source_root_implementation_with_open_docs,
        workspace_source_struct_field_completions,
        workspace_source_trait_method_implementation_in_broken_source_with_open_docs,
        workspace_source_trait_method_implementation_with_open_docs,
        workspace_source_trait_method_references_with_open_docs,
        workspace_source_type_definition_for_dependency,
        workspace_source_type_definition_for_dependency_with_open_docs,
        workspace_source_type_definition_for_import,
        workspace_source_type_definition_for_import_in_broken_source,
        workspace_source_type_definition_for_import_with_open_docs,
        workspace_source_variant_completions, workspace_symbols_for_documents,
        workspace_symbols_for_documents_and_roots,
    };
    use crate::bridge::{implementation_for_analysis, semantic_tokens_legend, span_to_range};
    use ql_analysis::{RenameError, SymbolKind as AnalysisSymbolKind, analyze_source};
    use ql_diagnostics::UNRESOLVED_VALUE_CODE;
    use ql_span::Span;
    use std::env;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};
    use tower_lsp::lsp_types::request::GotoImplementationResponse;
    use tower_lsp::lsp_types::{
        CodeActionOrCommand, CompletionItemKind, CompletionResponse, Diagnostic,
        GotoDefinitionResponse, HoverContents, Location, NumberOrString, Position,
        PrepareRenameResponse, Range, SemanticTokenType, SemanticTokensResult, SymbolInformation,
        SymbolKind, TextEdit, Url, WorkspaceEdit,
    };

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
                fs::create_dir_all(parent).expect("create parent directories");
            }
            fs::write(&path, contents).expect("write file");
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

    fn nth_span(source: &str, needle: &str, occurrence: usize) -> Span {
        let start = nth_offset(source, needle, occurrence);
        Span::new(start, start + needle.len())
    }

    fn nth_offset_in_context(source: &str, needle: &str, context: &str, occurrence: usize) -> usize {
        let context_start = nth_offset(source, context, occurrence);
        let needle_start = context
            .match_indices(needle)
            .last()
            .map(|(start, _)| start)
            .expect("needle should exist inside context");
        context_start + needle_start
    }

    fn nth_span_in_context(source: &str, needle: &str, context: &str, occurrence: usize) -> Span {
        let start = nth_offset_in_context(source, needle, context, occurrence);
        Span::new(start, start + needle.len())
    }

    #[test]
    fn document_formatting_edits_replace_entire_document_when_qfmt_changes_source() {
        let source = "fn main()->Int{return 1}\n";
        let edits = document_formatting_edits(source).expect("formatting should succeed");

        assert_eq!(
            edits,
            vec![TextEdit::new(
                Range::new(Position::new(0, 0), Position::new(1, 0)),
                "fn main() -> Int {\n    return 1\n}\n".to_owned(),
            )]
        );
    }

    #[test]
    fn document_formatting_edits_return_empty_when_source_is_already_formatted() {
        let source = "fn main() -> Int {\n    return 1\n}\n";

        assert!(
            document_formatting_edits(source)
                .expect("formatting should succeed")
                .is_empty()
        );
    }

    #[test]
    fn document_formatting_edits_report_parse_errors_without_returning_edits() {
        let source = "fn main( {\n";
        let error = document_formatting_edits(source).expect_err("formatting should fail");

        assert!(
            error.contains("document formatting skipped because the document has parse errors"),
            "unexpected formatting error: {error}"
        );
        assert!(
            error.contains("expected parameter name"),
            "unexpected formatting parse detail: {error}"
        );
    }

    #[test]
    fn implementation_for_analysis_returns_scalar_for_trait_implementations() {
        let source = r#"
trait Runner {
    fn run(self) -> Int
}

struct Worker {}

impl Runner for Worker {
    fn run(self) -> Int {
        return 1
    }
}
"#;
        let analysis = analyze_source(source).expect("analysis should succeed");
        let uri = Url::parse("file:///test.ql").expect("uri should parse");

        let implementation = implementation_for_analysis(
            &uri,
            source,
            &analysis,
            offset_to_position(source, nth_offset(source, "Runner", 1)),
        )
        .expect("trait implementation should exist");

        let GotoDefinitionResponse::Scalar(location) = implementation else {
            panic!("single trait implementation should resolve to one location")
        };
        assert_eq!(location.uri, uri);
        assert_eq!(
            location.range,
            span_to_range(
                source,
                Span::new(
                    nth_offset(source, "impl Runner for Worker", 1),
                    source.rfind('}').expect("impl block should close") + 1,
                ),
            )
        );
    }

    #[test]
    fn implementation_for_analysis_returns_array_for_trait_method_implementations() {
        let source = r#"
trait Runner {
    fn run(self) -> Int
}

struct Worker {}
struct Helper {}

impl Runner for Worker {
    fn run(self) -> Int {
        return 1
    }
}

impl Runner for Helper {
    fn run(self) -> Int {
        return 2
    }
}
"#;
        let analysis = analyze_source(source).expect("analysis should succeed");
        let uri = Url::parse("file:///test.ql").expect("uri should parse");

        let implementation = implementation_for_analysis(
            &uri,
            source,
            &analysis,
            offset_to_position(source, nth_offset(source, "run", 1)),
        )
        .expect("trait method implementations should exist");

        let GotoDefinitionResponse::Array(locations) = implementation else {
            panic!("multiple trait method implementations should resolve to many locations")
        };
        assert_eq!(
            locations,
            vec![
                Location::new(
                    uri.clone(),
                    span_to_range(source, nth_span(source, "run", 2))
                ),
                Location::new(uri, span_to_range(source, nth_span(source, "run", 3))),
            ]
        );
    }

    #[test]
    fn implementation_for_analysis_returns_none_on_method_definition_site() {
        let source = r#"
struct Config {
    value: Int,
}

impl Config {
    fn get(self) -> Int {
        return self.value
    }
}

fn read(config: Config) -> Int {
    return config.get()
}
"#;
        let analysis = analyze_source(source).expect("analysis should succeed");
        let uri = Url::parse("file:///test.ql").expect("uri should parse");

        assert_eq!(
            implementation_for_analysis(
                &uri,
                source,
                &analysis,
                offset_to_position(source, nth_offset(source, "get", 1)),
            ),
            None,
        );
    }

    #[test]
    fn workspace_type_import_implementation_prefers_workspace_member_source_over_interface_artifact()
     {
        let temp = TempDir::new("ql-lsp-workspace-type-import-implementation");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.Config

pub fn main(value: Config) -> Config {
    return value
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub struct Config {
    value: Int,
}

impl Config {
    fn build(self) -> Int {
        return self.value
    }
}

extend Config {
    fn label(self) -> Int {
        return self.value
    }
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub struct Config {
    value: Int,
}
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        let analysis = analyze_source(&source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");

        let implementation = workspace_source_implementation_for_dependency_with_open_docs(
            &source,
            Some(&analysis),
            &package,
            &file_open_documents(vec![]),
            offset_to_position(&source, nth_offset(&source, "Config", 2)),
        )
        .expect("workspace import implementation should exist");

        let GotoDefinitionResponse::Array(locations) = implementation else {
            panic!("workspace import implementation should resolve to many locations")
        };
        assert_eq!(locations.len(), 2);
        assert!(
            locations.iter().all(|location| {
                location
                    .uri
                    .to_file_path()
                    .ok()
                    .and_then(|path| fs::canonicalize(path).ok())
                    == Some(
                        fs::canonicalize(&core_source_path)
                            .expect("core source path should canonicalize"),
                    )
            }),
            "all implementation locations should point at workspace source",
        );
        assert_eq!(
            locations[0].range.start,
            offset_to_position(
                &fs::read_to_string(&core_source_path)
                    .expect("core source should read")
                    .replace("\r\n", "\n"),
                nth_offset(
                    &fs::read_to_string(&core_source_path)
                        .expect("core source should read")
                        .replace("\r\n", "\n"),
                    "impl Config",
                    1
                )
            ),
        );
        assert_eq!(
            locations[1].range.start,
            offset_to_position(
                &fs::read_to_string(&core_source_path)
                    .expect("core source should read")
                    .replace("\r\n", "\n"),
                nth_offset(
                    &fs::read_to_string(&core_source_path)
                        .expect("core source should read")
                        .replace("\r\n", "\n"),
                    "extend Config",
                    1
                )
            ),
        );
    }

    #[test]
    fn workspace_dependency_non_import_positions_implementation_prefer_workspace_member_source_over_interface_artifact()
    {
        let temp = TempDir::new("ql-lsp-workspace-dependency-source-implementation-positions");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.Command as Cmd
use demo.core.Config as Cfg
use demo.core.Holder as Hold

pub fn main(config: Cfg) -> Int {
    let built = Cfg { value: 1, limit: 2 }
    let holder = Hold { child: config.clone_self() }
    let command = Cmd.Retry(1)
    return holder.child.value + built.value + command.unwrap_or(0)
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub enum Command {
    Retry(Int),
}

pub struct Config {
    value: Int,
    limit: Int,
}

pub struct Holder {
    child: Config,
}

impl Command {
    pub fn unwrap_or(self, fallback: Int) -> Int {
        match self {
            Command.Retry(value) => value,
        }
    }
}

impl Config {
    pub fn clone_self(self) -> Config {
        return self
    }
}

extend Config {
    pub fn extra(self) -> Int {
        return self.limit
    }
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub enum Command {
    Retry(Int),
}

pub struct Config {
    value: Int,
    limit: Int,
}

pub struct Holder {
    child: Config,
}

impl Command {
    pub fn unwrap_or(self, fallback: Int) -> Int
}

impl Config {
    pub fn clone_self(self) -> Config
}
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        let analysis = analyze_source(&source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let core_source = fs::read_to_string(&core_source_path)
            .expect("core source should read")
            .replace("\r\n", "\n");
        let expected_path =
            fs::canonicalize(&core_source_path).expect("core source path should canonicalize");

        for (needle, occurrence, expected_markers) in [
            ("built", 2usize, &["impl Config", "extend Config"][..]),
            ("Retry", 1usize, &["impl Command"][..]),
            ("child", 2usize, &["impl Config", "extend Config"][..]),
        ] {
            let implementation = workspace_source_implementation_for_dependency_with_open_docs(
                &source,
                Some(&analysis),
                &package,
                &file_open_documents(vec![]),
                offset_to_position(&source, nth_offset(&source, needle, occurrence)),
            )
            .unwrap_or_else(|| {
                panic!("workspace dependency implementation should exist for {needle}")
            });

            let locations = match implementation {
                GotoDefinitionResponse::Scalar(location) => vec![location],
                GotoDefinitionResponse::Array(locations) => locations,
                GotoDefinitionResponse::Link(_) => {
                    panic!("workspace dependency implementation should resolve to locations")
                }
            };
            assert_eq!(
                locations.len(),
                expected_markers.len(),
                "workspace dependency implementation should return all source impl blocks for {needle}",
            );
            for location in &locations {
                assert_eq!(
                    location
                        .uri
                        .to_file_path()
                        .expect("implementation URI should convert to a file path")
                        .canonicalize()
                        .expect("implementation path should canonicalize"),
                    expected_path,
                );
            }
            for marker in expected_markers {
                assert!(
                    locations.iter().any(|location| {
                        location.range.start
                            == offset_to_position(&core_source, nth_offset(&core_source, marker, 1))
                    }),
                    "workspace dependency implementation should include {marker} for {needle}",
                );
            }
        }
    }

    struct WorkspaceTraitImportImplementationFixture {
        _temp: TempDir,
        app_path: PathBuf,
        app_source: String,
        app_uri: Url,
        package: ql_analysis::PackageAnalysis,
        tools_path: PathBuf,
        tools_uri: Url,
    }

    fn setup_workspace_trait_import_implementation_fixture(
        prefix: &str,
        app_source: &str,
        tools_source: &str,
    ) -> WorkspaceTraitImportImplementationFixture {
        let temp = TempDir::new(prefix);
        let app_path = temp.write("workspace/packages/app/src/main.ql", app_source);
        let tools_path = temp.write("workspace/packages/tools/src/lib.ql", tools_source);
        temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub trait Runner {
    fn run(self) -> Int
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core", "packages/tools"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/tools/qlang.toml",
            r#"
[package]
name = "tools"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub trait Runner {
    fn run(self) -> Int
}
"#,
        );

        let app_source = fs::read_to_string(&app_path).expect("app source should read");
        let app_uri = Url::from_file_path(&app_path).expect("app source path should convert to URI");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let tools_uri =
            Url::from_file_path(&tools_path).expect("tools source path should convert to URI");

        WorkspaceTraitImportImplementationFixture {
            _temp: temp,
            app_path,
            app_source,
            app_uri,
            package,
            tools_path,
            tools_uri,
        }
    }

    #[test]
    fn workspace_trait_import_implementation_includes_workspace_consumer_impls() {
        let fixture = setup_workspace_trait_import_implementation_fixture(
            "ql-lsp-workspace-trait-import-implementation-consumers",
            r#"
package demo.app

use demo.core.Runner

struct AppWorker {}

impl Runner for AppWorker {
    fn run(self) -> Int {
        return 1
    }
}
"#,
            r#"
package demo.tools

use demo.core.Runner

struct ToolWorker {}

impl Runner for ToolWorker {
    fn run(self) -> Int {
        return 2
    }
}
"#,
        );
        let analysis =
            analyze_source(&fixture.app_source).expect("app source should analyze");

        let implementation = workspace_source_implementation_for_dependency_with_open_docs(
            &fixture.app_source,
            Some(&analysis),
            &fixture.package,
            &file_open_documents(vec![]),
            offset_to_position(
                &fixture.app_source,
                nth_offset(&fixture.app_source, "Runner", 2),
            ),
        )
        .expect("workspace trait implementation should exist");

        let GotoDefinitionResponse::Array(locations) = implementation else {
            panic!("workspace trait implementation should resolve to many locations")
        };
        assert_eq!(locations.len(), 2);
        let implementation_paths = locations
            .iter()
            .map(|location| {
                fs::canonicalize(
                    location
                        .uri
                        .to_file_path()
                        .expect("implementation URI should convert to a file path"),
                )
                .expect("implementation path should canonicalize")
            })
            .collect::<Vec<_>>();
        assert!(
            implementation_paths
                .contains(
                    &fs::canonicalize(&fixture.app_path).expect("app path should canonicalize")
                )
        );
        assert!(implementation_paths.contains(
            &fs::canonicalize(&fixture.tools_path).expect("tools source path should canonicalize")
        ));
    }

    #[test]
    fn workspace_trait_import_implementation_uses_broken_open_workspace_source_and_new_impl_blocks()
    {
        let fixture = setup_workspace_trait_import_implementation_fixture(
            "ql-lsp-workspace-trait-import-implementation-broken-open-docs",
            r#"
package demo.app

use demo.core.Runner

struct AppWorker {}

impl Runner for AppWorker {
    fn run(self) -> Int {
        return 1
    }
}
"#,
            r#"
package demo.tools

pub fn ready() -> Int {
    return 1
}
"#,
        );
        let analysis =
            analyze_source(&fixture.app_source).expect("app source should analyze");
        let open_tools_source = r#"
package demo.tools

use demo.core.Runner

struct ToolWorker {}

impl Runner for ToolWorker {
    fn run(self) -> Int {
        return 2
    }
}

pub fn broken() -> Int {
    return ToolWorker {
"#
        .to_owned();

        assert!(analyze_source(&open_tools_source).is_err());
        let disk_only = workspace_source_implementation_for_dependency_with_open_docs(
            &fixture.app_source,
            Some(&analysis),
            &fixture.package,
            &file_open_documents(vec![]),
            offset_to_position(
                &fixture.app_source,
                nth_offset(&fixture.app_source, "Runner", 2),
            ),
        )
        .expect("workspace trait implementation should exist");
        let GotoDefinitionResponse::Scalar(location) = disk_only else {
            panic!("disk-only workspace trait implementation should resolve to one location")
        };
        assert_eq!(location.uri, fixture.app_uri);

        let implementation = workspace_source_implementation_for_dependency_with_open_docs(
            &fixture.app_source,
            Some(&analysis),
            &fixture.package,
            &file_open_documents(vec![(
                fixture.tools_uri.clone(),
                open_tools_source.clone(),
            )]),
            offset_to_position(
                &fixture.app_source,
                nth_offset(&fixture.app_source, "Runner", 2),
            ),
        )
        .expect("workspace trait implementation should use broken open workspace source");

        let GotoDefinitionResponse::Array(locations) = implementation else {
            panic!("broken open-doc trait implementation should resolve to many locations")
        };
        assert_eq!(locations.len(), 2);
        assert_eq!(locations[0].uri, fixture.app_uri);
        assert_eq!(locations[1].uri, fixture.tools_uri);
        assert_eq!(
            locations[1].range.start,
            offset_to_position(
                &open_tools_source,
                nth_offset(&open_tools_source, "impl Runner for ToolWorker", 1),
            ),
        );
    }

    #[test]
    fn workspace_type_import_implementation_in_broken_current_source_uses_workspace_source() {
        let temp = TempDir::new("ql-lsp-workspace-type-import-implementation-broken-current");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.Config

pub fn main(value: Config) -> Config {
    return value
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub struct Config {
    value: Int,
}

impl Config {
    fn build(self) -> Int {
        return self.value
    }
}

extend Config {
    fn label(self) -> Int {
        return self.value
    }
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub struct Config {
    value: Int,
}
"#,
        );

        let source = r#"
package demo.app

use demo.core.Config

pub fn main(value: Config) -> Config {
    return Config {
"#
        .to_owned();
        assert!(analyze_source(&source).is_err());
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let core_source = fs::read_to_string(&core_source_path)
            .expect("core source should read")
            .replace("\r\n", "\n");
        let core_uri =
            Url::from_file_path(&core_source_path).expect("core source path should convert to URI");

        let implementation = workspace_source_implementation_for_dependency_with_open_docs(
            &source,
            None,
            &package,
            &file_open_documents(vec![(core_uri.clone(), core_source.clone())]),
            offset_to_position(&source, nth_offset(&source, "Config", 2)),
        )
        .expect("broken current workspace type import implementation should exist");

        let GotoDefinitionResponse::Array(locations) = implementation else {
            panic!("broken current workspace type import implementation should resolve to many locations")
        };
        assert_eq!(locations.len(), 2);
        assert!(locations.iter().all(|location| location.uri == core_uri));
        assert_eq!(
            locations[0].range.start,
            offset_to_position(&core_source, nth_offset(&core_source, "impl Config", 1)),
        );
        assert_eq!(
            locations[1].range.start,
            offset_to_position(&core_source, nth_offset(&core_source, "extend Config", 1)),
        );
    }

    #[test]
    fn workspace_trait_import_implementation_in_broken_current_source_uses_workspace_source() {
        let fixture = setup_workspace_trait_import_implementation_fixture(
            "ql-lsp-workspace-trait-import-implementation-broken-current",
            r#"
package demo.app

use demo.core.Runner

struct AppWorker {}

impl Runner for AppWorker {
    fn run(self) -> Int {
        return 1
    }
}
"#,
            r#"
package demo.tools

use demo.core.Runner

struct ToolWorker {}

impl Runner for ToolWorker {
    fn run(self) -> Int {
        return 2
    }
}
"#,
        );

        let source = r#"
package demo.app

use demo.core.Runner

struct AppWorker {}

impl Runner for AppWorker {
    fn run(self) -> Int {
        return 1
    }
}

pub fn broken() -> Int {
    return AppWorker {
"#
        .to_owned();
        assert!(analyze_source(&source).is_err());

        let implementation = workspace_source_implementation_for_dependency_with_open_docs(
            &source,
            None,
            &fixture.package,
            &file_open_documents(vec![]),
            offset_to_position(&source, nth_offset(&source, "Runner", 2)),
        )
        .expect("broken current workspace trait implementation should exist");

        let GotoDefinitionResponse::Array(locations) = implementation else {
            panic!("broken current workspace trait implementation should resolve to many locations")
        };
        assert_eq!(locations.len(), 2);
        let implementation_paths = locations
            .iter()
            .map(|location| {
                fs::canonicalize(
                    location
                        .uri
                        .to_file_path()
                        .expect("implementation URI should convert to a file path"),
                )
                .expect("implementation path should canonicalize")
            })
            .collect::<Vec<_>>();
        assert!(
            implementation_paths
                .contains(
                    &fs::canonicalize(&fixture.app_path).expect("app path should canonicalize")
                )
        );
        assert!(implementation_paths.contains(
            &fs::canonicalize(&fixture.tools_path).expect("tools path should canonicalize")
        ));
    }

    #[test]
    fn workspace_root_trait_implementation_includes_workspace_consumer_impls() {
        let temp = TempDir::new("ql-lsp-workspace-root-trait-implementation-consumers");
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub trait Runner {
    fn run(self) -> Int
}
"#,
        );
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.Runner

struct AppWorker {}

impl Runner for AppWorker {
    fn run(self) -> Int {
        return 1
    }
}
"#,
        );
        let tools_source_path = temp.write(
            "workspace/packages/tools/src/lib.ql",
            r#"
package demo.tools

use demo.core.Runner

struct ToolWorker {}

impl Runner for ToolWorker {
    fn run(self) -> Int {
        return 2
    }
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core", "packages/tools"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/tools/qlang.toml",
            r#"
[package]
name = "tools"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub trait Runner {
    fn run(self) -> Int
}
"#,
        );

        let source = fs::read_to_string(&core_source_path).expect("core source should read");
        let analysis = analyze_source(&source).expect("core source should analyze");
        let package =
            package_analysis_for_path(&core_source_path).expect("package analysis should succeed");
        let core_uri =
            Url::from_file_path(&core_source_path).expect("core source path should convert to URI");
        let app_source = fs::read_to_string(&app_path).expect("app source should read");
        let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let tools_source =
            fs::read_to_string(&tools_source_path).expect("tools source should read");
        let tools_uri = Url::from_file_path(&tools_source_path)
            .expect("tools source path should convert to URI");
        let open_docs = file_open_documents(vec![
            (core_uri.clone(), source.clone()),
            (app_uri, app_source),
            (tools_uri, tools_source),
        ]);

        let implementation = workspace_source_root_implementation_with_open_docs(
            &core_uri,
            &source,
            &analysis,
            &package,
            &open_docs,
            offset_to_position(&source, nth_offset(&source, "Runner", 1)),
        )
        .expect("workspace root trait implementation should exist");

        let GotoDefinitionResponse::Array(locations) = implementation else {
            panic!("workspace root trait implementation should resolve to many locations")
        };
        assert_eq!(locations.len(), 2);
        let implementation_paths = locations
            .iter()
            .map(|location| {
                fs::canonicalize(
                    location
                        .uri
                        .to_file_path()
                        .expect("implementation URI should convert to a file path"),
                )
                .expect("implementation path should canonicalize")
            })
            .collect::<Vec<_>>();
        assert!(
            implementation_paths
                .contains(&fs::canonicalize(&app_path).expect("app path should canonicalize"))
        );
        assert!(implementation_paths.contains(
            &fs::canonicalize(&tools_source_path).expect("tools source path should canonicalize")
        ));
    }

    #[test]
    fn workspace_root_trait_implementation_in_broken_current_source_still_uses_workspace_impls() {
        let temp = TempDir::new("ql-lsp-workspace-root-trait-implementation-broken-current");
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub trait Runner {
    fn run(self) -> Int
}
"#,
        );
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.Runner

struct AppWorker {}

impl Runner for AppWorker {
    fn run(self) -> Int {
        return 1
    }
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub trait Runner {
    fn run(self) -> Int
}
"#,
        );

        let package =
            package_analysis_for_path(&core_source_path).expect("package analysis should succeed");
        let core_uri =
            Url::from_file_path(&core_source_path).expect("core path should convert to URI");
        let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let app_source = fs::read_to_string(&app_path)
            .expect("app source should read")
            .replace("\r\n", "\n");
        let open_core_source = r#"
package demo.core

pub trait Runner {
    fn run(self) -> Int
}

pub fn broken() -> Int {
    return 0
"#
        .to_owned();

        assert!(analyze_source(&open_core_source).is_err());

        let implementation = workspace_source_root_implementation_in_broken_source_with_open_docs(
            &core_uri,
            &open_core_source,
            &package,
            &file_open_documents(vec![(core_uri.clone(), open_core_source.clone())]),
            offset_to_position(&open_core_source, nth_offset(&open_core_source, "Runner", 1)),
        )
        .expect("broken current root source should still provide implementation locations");

        let GotoDefinitionResponse::Scalar(location) = implementation else {
            panic!("single broken current root implementation should resolve to one location")
        };
        assert_eq!(location.uri, app_uri);
        assert_eq!(
            location.range.start,
            offset_to_position(&app_source, nth_offset(&app_source, "impl Runner for AppWorker", 1)),
        );
    }

    #[test]
    fn workspace_root_trait_implementation_uses_broken_open_workspace_source_and_new_impl_blocks()
    {
        let temp =
            TempDir::new("ql-lsp-workspace-root-trait-implementation-broken-open-docs");
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub trait Runner {
    fn run(self) -> Int
}
"#,
        );
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

pub fn main() -> Int {
    return 0
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub trait Runner {
    fn run(self) -> Int
}
"#,
        );

        let source = fs::read_to_string(&core_source_path).expect("core source should read");
        let analysis = analyze_source(&source).expect("core source should analyze");
        let package =
            package_analysis_for_path(&core_source_path).expect("package analysis should succeed");
        let core_uri =
            Url::from_file_path(&core_source_path).expect("core path should convert to URI");
        let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let open_app_source = r#"
package demo.app

use demo.core.Runner

struct AppWorker {}

impl Runner for AppWorker {
    fn run(self) -> Int {
        return 1
    }
}

pub fn broken() -> Int {
    return AppWorker {
"#
        .to_owned();

        assert!(analyze_source(&open_app_source).is_err());
        assert_eq!(
            workspace_source_root_implementation_with_open_docs(
                &core_uri,
                &source,
                &analysis,
                &package,
                &file_open_documents(vec![]),
                offset_to_position(&source, nth_offset(&source, "Runner", 1)),
            ),
            None,
            "disk-only implementation search should miss unsaved broken workspace impl blocks",
        );

        let implementation = workspace_source_root_implementation_with_open_docs(
            &core_uri,
            &source,
            &analysis,
            &package,
            &file_open_documents(vec![(app_uri.clone(), open_app_source.clone())]),
            offset_to_position(&source, nth_offset(&source, "Runner", 1)),
        )
        .expect("broken open workspace source should provide root trait impl blocks");

        let GotoDefinitionResponse::Scalar(location) = implementation else {
            panic!("single broken open-doc root trait implementation should resolve to one location")
        };
        assert_eq!(location.uri, app_uri);
        assert_eq!(
            location.range.start,
            offset_to_position(
                &open_app_source,
                nth_offset(&open_app_source, "impl Runner for AppWorker", 1),
            ),
        );
    }

    #[test]
    fn workspace_root_method_use_implementation_resolves_to_source_definition() {
        let temp = TempDir::new("ql-lsp-workspace-root-method-use-implementation");
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int {
        return self.value
    }
}

pub fn read(config: Config) -> Int {
    return config.get()
}
"#,
        );
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.Config

pub fn main(config: Config) -> Int {
    return config.get()
}
"#,
        );
        let jobs_source_path = temp.write(
            "workspace/packages/jobs/src/lib.ql",
            r#"
package demo.jobs

use demo.core.Config

pub fn run(config: Config) -> Int {
    return config.get()
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core", "packages/jobs"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/jobs/qlang.toml",
            r#"
[package]
name = "jobs"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int
}
"#,
        );

        let source = fs::read_to_string(&core_source_path).expect("core source should read");
        let analysis = analyze_source(&source).expect("core source should analyze");
        let package =
            package_analysis_for_path(&core_source_path).expect("package analysis should succeed");
        let core_uri =
            Url::from_file_path(&core_source_path).expect("core path should convert to URI");
        let app_source = fs::read_to_string(&app_path).expect("app source should read");
        let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let jobs_source = fs::read_to_string(&jobs_source_path).expect("jobs source should read");
        let jobs_uri =
            Url::from_file_path(&jobs_source_path).expect("jobs path should convert to URI");
        let open_docs = file_open_documents(vec![
            (core_uri.clone(), source.clone()),
            (app_uri, app_source),
            (jobs_uri, jobs_source),
        ]);

        let implementation =
            workspace_source_method_implementation_for_local_source_with_open_docs(
                &core_uri,
                &source,
                &analysis,
                &package,
                &open_docs,
                offset_to_position(&source, nth_offset(&source, "get", 2)),
            )
            .expect("workspace root method implementation should exist");

        assert_eq!(
            implementation,
            GotoImplementationResponse::Scalar(Location::new(
                core_uri,
                span_to_range(&source, nth_span(&source, "get", 1)),
            )),
        );
    }

    #[test]
    fn workspace_root_method_implementation_returns_none_on_definition_site_even_with_workspace_consumers()
     {
        let temp = TempDir::new("ql-lsp-workspace-root-method-definition-implementation");
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int {
        return self.value
    }
}

pub fn read(config: Config) -> Int {
    return config.get()
}
"#,
        );
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.Config

pub fn main(config: Config) -> Int {
    return config.get()
}
"#,
        );
        let jobs_source_path = temp.write(
            "workspace/packages/jobs/src/lib.ql",
            r#"
package demo.jobs

use demo.core.Config

pub fn run(config: Config) -> Int {
    return config.get()
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core", "packages/jobs"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/jobs/qlang.toml",
            r#"
[package]
name = "jobs"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int
}
"#,
        );

        let source = fs::read_to_string(&core_source_path).expect("core source should read");
        let analysis = analyze_source(&source).expect("core source should analyze");
        let package =
            package_analysis_for_path(&core_source_path).expect("package analysis should succeed");
        let core_uri =
            Url::from_file_path(&core_source_path).expect("core path should convert to URI");
        let app_source = fs::read_to_string(&app_path).expect("app source should read");
        let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let jobs_source = fs::read_to_string(&jobs_source_path).expect("jobs source should read");
        let jobs_uri =
            Url::from_file_path(&jobs_source_path).expect("jobs path should convert to URI");
        let open_docs = file_open_documents(vec![
            (core_uri.clone(), source.clone()),
            (app_uri, app_source),
            (jobs_uri, jobs_source),
        ]);

        assert_eq!(
            workspace_source_method_implementation_for_local_source_with_open_docs(
                &core_uri,
                &source,
                &analysis,
                &package,
                &open_docs,
                offset_to_position(&source, nth_offset(&source, "get", 1)),
            ),
            None,
        );
    }

    #[test]
    fn workspace_root_method_implementation_uses_broken_open_workspace_consumers() {
        let temp =
            TempDir::new("ql-lsp-workspace-root-method-implementation-broken-open-consumers");
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub struct Config {
    value: Int,
}

impl Config {
    pub fn pulse(self) -> Int {
        return self.value
    }
}

pub fn read(config: Config) -> Int {
    return config.pulse()
}
"#,
        );
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.Config

pub fn main(config: Config) -> Int {
    return 0
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub struct Config {
    value: Int,
}

impl Config {
    pub fn pulse(self) -> Int
}
"#,
        );

        let open_app_source = r#"
package demo.app

use demo.core.Config

pub fn main(config: Config) -> Int {
    return config.pulse(
"#;

        let source = fs::read_to_string(&core_source_path).expect("core source should read");
        let analysis = analyze_source(&source).expect("core source should analyze");
        let package =
            package_analysis_for_path(&core_source_path).expect("package analysis should succeed");
        let core_uri =
            Url::from_file_path(&core_source_path).expect("core path should convert to URI");
        let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let open_docs = file_open_documents(vec![
            (core_uri.clone(), source.clone()),
            (app_uri, open_app_source.to_owned()),
        ]);

        let implementation =
            workspace_source_method_implementation_for_local_source_with_open_docs(
                &core_uri,
                &source,
                &analysis,
                &package,
                &open_docs,
                offset_to_position(&source, nth_offset(&source, "pulse", 2)),
            )
            .expect("workspace root method implementation should use broken open consumers");

        assert_eq!(
            implementation,
            GotoImplementationResponse::Scalar(Location::new(
                core_uri,
                span_to_range(&source, nth_span(&source, "pulse", 1)),
            )),
        );
    }

    #[test]
    fn workspace_root_trait_method_call_implementation_aggregates_workspace_impl_methods() {
        let temp = TempDir::new("ql-lsp-workspace-root-trait-method-call-implementation");
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub trait Runner {
    fn run(self) -> Int
}

pub fn call(runner: Runner) -> Int {
    return runner.run()
}
"#,
        );
        let app_source_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.Runner

struct AppWorker {}

impl Runner for AppWorker {
    fn run(self) -> Int {
        return 1
    }
}
"#,
        );
        let tools_source_path = temp.write(
            "workspace/packages/tools/src/lib.ql",
            r#"
package demo.tools

use demo.core.Runner

struct ToolWorker {}

impl Runner for ToolWorker {
    fn run(self) -> Int {
        return 2
    }
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core", "packages/tools"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/tools/qlang.toml",
            r#"
[package]
name = "tools"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub trait Runner {
    fn run(self) -> Int
}
"#,
        );

        let source = fs::read_to_string(&core_source_path).expect("core source should read");
        let analysis = analyze_source(&source).expect("core source should analyze");
        let package =
            package_analysis_for_path(&core_source_path).expect("package analysis should succeed");
        let core_uri =
            Url::from_file_path(&core_source_path).expect("core path should convert to URI");
        let call_offset = nth_offset_in_context(&source, "run", "runner.run()", 1);

        let implementation = workspace_source_method_implementation_for_local_source_with_open_docs(
            &core_uri,
            &source,
            &analysis,
            &package,
            &file_open_documents(vec![]),
            offset_to_position(&source, call_offset),
        )
        .expect("workspace root trait call implementation should exist");

        let GotoDefinitionResponse::Array(locations) = implementation else {
            panic!("workspace root trait call implementation should resolve to many locations")
        };
        assert_eq!(locations.len(), 2);

        let app_source = fs::read_to_string(&app_source_path).expect("app source should read");
        let tools_source =
            fs::read_to_string(&tools_source_path).expect("tools source should read");

        assert!(locations.contains(&Location::new(
            Url::from_file_path(&app_source_path).expect("app path should convert to URI"),
            span_to_range(&app_source, nth_span_in_context(&app_source, "run", "fn run(self)", 1)),
        )));
        assert!(locations.contains(&Location::new(
            Url::from_file_path(&tools_source_path).expect("tools path should convert to URI"),
            span_to_range(
                &tools_source,
                nth_span_in_context(&tools_source, "run", "fn run(self)", 1),
            ),
        )));
    }

    #[test]
    fn workspace_root_concrete_trait_impl_method_call_stays_scalar() {
        let temp = TempDir::new("ql-lsp-workspace-root-concrete-trait-method-implementation");
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub trait Runner {
    fn run(self) -> Int
}

pub struct AppWorker {}

impl Runner for AppWorker {
    fn run(self) -> Int {
        return 1
    }
}

pub fn call(worker: AppWorker) -> Int {
    return worker.run()
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub trait Runner {
    fn run(self) -> Int
}

pub struct AppWorker {}

impl Runner for AppWorker {
    fn run(self) -> Int
}
"#,
        );

        let source = fs::read_to_string(&core_source_path).expect("core source should read");
        let analysis = analyze_source(&source).expect("core source should analyze");
        let package =
            package_analysis_for_path(&core_source_path).expect("package analysis should succeed");
        let core_uri =
            Url::from_file_path(&core_source_path).expect("core path should convert to URI");

        let implementation = workspace_source_method_implementation_for_local_source_with_open_docs(
            &core_uri,
            &source,
            &analysis,
            &package,
            &file_open_documents(vec![]),
            offset_to_position(&source, nth_offset_in_context(&source, "run", "worker.run()", 1)),
        )
        .expect("concrete trait impl method call should resolve");

        let GotoDefinitionResponse::Scalar(location) = implementation else {
            panic!("concrete trait impl method call should stay scalar")
        };
        assert_eq!(location.uri, core_uri);
        assert_eq!(
            location.range,
            span_to_range(&source, nth_span_in_context(&source, "run", "fn run(self)", 2)),
        );
    }

    #[test]
    fn workspace_root_method_implementation_in_broken_current_source_uses_local_fallback() {
        let temp = TempDir::new("ql-lsp-workspace-root-method-implementation-broken-current");
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub struct Config {
    value: Int,
}

impl Config {
    pub fn pulse(self) -> Int {
        return self.value
    }
}

pub fn read(config: Config) -> Int {
    return config.pulse()
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub struct Config {
    value: Int,
}

impl Config {
    pub fn pulse(self) -> Int
}
"#,
        );

        let core_uri =
            Url::from_file_path(&core_source_path).expect("core path should convert to URI");
        let open_core_source = r#"
package demo.core

pub struct Config {
    value: Int,
}

impl Config {
    pub fn pulse(self) -> Int {
        return self.value
    }
}

pub fn read(config: Config) -> Int {
    return config.pulse(
"#
        .to_owned();

        assert!(analyze_source(&open_core_source).is_err());

        let implementation =
            workspace_source_method_implementation_for_local_source_in_broken_source_with_open_docs(
                &core_uri,
                &open_core_source,
                offset_to_position(&open_core_source, nth_offset(&open_core_source, "pulse", 2)),
            )
            .expect("broken current root method call should resolve with a local fallback");

        assert_eq!(
            implementation,
            GotoImplementationResponse::Scalar(Location::new(
                core_uri,
                span_to_range(&open_core_source, nth_span(&open_core_source, "pulse", 1)),
            )),
        );
    }

    #[test]
    fn workspace_root_method_implementation_in_broken_current_source_stays_none_when_ambiguous() {
        let temp = TempDir::new("ql-lsp-workspace-root-method-implementation-broken-ambiguous");
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub struct Config {
    value: Int,
}

pub struct Other {
    value: Int,
}

impl Config {
    pub fn pulse(self) -> Int {
        return self.value
    }
}

impl Other {
    pub fn pulse(self) -> Int {
        return self.value
    }
}

pub fn read(config: Config) -> Int {
    return config.pulse()
}
"#,
        );
        let core_uri =
            Url::from_file_path(&core_source_path).expect("core path should convert to URI");
        let open_core_source = r#"
package demo.core

pub struct Config {
    value: Int,
}

pub struct Other {
    value: Int,
}

impl Config {
    pub fn pulse(self) -> Int {
        return self.value
    }
}

impl Other {
    pub fn pulse(self) -> Int {
        return self.value
    }
}

pub fn read(config: Config) -> Int {
    return config.pulse(
"#
        .to_owned();

        assert!(analyze_source(&open_core_source).is_err());

        let implementation =
            workspace_source_method_implementation_for_local_source_in_broken_source_with_open_docs(
                &core_uri,
                &open_core_source,
                offset_to_position(&open_core_source, nth_offset(&open_core_source, "pulse", 3)),
            );

        assert!(
            implementation.is_none(),
            "ambiguous broken current method calls should not guess an implementation",
        );
    }

    #[test]
    fn workspace_root_trait_method_call_implementation_in_broken_current_source_aggregates_workspace_impls(
    ) {
        let temp = TempDir::new("ql-lsp-workspace-root-trait-method-implementation-broken-current");
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub trait Runner {
    fn run(self) -> Int
}

pub fn call(runner: Runner) -> Int {
    return runner.run(
}
"#,
        );
        let app_source_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.Runner

struct AppWorker {}

impl Runner for AppWorker {
    fn run(self) -> Int {
        return 1
    }
}
"#,
        );
        let tools_source_path = temp.write(
            "workspace/packages/tools/src/lib.ql",
            r#"
package demo.tools

use demo.core.Runner

struct ToolWorker {}

impl Runner for ToolWorker {
    fn run(self) -> Int {
        return 2
    }
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core", "packages/tools"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/tools/qlang.toml",
            r#"
[package]
name = "tools"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub trait Runner {
    fn run(self) -> Int
}
"#,
        );

        let open_core_source = r#"
package demo.core

pub trait Runner {
    fn run(self) -> Int
}

pub fn call(runner: Runner) -> Int {
    return runner.run(
}
"#
        .to_owned();
        let core_uri =
            Url::from_file_path(&core_source_path).expect("core path should convert to URI");
        assert!(analyze_source(&open_core_source).is_err());
        let package =
            package_analysis_for_path(&core_source_path).expect("package analysis should succeed");

        let implementation = workspace_source_method_implementation_for_broken_source_with_open_docs(
            &core_uri,
            &open_core_source,
            &package,
            &file_open_documents(vec![]),
            offset_to_position(
                &open_core_source,
                nth_offset_in_context(&open_core_source, "run", "runner.run(", 1),
            ),
        )
        .expect("broken current root trait call should resolve implementations");

        let GotoImplementationResponse::Array(locations) = implementation else {
            panic!("broken current root trait call should resolve to many locations")
        };
        assert_eq!(locations.len(), 2);

        let app_source = fs::read_to_string(&app_source_path).expect("app source should read");
        let tools_source =
            fs::read_to_string(&tools_source_path).expect("tools source should read");
        assert!(locations.contains(&Location::new(
            Url::from_file_path(&app_source_path).expect("app path should convert to URI"),
            span_to_range(&app_source, nth_span_in_context(&app_source, "run", "fn run(self)", 1)),
        )));
        assert!(locations.contains(&Location::new(
            Url::from_file_path(&tools_source_path).expect("tools path should convert to URI"),
            span_to_range(
                &tools_source,
                nth_span_in_context(&tools_source, "run", "fn run(self)", 1),
            ),
        )));
    }

    struct WorkspaceTraitMethodFixture {
        _temp: TempDir,
        core_source: String,
        core_uri: Url,
        package: ql_analysis::PackageAnalysis,
        app_source: String,
        app_uri: Url,
        tools_source: Option<String>,
        tools_uri: Option<Url>,
    }

    fn setup_workspace_trait_method_fixture(
        prefix: &str,
        app_source: &str,
        tools_source: Option<&str>,
    ) -> WorkspaceTraitMethodFixture {
        let temp = TempDir::new(prefix);
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub trait Runner {
    fn run(self) -> Int
}
"#,
        );
        let app_source_path = temp.write("workspace/packages/app/src/main.ql", app_source);
        let tools_source_path = tools_source
            .map(|source| temp.write("workspace/packages/tools/src/lib.ql", source));

        temp.write(
            "workspace/qlang.toml",
            if tools_source_path.is_some() {
                r#"
[workspace]
members = ["packages/app", "packages/core", "packages/tools"]
"#
            } else {
                r#"
[workspace]
members = ["packages/app", "packages/core"]
"#
            },
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        if tools_source_path.is_some() {
            temp.write(
                "workspace/packages/tools/qlang.toml",
                r#"
[package]
name = "tools"

[references]
packages = ["../core"]
"#,
            );
        }
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub trait Runner {
    fn run(self) -> Int
}
"#,
        );

        let core_source = fs::read_to_string(&core_source_path).expect("core source should read");
        let core_uri =
            Url::from_file_path(&core_source_path).expect("core path should convert to URI");
        let package =
            package_analysis_for_path(&core_source_path).expect("package analysis should succeed");
        let app_source = fs::read_to_string(&app_source_path).expect("app source should read");
        let app_uri =
            Url::from_file_path(&app_source_path).expect("app path should convert to URI");
        let tools_source = tools_source_path
            .as_ref()
            .map(|path| fs::read_to_string(path).expect("tools source should read"));
        let tools_uri = tools_source_path
            .map(|path| Url::from_file_path(path).expect("tools path should convert to URI"));

        WorkspaceTraitMethodFixture {
            _temp: temp,
            core_source,
            core_uri,
            package,
            app_source,
            app_uri,
            tools_source,
            tools_uri,
        }
    }

    #[test]
    fn workspace_trait_method_implementation_includes_workspace_consumer_impl_methods() {
        let fixture = setup_workspace_trait_method_fixture(
            "ql-lsp-workspace-trait-method-implementation-consumers",
            r#"
package demo.app

use demo.core.Runner

struct AppWorker {}

impl Runner for AppWorker {
    fn run(self) -> Int {
        return 1
    }
}
"#,
            Some(
                r#"
package demo.tools

use demo.core.Runner

struct ToolWorker {}

impl Runner for ToolWorker {
    fn run(self) -> Int {
        return 2
    }
}
"#,
            ),
        );
        let analysis =
            analyze_source(&fixture.core_source).expect("core source should analyze");

        let implementation = workspace_source_trait_method_implementation_with_open_docs(
            &fixture.core_uri,
            &fixture.core_source,
            &analysis,
            &fixture.package,
            &file_open_documents(vec![]),
            offset_to_position(&fixture.core_source, nth_offset(&fixture.core_source, "run", 1)),
        )
        .expect("workspace trait method implementation should exist");

        let GotoDefinitionResponse::Array(locations) = implementation else {
            panic!("workspace trait method implementation should resolve to many locations")
        };
        assert_eq!(locations.len(), 2);

        assert!(locations.contains(&Location::new(
            fixture.app_uri.clone(),
            span_to_range(&fixture.app_source, nth_span(&fixture.app_source, "run", 1)),
        )));

        let tools_source = fixture
            .tools_source
            .as_ref()
            .expect("tools source should exist");
        let tools_uri = fixture.tools_uri.clone().expect("tools URI should exist");
        assert!(locations.contains(&Location::new(
            tools_uri,
            span_to_range(tools_source, nth_span(tools_source, "run", 1)),
        )));
    }

    struct WorkspaceTypeImportImplementationFixture {
        _temp: TempDir,
        app_source: String,
        package: ql_analysis::PackageAnalysis,
        core_uri: Url,
    }

    fn setup_workspace_type_import_implementation_fixture(
        prefix: &str,
        app_source: &str,
        core_source: &str,
    ) -> WorkspaceTypeImportImplementationFixture {
        let temp = TempDir::new(prefix);
        let app_path = temp.write("workspace/packages/app/src/main.ql", app_source);
        let core_source_path = temp.write("workspace/packages/core/src/lib.ql", core_source);
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub struct Config {
    value: Int,
}
"#,
        );

        let app_source = fs::read_to_string(&app_path).expect("app source should read");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let core_uri =
            Url::from_file_path(&core_source_path).expect("core source path should convert to URI");

        WorkspaceTypeImportImplementationFixture {
            _temp: temp,
            app_source,
            package,
            core_uri,
        }
    }

    #[test]
    fn workspace_type_import_implementation_prefers_open_workspace_source_and_new_impl_blocks() {
        let fixture = setup_workspace_type_import_implementation_fixture(
            "ql-lsp-workspace-type-import-implementation-open-docs",
            r#"
package demo.app

use demo.core.Config

pub fn main(value: Config) -> Config {
    return value
}
"#,
            r#"
package demo.core

pub struct Config {
    value: Int,
}

impl Config {
    fn build(self) -> Int {
        return self.value
    }
}
"#,
        );
        let analysis =
            analyze_source(&fixture.app_source).expect("app source should analyze");
        let open_core_source = r#"
package demo.core

pub struct Config {
    value: Int,
}

pub fn helper() -> Int {
    return 0
}

impl Config {
    fn build(self) -> Int {
        return self.value
    }
}

extend Config {
    fn label(self) -> Int {
        return self.value
    }
}
"#
        .to_owned();

        let implementation = workspace_source_implementation_for_dependency_with_open_docs(
            &fixture.app_source,
            Some(&analysis),
            &fixture.package,
            &file_open_documents(vec![(
                fixture.core_uri.clone(),
                open_core_source.clone(),
            )]),
            offset_to_position(&fixture.app_source, nth_offset(&fixture.app_source, "Config", 2)),
        )
        .expect("workspace import implementation should exist");

        let GotoDefinitionResponse::Array(locations) = implementation else {
            panic!("workspace import implementation should resolve to many locations")
        };
        assert_eq!(locations.len(), 2);
        assert!(locations
            .iter()
            .all(|location| location.uri == fixture.core_uri));
        assert_eq!(
            locations[0].range.start,
            offset_to_position(
                &open_core_source,
                nth_offset(&open_core_source, "impl Config", 1)
            ),
        );
        assert_eq!(
            locations[1].range.start,
            offset_to_position(
                &open_core_source,
                nth_offset(&open_core_source, "extend Config", 1)
            ),
        );
    }

    #[test]
    fn workspace_type_import_implementation_uses_broken_open_workspace_source_and_new_impl_blocks()
    {
        let fixture = setup_workspace_type_import_implementation_fixture(
            "ql-lsp-workspace-type-import-implementation-broken-open-docs",
            r#"
package demo.app

use demo.core.Config

pub fn main(value: Config) -> Config {
    return value
}
"#,
            r#"
package demo.core

pub struct Config {
    value: Int,
}
"#,
        );
        let analysis =
            analyze_source(&fixture.app_source).expect("app source should analyze");
        let open_core_source = r#"
package demo.core

pub struct Config {
    value: Int,
}

pub trait Runner {
    fn run(self) -> Int
}

impl Config {
    fn build(self) -> Int {
        return self.value
    }
}

extend Config {
    fn label(self) -> Int {
        return self.value
    }
}

impl Runner for Config {
    fn run(self) -> Int {
        return self.value
    }
}

pub fn broken() -> Int {
    return Config {
"#
        .to_owned();

        assert!(analyze_source(&open_core_source).is_err());

        let implementation = workspace_source_implementation_for_dependency_with_open_docs(
            &fixture.app_source,
            Some(&analysis),
            &fixture.package,
            &file_open_documents(vec![(
                fixture.core_uri.clone(),
                open_core_source.clone(),
            )]),
            offset_to_position(&fixture.app_source, nth_offset(&fixture.app_source, "Config", 2)),
        )
        .expect("workspace import implementation should use broken open workspace source");

        let GotoDefinitionResponse::Array(locations) = implementation else {
            panic!("workspace import implementation should resolve to many locations")
        };
        assert_eq!(locations.len(), 3);
        assert!(locations
            .iter()
            .all(|location| location.uri == fixture.core_uri));
        assert_eq!(
            locations[0].range.start,
            offset_to_position(
                &open_core_source,
                nth_offset(&open_core_source, "impl Config", 1)
            ),
        );
        assert_eq!(
            locations[1].range.start,
            offset_to_position(
                &open_core_source,
                nth_offset(&open_core_source, "extend Config", 1)
            ),
        );
        assert_eq!(
            locations[2].range.start,
            offset_to_position(
                &open_core_source,
                nth_offset(&open_core_source, "impl Runner for Config", 1)
            ),
        );
    }

    #[test]
    fn workspace_trait_method_implementation_prefers_open_workspace_source_and_new_impl_methods() {
        let fixture = setup_workspace_trait_method_fixture(
            "ql-lsp-workspace-trait-method-implementation-open-docs",
            r#"
package demo.app

use demo.core.Runner

struct AppWorker {}
"#,
            None,
        );
        let analysis =
            analyze_source(&fixture.core_source).expect("core source should analyze");
        let open_app_source = r#"
package demo.app

use demo.core.Runner

struct AppWorker {}

impl Runner for AppWorker {
    fn run(self) -> Int {
        return 1
    }
}
"#
        .to_owned();

        assert_eq!(
            workspace_source_trait_method_implementation_with_open_docs(
                &fixture.core_uri,
                &fixture.core_source,
                &analysis,
                &fixture.package,
                &file_open_documents(vec![]),
                offset_to_position(&fixture.core_source, nth_offset(&fixture.core_source, "run", 1)),
            ),
            None,
            "disk-only implementation search should miss unsaved workspace impl methods",
        );

        let implementation = workspace_source_trait_method_implementation_with_open_docs(
            &fixture.core_uri,
            &fixture.core_source,
            &analysis,
            &fixture.package,
            &file_open_documents(vec![(
                fixture.app_uri.clone(),
                open_app_source.clone(),
            )]),
            offset_to_position(&fixture.core_source, nth_offset(&fixture.core_source, "run", 1)),
        )
        .expect("open workspace source should provide trait method implementations");

        let GotoDefinitionResponse::Scalar(location) = implementation else {
            panic!("single open-doc trait method implementation should resolve to one location")
        };
        assert_eq!(location.uri, fixture.app_uri);
        assert_eq!(
            location.range.start,
            offset_to_position(&open_app_source, nth_offset(&open_app_source, "run", 1)),
        );
    }

    #[test]
    fn workspace_trait_method_implementation_in_broken_current_source_still_uses_workspace_impl_methods()
     {
        let fixture = setup_workspace_trait_method_fixture(
            "ql-lsp-workspace-trait-method-implementation-broken-current",
            r#"
package demo.app

use demo.core.Runner

struct AppWorker {}

impl Runner for AppWorker {
    fn run(self) -> Int {
        return 1
    }
}
"#,
            None,
        );
        let app_source = fixture.app_source.replace("\r\n", "\n");
        let open_core_source = r#"
package demo.core

pub trait Runner {
    fn run(self) -> Int
}

pub fn broken() -> Int {
    return 0
"#
        .to_owned();

        assert!(analyze_source(&open_core_source).is_err());

        let implementation =
            workspace_source_trait_method_implementation_in_broken_source_with_open_docs(
                &fixture.core_uri,
                &open_core_source,
                &fixture.package,
                &file_open_documents(vec![(
                    fixture.core_uri.clone(),
                    open_core_source.clone(),
                )]),
                offset_to_position(&open_core_source, nth_offset(&open_core_source, "run", 1)),
            )
            .expect("broken current trait source should still provide impl method locations");

        let GotoDefinitionResponse::Scalar(location) = implementation else {
            panic!("single broken current trait method implementation should resolve to one location")
        };
        assert_eq!(location.uri, fixture.app_uri);
        assert_eq!(
            location.range.start,
            offset_to_position(&app_source, nth_offset(&app_source, "run", 1)),
        );
    }

    #[test]
    fn workspace_trait_method_implementation_uses_broken_open_workspace_source_and_new_impl_methods()
     {
        let fixture = setup_workspace_trait_method_fixture(
            "ql-lsp-workspace-trait-method-implementation-broken-open-docs",
            r#"
package demo.app

use demo.core.Runner

struct AppWorker {}
"#,
            None,
        );
        let analysis =
            analyze_source(&fixture.core_source).expect("core source should analyze");
        let open_app_source = r#"
package demo.app

use demo.core.Runner

struct AppWorker {}

impl Runner for AppWorker {
    fn run(self) -> Int {
        return 1
    }
}

pub fn broken() -> Int {
    return AppWorker {
"#
        .to_owned();

        assert!(analyze_source(&open_app_source).is_err());

        let implementation = workspace_source_trait_method_implementation_with_open_docs(
            &fixture.core_uri,
            &fixture.core_source,
            &analysis,
            &fixture.package,
            &file_open_documents(vec![(
                fixture.app_uri.clone(),
                open_app_source.clone(),
            )]),
            offset_to_position(&fixture.core_source, nth_offset(&fixture.core_source, "run", 1)),
        )
        .expect("open broken workspace source should provide trait method implementations");

        let GotoDefinitionResponse::Scalar(location) = implementation else {
            panic!(
                "single broken open-doc trait method implementation should resolve to one location"
            )
        };
        assert_eq!(location.uri, fixture.app_uri);
        assert_eq!(
            location.range.start,
            offset_to_position(&open_app_source, nth_offset(&open_app_source, "run", 1)),
        );
    }

    #[test]
    fn workspace_trait_method_references_include_workspace_consumer_impl_methods() {
        let fixture = setup_workspace_trait_method_fixture(
            "ql-lsp-workspace-trait-method-references-consumers",
            r#"
package demo.app

use demo.core.Runner

struct AppWorker {}

impl Runner for AppWorker {
    fn run(self) -> Int {
        return 1
    }
}
"#,
            Some(
                r#"
package demo.tools

use demo.core.Runner

struct ToolWorker {}

impl Runner for ToolWorker {
    fn run(self) -> Int {
        return 2
    }
}
"#,
            ),
        );
        let core_analysis =
            analyze_source(&fixture.core_source).expect("core source should analyze");

        let references = workspace_source_trait_method_references_with_open_docs(
            &fixture.core_uri,
            &fixture.core_source,
            &core_analysis,
            &fixture.package,
            &file_open_documents(vec![]),
            offset_to_position(&fixture.core_source, nth_offset(&fixture.core_source, "run", 1)),
            true,
        )
        .expect("workspace trait method references should exist");

        assert_eq!(references.len(), 3);
        assert!(references.contains(&Location::new(
            fixture.core_uri.clone(),
            span_to_range(&fixture.core_source, nth_span(&fixture.core_source, "run", 1)),
        )));
        assert!(references.contains(&Location::new(
            fixture.app_uri.clone(),
            span_to_range(&fixture.app_source, nth_span(&fixture.app_source, "run", 1)),
        )));

        let tools_source = fixture
            .tools_source
            .as_ref()
            .expect("tools source should exist");
        let tools_uri = fixture.tools_uri.clone().expect("tools URI should exist");
        assert!(references.contains(&Location::new(
            tools_uri,
            span_to_range(tools_source, nth_span(tools_source, "run", 1)),
        )));
    }

    #[test]
    fn workspace_trait_method_references_prefer_open_workspace_source_and_new_impl_methods() {
        let fixture = setup_workspace_trait_method_fixture(
            "ql-lsp-workspace-trait-method-references-open-docs",
            r#"
package demo.app

use demo.core.Runner

struct AppWorker {}
"#,
            None,
        );
        let core_analysis =
            analyze_source(&fixture.core_source).expect("core source should analyze");
        let open_app_source = r#"
package demo.app

use demo.core.Runner

struct AppWorker {}

impl Runner for AppWorker {
    fn run(self) -> Int {
        return 1
    }
}
"#
        .to_owned();

        let disk_references = workspace_source_trait_method_references_with_open_docs(
            &fixture.core_uri,
            &fixture.core_source,
            &core_analysis,
            &fixture.package,
            &file_open_documents(vec![]),
            offset_to_position(&fixture.core_source, nth_offset(&fixture.core_source, "run", 1)),
            true,
        )
        .expect("disk references should still include the declaration");
        assert_eq!(disk_references.len(), 1);
        assert_eq!(
            disk_references[0].range.start,
            offset_to_position(&fixture.core_source, nth_offset(&fixture.core_source, "run", 1)),
        );

        let references = workspace_source_trait_method_references_with_open_docs(
            &fixture.core_uri,
            &fixture.core_source,
            &core_analysis,
            &fixture.package,
            &file_open_documents(vec![(
                fixture.app_uri.clone(),
                open_app_source.clone(),
            )]),
            offset_to_position(&fixture.core_source, nth_offset(&fixture.core_source, "run", 1)),
            true,
        )
        .expect("open workspace source should provide trait method references");

        assert_eq!(references.len(), 2);
        assert!(references.contains(&Location::new(
            fixture.core_uri,
            span_to_range(&fixture.core_source, nth_span(&fixture.core_source, "run", 1)),
        )));
        assert!(references.contains(&Location::new(
            fixture.app_uri,
            span_to_range(&open_app_source, nth_span(&open_app_source, "run", 1)),
        )));
    }

    fn offset_to_position(source: &str, offset: usize) -> Position {
        let prefix = &source[..offset];
        let line = prefix.bytes().filter(|byte| *byte == b'\n').count() as u32;
        let line_start = prefix.rfind('\n').map(|index| index + 1).unwrap_or(0);
        Position::new(line, prefix[line_start..].chars().count() as u32)
    }

    fn decode_semantic_tokens(
        tokens: &[tower_lsp::lsp_types::SemanticToken],
    ) -> Vec<(u32, u32, u32, u32)> {
        let mut line = 0u32;
        let mut start = 0u32;
        let mut decoded = Vec::new();

        for token in tokens {
            line += token.delta_line;
            if token.delta_line == 0 {
                start += token.delta_start;
            } else {
                start = token.delta_start;
            }
            decoded.push((line, start, token.length, token.token_type));
        }

        decoded
    }

    #[test]
    fn completion_options_trigger_on_member_access_dot() {
        let options = completion_options();
        assert_eq!(options.trigger_characters, Some(vec![".".to_owned()]));
    }

    fn setup_auto_import_workspace_fixture(temp: &TempDir, app_source: &str) -> (PathBuf, PathBuf) {
        let workspace_root = temp.path().join("workspace");
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[dependencies]
core = { path = "../core" }
"#,
        );
        let app_path = temp.write("workspace/packages/app/src/main.ql", app_source);
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn exported(value: Int) -> Int {
    return value
}
"#,
        );
        (workspace_root, app_path)
    }

    fn setup_auto_import_workspace_missing_dependency_fixture(
        temp: &TempDir,
        app_source: &str,
    ) -> (PathBuf, PathBuf, PathBuf, String) {
        let workspace_root = temp.path().join("workspace");
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        let app_manifest_source = r#"
[package]
name = "app"
"#
        .to_owned();
        let app_manifest_path =
            temp.write("workspace/packages/app/qlang.toml", &app_manifest_source);
        let app_path = temp.write("workspace/packages/app/src/main.ql", app_source);
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn exported(value: Int) -> Int {
    return value
}
"#,
        );
        (
            workspace_root,
            app_path,
            app_manifest_path,
            app_manifest_source,
        )
    }

    fn unresolved_symbol_diagnostic(
        source: &str,
        name: &str,
        code: &str,
        label: &str,
    ) -> Diagnostic {
        let start = nth_offset(source, name, 1);
        Diagnostic {
            range: Range::new(
                offset_to_position(source, start),
                offset_to_position(source, start + name.len()),
            ),
            severity: None,
            code: Some(NumberOrString::String(code.to_owned())),
            code_description: None,
            source: None,
            message: format!("unresolved {label} `{name}`"),
            related_information: None,
            tags: None,
            data: None,
        }
    }

    #[test]
    fn auto_import_code_actions_offer_workspace_member_source_imports_for_unresolved_values() {
        let temp = TempDir::new("ql-lsp-auto-import-workspace-member-source-value");
        let app_source = r#"package demo.app

pub fn main() -> Int {
    return exported(1)
}
"#;
        let (workspace_root, app_path) = setup_auto_import_workspace_fixture(&temp, app_source);
        let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let diagnostic =
            unresolved_symbol_diagnostic(app_source, "exported", UNRESOLVED_VALUE_CODE, "value");

        let actions = auto_import_code_actions_for_source(
            &app_uri,
            app_source,
            &[diagnostic.clone()],
            vec![(app_uri.clone(), app_source.to_owned())],
            &[workspace_root],
        );

        assert_eq!(actions.len(), 1, "actual actions: {actions:#?}");
        let action = match &actions[0] {
            CodeActionOrCommand::CodeAction(action) => action,
            other => panic!("expected code action, got {other:#?}"),
        };
        assert_eq!(action.title, "Import `demo.core.exported`");
        assert_eq!(action.diagnostics, Some(vec![diagnostic]));
        assert_workspace_edit(
            action
                .edit
                .clone()
                .expect("code action should contain workspace edit"),
            &app_uri,
            vec![TextEdit::new(
                Range::new(Position::new(1, 0), Position::new(1, 0)),
                "use demo.core.exported\n".to_owned(),
            )],
        );
    }

    #[test]
    fn auto_import_code_actions_skip_existing_exact_import_paths() {
        let temp = TempDir::new("ql-lsp-auto-import-skip-existing-import");
        let app_source = r#"package demo.app

use demo.core.{exported}

pub fn main() -> Int {
    return exported(1)
}
"#;
        let (workspace_root, app_path) = setup_auto_import_workspace_fixture(&temp, app_source);
        let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let diagnostic =
            unresolved_symbol_diagnostic(app_source, "exported", UNRESOLVED_VALUE_CODE, "value");

        let actions = auto_import_code_actions_for_source(
            &app_uri,
            app_source,
            &[diagnostic],
            vec![(app_uri.clone(), app_source.to_owned())],
            &[workspace_root],
        );

        assert!(actions.is_empty(), "actual actions: {actions:#?}");
    }

    #[test]
    fn auto_import_code_actions_add_workspace_dependency_for_missing_member_dependency() {
        let temp = TempDir::new("ql-lsp-auto-import-add-missing-workspace-dependency");
        let app_source = r#"package demo.app

pub fn main() -> Int {
    return exported(1)
}
"#;
        let (workspace_root, app_path, app_manifest_path, app_manifest_source) =
            setup_auto_import_workspace_missing_dependency_fixture(&temp, app_source);
        let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let app_manifest_uri =
            Url::from_file_path(&app_manifest_path).expect("manifest path should convert to URI");
        let diagnostic =
            unresolved_symbol_diagnostic(app_source, "exported", UNRESOLVED_VALUE_CODE, "value");

        let actions = auto_import_code_actions_for_source(
            &app_uri,
            app_source,
            &[diagnostic.clone()],
            vec![(app_uri.clone(), app_source.to_owned())],
            &[workspace_root],
        );

        assert_eq!(actions.len(), 1, "actual actions: {actions:#?}");
        let action = match &actions[0] {
            CodeActionOrCommand::CodeAction(action) => action,
            other => panic!("expected code action, got {other:#?}"),
        };
        assert_eq!(
            action.title,
            "Import `demo.core.exported` and add dependency `core`"
        );
        assert_eq!(action.diagnostics, Some(vec![diagnostic]));

        let changes = action
            .edit
            .clone()
            .expect("code action should contain workspace edit")
            .changes
            .expect("workspace edit should contain direct changes");
        assert_eq!(changes.len(), 2, "actual changes: {changes:#?}");
        assert_eq!(
            changes.get(&app_uri),
            Some(&vec![TextEdit::new(
                Range::new(Position::new(1, 0), Position::new(1, 0)),
                "use demo.core.exported\n".to_owned(),
            )]),
        );

        let manifest_edits = changes
            .get(&app_manifest_uri)
            .expect("workspace edit should update the app manifest");
        assert_eq!(
            manifest_edits.len(),
            1,
            "actual manifest edits: {manifest_edits:#?}"
        );
        assert_eq!(
            manifest_edits[0].range,
            span_to_range(
                &app_manifest_source,
                Span::new(0, app_manifest_source.len())
            )
        );
        assert!(
            manifest_edits[0]
                .new_text
                .contains("[dependencies]\ncore = \"../core\"\n"),
            "actual manifest edit: {:#?}",
            manifest_edits[0]
        );
    }

    #[test]
    fn import_missing_dependency_code_actions_offer_manifest_edit_for_explicit_workspace_import() {
        let temp = TempDir::new("ql-lsp-import-missing-dependency-explicit-workspace-import");
        let app_source = r#"package demo.app

use demo.core.exported as run

pub fn main() -> Int {
    return run(1)
}
"#;
        let (workspace_root, app_path, app_manifest_path, app_manifest_source) =
            setup_auto_import_workspace_missing_dependency_fixture(&temp, app_source);
        let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let app_manifest_uri =
            Url::from_file_path(&app_manifest_path).expect("manifest path should convert to URI");

        let actions = import_missing_dependency_code_actions_for_position(
            &app_uri,
            app_source,
            Position::new(2, 14),
            vec![(app_uri.clone(), app_source.to_owned())],
            &[workspace_root],
        );

        assert_eq!(actions.len(), 1, "actual actions: {actions:#?}");
        let action = match &actions[0] {
            CodeActionOrCommand::CodeAction(action) => action,
            other => panic!("expected code action, got {other:#?}"),
        };
        assert_eq!(
            action.title,
            "Add dependency `core` for `demo.core.exported`"
        );
        assert_eq!(action.diagnostics, None);

        let changes = action
            .edit
            .clone()
            .expect("code action should contain workspace edit")
            .changes
            .expect("workspace edit should contain direct changes");
        assert_eq!(changes.len(), 1, "actual changes: {changes:#?}");
        let manifest_edits = changes
            .get(&app_manifest_uri)
            .expect("workspace edit should update the app manifest");
        assert_eq!(
            manifest_edits.len(),
            1,
            "actual manifest edits: {manifest_edits:#?}"
        );
        assert_eq!(
            manifest_edits[0].range,
            span_to_range(
                &app_manifest_source,
                Span::new(0, app_manifest_source.len())
            )
        );
        assert!(
            manifest_edits[0]
                .new_text
                .contains("[dependencies]\ncore = \"../core\"\n"),
            "actual manifest edit: {:#?}",
            manifest_edits[0]
        );
    }

    #[test]
    fn import_missing_dependency_code_actions_skip_existing_workspace_dependency() {
        let temp = TempDir::new("ql-lsp-import-missing-dependency-skip-existing");
        let app_source = r#"package demo.app

use demo.core.exported as run

pub fn main() -> Int {
    return run(1)
}
"#;
        let (workspace_root, app_path) = setup_auto_import_workspace_fixture(&temp, app_source);
        let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");

        let actions = import_missing_dependency_code_actions_for_position(
            &app_uri,
            app_source,
            Position::new(2, 14),
            vec![(app_uri.clone(), app_source.to_owned())],
            &[workspace_root],
        );

        assert!(actions.is_empty(), "actual actions: {actions:#?}");
    }

    fn assert_workspace_edit_changes(edit: WorkspaceEdit, expected: Vec<(Url, Vec<TextEdit>)>) {
        let changes = edit
            .changes
            .expect("workspace edit should contain direct changes");
        let actual_uris = changes.keys().cloned().collect::<Vec<_>>();
        assert_eq!(
            changes.len(),
            expected.len(),
            "workspace edit targeted unexpected URIs: {actual_uris:?}",
        );
        for (uri, edits) in expected {
            let actual = changes
                .get(&uri)
                .unwrap_or_else(|| panic!("workspace edit should target {uri}"));
            assert_eq!(actual, &edits);
        }
    }

    fn assert_workspace_edit(edit: WorkspaceEdit, uri: &Url, expected: Vec<TextEdit>) {
        assert_workspace_edit_changes(edit, vec![(uri.clone(), expected)]);
    }

    fn assert_single_dependency_method_symbol(
        symbols: Vec<SymbolInformation>,
        name: &str,
        interface_path: &Path,
        line: u32,
        start: u32,
        end: u32,
        package_name: &str,
    ) {
        assert_eq!(
            symbols,
            vec![SymbolInformation {
                name: name.to_owned(),
                kind: SymbolKind::METHOD,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(
                        fs::canonicalize(interface_path)
                            .expect("dependency interface path should canonicalize"),
                    )
                    .expect("dependency interface path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        tower_lsp::lsp_types::Position::new(line, start),
                        tower_lsp::lsp_types::Position::new(line, end),
                    ),
                ),
                container_name: Some(package_name.to_owned()),
            }]
        );
    }

    fn assert_single_dependency_symbol(
        symbols: Vec<SymbolInformation>,
        name: &str,
        kind: SymbolKind,
        interface_path: &Path,
        line: u32,
        start: u32,
        end: u32,
        package_name: &str,
    ) {
        assert_eq!(
            symbols,
            vec![SymbolInformation {
                name: name.to_owned(),
                kind,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(
                        fs::canonicalize(interface_path)
                            .expect("dependency interface path should canonicalize"),
                    )
                    .expect("dependency interface path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        tower_lsp::lsp_types::Position::new(line, start),
                        tower_lsp::lsp_types::Position::new(line, end),
                    ),
                ),
                container_name: Some(package_name.to_owned()),
            }]
        );
    }

    fn assert_single_source_symbol(
        symbols: Vec<SymbolInformation>,
        name: &str,
        kind: SymbolKind,
        source_path: &Path,
        source: &str,
        occurrence: usize,
    ) {
        assert_eq!(
            symbols,
            vec![SymbolInformation {
                name: name.to_owned(),
                kind,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(source_path).expect("source path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        offset_to_position(source, nth_offset(source, name, occurrence)),
                        offset_to_position(
                            source,
                            nth_offset(source, name, occurrence) + name.len(),
                        ),
                    ),
                ),
                container_name: None,
            }]
        );
    }

    struct SameNamedDependencyMethodSymbolsFixture {
        workspace_root: PathBuf,
        open_path: PathBuf,
        dependency_source_path: PathBuf,
        dependency_source: String,
        dependency_interface_path: PathBuf,
    }

    struct SameNamedDependencyEnumSymbolsFixture {
        workspace_root: PathBuf,
        open_path: PathBuf,
        dependency_source_path: PathBuf,
        dependency_source: String,
        dependency_interface_path: PathBuf,
    }

    struct SameNamedDependencyInterfaceSymbolsFixture {
        workspace_root: PathBuf,
        open_path: PathBuf,
        dependency_interface_path: PathBuf,
    }

    fn setup_same_named_dependency_interface_symbols_broken_fixture(
        temp: &TempDir,
    ) -> SameNamedDependencyInterfaceSymbolsFixture {
        let workspace_root = temp.path().join("workspace");
        let open_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

pub fn main() -> Int {
    let broken: Int = "oops"
    return 0
}
"#,
        );

        temp.write(
            "workspace/vendor/dep-source/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        temp.write(
            "workspace/vendor/dep-source/src/lib.ql",
            r#"
package demo.dep.source

pub fn alpha() -> Int {
    return 1
}
"#,
        );
        temp.write(
            "workspace/vendor/dep-source/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep.source

pub fn alpha() -> Int
"#,
        );
        temp.write(
            "workspace/vendor/dep-interface/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        let dependency_interface_path = temp.write(
            "workspace/vendor/dep-interface/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep.interface

pub fn beta() -> Int
"#,
        );
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
alpha = { path = "../../vendor/dep-source" }
beta = { path = "../../vendor/dep-interface" }
"#,
        );

        SameNamedDependencyInterfaceSymbolsFixture {
            workspace_root,
            open_path,
            dependency_interface_path,
        }
    }

    fn setup_same_named_dependency_method_symbols_fixture(
        temp: &TempDir,
    ) -> SameNamedDependencyMethodSymbolsFixture {
        let workspace_root = temp.path().join("workspace");
        let open_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
pub fn main() -> Int {
    return 0
}
"#,
        );

        temp.write(
            "workspace/vendor/dep-source/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        let dependency_source_path = temp.write(
            "workspace/vendor/dep-source/src/lib.ql",
            r#"
package demo.dep.source

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int {
        return self.value
    }
}

pub trait Reader {
    fn poll(self) -> Int
}

pub struct Buffer {
    value: Int,
}

extend Buffer {
    pub fn twice(self) -> Int {
        return 2
    }
}
"#,
        );
        temp.write(
            "workspace/vendor/dep-source/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep.source

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int
}

pub trait Reader {
    fn poll(self) -> Int
}

pub struct Buffer {
    value: Int,
}

extend Buffer {
    pub fn twice(self) -> Int
}
"#,
        );
        temp.write(
            "workspace/vendor/dep-interface/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        let dependency_interface_path = temp.write(
            "workspace/vendor/dep-interface/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep.interface

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int
}

pub trait Reader {
    fn poll(self) -> Int
}

pub struct Buffer {
    value: Int,
}

extend Buffer {
    pub fn twice(self) -> Int
}
"#,
        );
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

[references]
packages = ["../../vendor/dep-source", "../../vendor/dep-interface"]
"#,
        );

        SameNamedDependencyMethodSymbolsFixture {
            workspace_root,
            open_path,
            dependency_source: fs::read_to_string(&dependency_source_path)
                .expect("dependency source should read"),
            dependency_source_path,
            dependency_interface_path,
        }
    }

    fn setup_same_named_dependency_method_symbols_local_fixture(
        temp: &TempDir,
    ) -> SameNamedDependencyMethodSymbolsFixture {
        let workspace_root = temp.path().join("workspace");
        let open_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
pub fn main() -> Int {
    return 0
}
"#,
        );

        temp.write(
            "workspace/vendor/dep-source/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        let dependency_source_path = temp.write(
            "workspace/vendor/dep-source/src/lib.ql",
            r#"
package demo.dep.source

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int {
        return self.value
    }
}

pub trait Reader {
    fn poll(self) -> Int
}

pub struct Buffer {
    value: Int,
}

extend Buffer {
    pub fn twice(self) -> Int {
        return 2
    }
}
"#,
        );
        temp.write(
            "workspace/vendor/dep-source/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep.source

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int
}

pub trait Reader {
    fn poll(self) -> Int
}

pub struct Buffer {
    value: Int,
}

extend Buffer {
    pub fn twice(self) -> Int
}
"#,
        );
        temp.write(
            "workspace/vendor/dep-interface/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        let dependency_interface_path = temp.write(
            "workspace/vendor/dep-interface/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep.interface

pub struct Config {
    value: Int,
}

pub trait Reader {
    fn poll(self) -> Int
}

pub struct Buffer {
    value: Int,
}
"#,
        );
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
alpha = { path = "../../vendor/dep-source" }
beta = { path = "../../vendor/dep-interface" }
"#,
        );

        SameNamedDependencyMethodSymbolsFixture {
            workspace_root,
            open_path,
            dependency_source: fs::read_to_string(&dependency_source_path)
                .expect("dependency source should read"),
            dependency_source_path,
            dependency_interface_path,
        }
    }

    fn setup_same_named_dependency_enum_symbols_fixture(
        temp: &TempDir,
    ) -> SameNamedDependencyEnumSymbolsFixture {
        let workspace_root = temp.path().join("workspace");
        let open_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
pub fn main() -> Int {
    return 0
}
"#,
        );
        let dependency_source_path = temp.write(
            "workspace/vendor/alpha/src/lib.ql",
            r#"
package demo.shared.alpha

pub enum Command {
    Retry(Int),
}
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
        let dependency_interface_path = temp.write(
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
        temp.write(
            "workspace/vendor/beta/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
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

        SameNamedDependencyEnumSymbolsFixture {
            workspace_root,
            open_path,
            dependency_source: fs::read_to_string(&dependency_source_path)
                .expect("dependency source should read"),
            dependency_source_path,
            dependency_interface_path,
        }
    }

    fn setup_same_named_dependency_enum_symbols_broken_fixture(
        temp: &TempDir,
    ) -> SameNamedDependencyEnumSymbolsFixture {
        let workspace_root = temp.path().join("workspace");
        let open_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.shared.alpha.Command as Cmd
use demo.shared.beta.Command as OtherCmd

pub fn main() -> Int {
    let first = Cmd.Retry(1)
    let second = Cmd.Retry(2)
    let third = OtherCmd.Retry(
"#,
        );
        let dependency_source_path = temp.write(
            "workspace/vendor/alpha/src/lib.ql",
            r#"
package demo.shared.alpha

pub enum Command {
    Retry(Int),
}
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
        let dependency_interface_path = temp.write(
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
        temp.write(
            "workspace/vendor/beta/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
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

        SameNamedDependencyEnumSymbolsFixture {
            workspace_root,
            open_path,
            dependency_source: fs::read_to_string(&dependency_source_path)
                .expect("dependency source should read"),
            dependency_source_path,
            dependency_interface_path,
        }
    }

    fn setup_same_named_dependency_method_symbols_broken_fixture(
        temp: &TempDir,
    ) -> SameNamedDependencyMethodSymbolsFixture {
        let workspace_root = temp.path().join("workspace");
        let open_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

pub fn main() -> Int {
    let broken: Int = "oops"
    return 0
}
"#,
        );

        temp.write(
            "workspace/vendor/dep-source/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        let dependency_source_path = temp.write(
            "workspace/vendor/dep-source/src/lib.ql",
            r#"
package demo.dep.source

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int {
        return self.value
    }
}

pub trait Reader {
    fn poll(self) -> Int
}

pub struct Buffer {
    value: Int,
}

extend Buffer {
    pub fn twice(self) -> Int {
        return 2
    }
}
"#,
        );
        temp.write(
            "workspace/vendor/dep-source/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep.source

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int
}

pub trait Reader {
    fn poll(self) -> Int
}

pub struct Buffer {
    value: Int,
}

extend Buffer {
    pub fn twice(self) -> Int
}
"#,
        );
        temp.write(
            "workspace/vendor/dep-interface/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        let dependency_interface_path = temp.write(
            "workspace/vendor/dep-interface/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep.interface

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int
}

pub trait Reader {
    fn poll(self) -> Int
}

pub struct Buffer {
    value: Int,
}

extend Buffer {
    pub fn twice(self) -> Int
}
"#,
        );
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
alpha = { path = "../../vendor/dep-source" }
beta = { path = "../../vendor/dep-interface" }
"#,
        );

        SameNamedDependencyMethodSymbolsFixture {
            workspace_root,
            open_path,
            dependency_source: fs::read_to_string(&dependency_source_path)
                .expect("dependency source should read"),
            dependency_source_path,
            dependency_interface_path,
        }
    }

    fn assert_source_and_dependency_method_symbols(
        symbols: Vec<SymbolInformation>,
        name: &str,
        source_path: &Path,
        source: &str,
        source_occurrence: usize,
        interface_path: &Path,
        line: u32,
        start: u32,
        end: u32,
        package_name: &str,
    ) {
        let source_symbol = SymbolInformation {
            name: name.to_owned(),
            kind: SymbolKind::METHOD,
            tags: None,
            deprecated: None,
            location: Location::new(
                Url::from_file_path(source_path).expect("source path should convert to URI"),
                tower_lsp::lsp_types::Range::new(
                    offset_to_position(source, nth_offset(source, name, source_occurrence)),
                    offset_to_position(
                        source,
                        nth_offset(source, name, source_occurrence) + name.len(),
                    ),
                ),
            ),
            container_name: None,
        };
        let dependency_symbol = SymbolInformation {
            name: name.to_owned(),
            kind: SymbolKind::METHOD,
            tags: None,
            deprecated: None,
            location: Location::new(
                Url::from_file_path(
                    fs::canonicalize(interface_path)
                        .expect("dependency interface path should canonicalize"),
                )
                .expect("dependency interface path should convert to URI"),
                tower_lsp::lsp_types::Range::new(
                    tower_lsp::lsp_types::Position::new(line, start),
                    tower_lsp::lsp_types::Position::new(line, end),
                ),
            ),
            container_name: Some(package_name.to_owned()),
        };

        assert_eq!(symbols.len(), 2, "actual symbols: {symbols:#?}");
        assert!(
            symbols.contains(&source_symbol),
            "actual symbols: {symbols:#?}\nexpected source symbol: {source_symbol:#?}",
        );
        assert!(
            symbols.contains(&dependency_symbol),
            "actual symbols: {symbols:#?}\nexpected dependency symbol: {dependency_symbol:#?}",
        );
    }

    fn assert_source_and_dependency_symbols(
        symbols: Vec<SymbolInformation>,
        name: &str,
        kind: SymbolKind,
        source_path: &Path,
        source: &str,
        source_occurrence: usize,
        interface_path: &Path,
        start_line: u32,
        start_character: u32,
        end_line: u32,
        end_character: u32,
        package_name: &str,
    ) {
        let source_symbol = SymbolInformation {
            name: name.to_owned(),
            kind,
            tags: None,
            deprecated: None,
            location: Location::new(
                Url::from_file_path(source_path).expect("source path should convert to URI"),
                tower_lsp::lsp_types::Range::new(
                    offset_to_position(source, nth_offset(source, name, source_occurrence)),
                    offset_to_position(
                        source,
                        nth_offset(source, name, source_occurrence) + name.len(),
                    ),
                ),
            ),
            container_name: None,
        };
        let dependency_symbol = SymbolInformation {
            name: name.to_owned(),
            kind,
            tags: None,
            deprecated: None,
            location: Location::new(
                Url::from_file_path(
                    fs::canonicalize(interface_path)
                        .expect("dependency interface path should canonicalize"),
                )
                .expect("dependency interface path should convert to URI"),
                tower_lsp::lsp_types::Range::new(
                    tower_lsp::lsp_types::Position::new(start_line, start_character),
                    tower_lsp::lsp_types::Position::new(end_line, end_character),
                ),
            ),
            container_name: Some(package_name.to_owned()),
        };

        assert_eq!(symbols.len(), 2, "actual symbols: {symbols:#?}");
        assert!(
            symbols.contains(&source_symbol),
            "actual symbols: {symbols:#?}\nexpected source symbol: {source_symbol:#?}",
        );
        assert!(
            symbols.contains(&dependency_symbol),
            "actual symbols: {symbols:#?}\nexpected dependency symbol: {dependency_symbol:#?}",
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_includes_package_modules_for_open_documents() {
        let temp = TempDir::new("ql-lsp-workspace-symbol-package");
        let root = temp.path().join("app");
        let main_path = temp.write(
            "app/qlang.toml",
            r#"
[package]
name = "app"
"#,
        );
        let _ = main_path;
        let open_path = temp.write(
            "app/src/main.ql",
            r#"
fn main() -> Int {
    return 0
}
"#,
        );
        let helper_path = temp.write(
            "app/src/helper.ql",
            r#"
fn helper_value() -> Int {
    return 1
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let symbols = workspace_symbols_for_documents(vec![(open_uri, open_source)], "helper");

        assert_eq!(
            symbols,
            vec![SymbolInformation {
                name: "helper_value".to_owned(),
                kind: SymbolKind::FUNCTION,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(&helper_path).expect("helper path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        tower_lsp::lsp_types::Position::new(1, 3),
                        tower_lsp::lsp_types::Position::new(1, 15),
                    ),
                ),
                container_name: None,
            }]
        );

        let _ = root;
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_includes_workspace_member_modules_for_open_documents() {
        let temp = TempDir::new("ql-lsp-workspace-symbol-members");

        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["app", "tool"]
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
fn main() -> Int {
    return 0
}
"#,
        );
        temp.write(
            "workspace/tool/qlang.toml",
            r#"
[package]
name = "tool"
"#,
        );
        let helper_path = temp.write(
            "workspace/tool/src/helper.ql",
            r#"
fn tool_helper() -> Int {
    return 1
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let symbols = workspace_symbols_for_documents(vec![(open_uri, open_source)], "helper");

        assert_eq!(
            symbols,
            vec![SymbolInformation {
                name: "tool_helper".to_owned(),
                kind: SymbolKind::FUNCTION,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(&helper_path).expect("helper path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        tower_lsp::lsp_types::Position::new(1, 3),
                        tower_lsp::lsp_types::Position::new(1, 14),
                    ),
                ),
                container_name: None,
            }]
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_keeps_workspace_member_modules_for_open_packages_when_member_has_source_diagnostics()
     {
        let temp = TempDir::new("ql-lsp-workspace-symbol-open-broken-member");

        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["app", "tool"]
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
fn main() -> Int {
    return 0
}
"#,
        );
        temp.write(
            "workspace/tool/qlang.toml",
            r#"
[package]
name = "tool"
"#,
        );
        let helper_path = temp.write(
            "workspace/tool/src/helper.ql",
            r#"
fn tool_helper() -> Int {
    return 1
}
"#,
        );
        temp.write(
            "workspace/tool/src/broken.ql",
            r#"
fn broken() -> Int {
    let value: Int = "oops"
    return value
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let symbols = workspace_symbols_for_documents(vec![(open_uri, open_source)], "helper");

        assert_eq!(
            symbols,
            vec![SymbolInformation {
                name: "tool_helper".to_owned(),
                kind: SymbolKind::FUNCTION,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(&helper_path).expect("helper path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        tower_lsp::lsp_types::Position::new(1, 3),
                        tower_lsp::lsp_types::Position::new(1, 14),
                    ),
                ),
                container_name: None,
            }]
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_keeps_member_dependency_methods_for_open_packages_when_member_has_source_diagnostics()
     {
        let temp = TempDir::new("ql-lsp-workspace-symbol-open-broken-member-method");

        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["app", "tool", "dep"]
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
fn main() -> Int {
    return 0
}
"#,
        );
        temp.write(
            "workspace/tool/qlang.toml",
            r#"
[package]
name = "tool"

[references]
packages = ["../dep"]
"#,
        );
        temp.write(
            "workspace/tool/src/helper.ql",
            r#"
use demo.dep.Config as Cfg

fn tool_helper(config: Cfg) -> Int {
    return config.get()
}
"#,
        );
        temp.write(
            "workspace/tool/src/broken.ql",
            r#"
fn broken() -> Int {
    let value: Int = "oops"
    return value
}
"#,
        );
        temp.write(
            "workspace/dep/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        let dependency_interface_path = temp.write(
            "workspace/dep/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let symbols = workspace_symbols_for_documents(vec![(open_uri, open_source)], "get");

        assert_eq!(
            symbols,
            vec![SymbolInformation {
                name: "get".to_owned(),
                kind: SymbolKind::METHOD,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(
                        fs::canonicalize(&dependency_interface_path)
                            .expect("dependency interface path should canonicalize"),
                    )
                    .expect("dependency interface path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        tower_lsp::lsp_types::Position::new(12, 8),
                        tower_lsp::lsp_types::Position::new(12, 27),
                    ),
                ),
                container_name: Some("dep".to_owned()),
            }]
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_keeps_dependency_symbols_for_broken_open_packages() {
        let temp = TempDir::new("ql-lsp-workspace-symbol-broken-dependency");

        temp.write(
            "workspace/dep/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        let dependency_interface_path = temp.write(
            "workspace/dep/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub fn exported(value: Int) -> Int
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
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
package demo.app

use demo.dep.exported as run

fn main() -> Int {
    let broken: Int = "oops"
    return run(1)
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let symbols = workspace_symbols_for_documents(vec![(open_uri, open_source)], "exported");

        assert_eq!(
            symbols,
            vec![SymbolInformation {
                name: "exported".to_owned(),
                kind: SymbolKind::FUNCTION,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(
                        fs::canonicalize(&dependency_interface_path)
                            .expect("dependency interface path should canonicalize"),
                    )
                    .expect("dependency interface path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        tower_lsp::lsp_types::Position::new(7, 4),
                        tower_lsp::lsp_types::Position::new(7, 34),
                    ),
                ),
                container_name: Some("dep".to_owned()),
            }]
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_prefers_local_dependency_source_symbols_for_broken_open_packages() {
        let temp = TempDir::new("ql-lsp-workspace-symbol-broken-local-dependency-source");

        temp.write(
            "workspace/dep/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        let dependency_source_path = temp.write(
            "workspace/dep/src/lib.ql",
            r#"
package demo.dep

pub fn exported(value: Int) -> Int {
    return value
}
"#,
        );
        temp.write(
            "workspace/dep/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub fn exported(value: Int) -> Int
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
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
package demo.app

use demo.dep.exported as run

fn main() -> Int {
    let broken: Int = "oops"
    return run(1)
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");
        let dependency_source =
            fs::read_to_string(&dependency_source_path).expect("dependency source should read");

        let symbols = workspace_symbols_for_documents(vec![(open_uri, open_source)], "exported");

        assert_eq!(
            symbols,
            vec![SymbolInformation {
                name: "exported".to_owned(),
                kind: SymbolKind::FUNCTION,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(&dependency_source_path)
                        .expect("dependency source path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        offset_to_position(
                            &dependency_source,
                            nth_offset(&dependency_source, "exported", 1),
                        ),
                        offset_to_position(
                            &dependency_source,
                            nth_offset(&dependency_source, "exported", 1) + "exported".len(),
                        ),
                    ),
                ),
                container_name: None,
            }]
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_prefers_local_dependency_source_methods_for_broken_open_packages() {
        let temp = TempDir::new("ql-lsp-workspace-symbol-broken-local-dependency-source-method");

        temp.write(
            "workspace/dep/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        let dependency_source_path = temp.write(
            "workspace/dep/src/lib.ql",
            r#"
package demo.dep

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int {
        return self.value
    }
}
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

impl Config {
    pub fn get(self) -> Int
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
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
package demo.app

use demo.dep.Config as Cfg

fn main(config: Cfg) -> Int {
    let broken: Int = "oops"
    return config.get()
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");
        let dependency_source =
            fs::read_to_string(&dependency_source_path).expect("dependency source should read");

        let symbols = workspace_symbols_for_documents(vec![(open_uri, open_source)], "get");

        assert_single_source_symbol(
            symbols,
            "get",
            SymbolKind::METHOD,
            &dependency_source_path,
            &dependency_source,
            1,
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_prefers_local_dependency_source_trait_and_extend_methods_for_broken_open_packages()
     {
        let temp = TempDir::new(
            "ql-lsp-workspace-symbol-broken-local-dependency-source-trait-extend-methods",
        );

        temp.write(
            "workspace/dep/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        let dependency_source_path = temp.write(
            "workspace/dep/src/lib.ql",
            r#"
package demo.dep

pub trait Reader {
    fn poll(self) -> Int
}

pub struct Buffer {
    value: Int,
}

extend Buffer {
    pub fn twice(self) -> Int {
        return 2
    }
}
"#,
        );
        temp.write(
            "workspace/dep/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub trait Reader {
    fn poll(self) -> Int
}

pub struct Buffer {
    value: Int,
}

extend Buffer {
    pub fn twice(self) -> Int
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
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
package demo.app

fn main() -> Int {
    let broken: Int = "oops"
    return 0
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");
        let dependency_source =
            fs::read_to_string(&dependency_source_path).expect("dependency source should read");

        let trait_symbols =
            workspace_symbols_for_documents(vec![(open_uri.clone(), open_source.clone())], "poll");
        let extend_symbols =
            workspace_symbols_for_documents(vec![(open_uri, open_source)], "twice");

        assert_single_source_symbol(
            trait_symbols,
            "poll",
            SymbolKind::METHOD,
            &dependency_source_path,
            &dependency_source,
            1,
        );
        assert_single_source_symbol(
            extend_symbols,
            "twice",
            SymbolKind::METHOD,
            &dependency_source_path,
            &dependency_source,
            1,
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_keeps_dependency_methods_for_broken_open_packages() {
        let temp = TempDir::new("ql-lsp-workspace-symbol-broken-dependency-method");

        temp.write(
            "workspace/dep/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        let dependency_interface_path = temp.write(
            "workspace/dep/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int
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
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
package demo.app

use demo.dep.Config as Cfg

fn main(config: Cfg) -> Int {
    let broken: Int = "oops"
    return config.get()
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let symbols = workspace_symbols_for_documents(vec![(open_uri, open_source)], "get");

        assert_eq!(
            symbols,
            vec![SymbolInformation {
                name: "get".to_owned(),
                kind: SymbolKind::METHOD,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(
                        fs::canonicalize(&dependency_interface_path)
                            .expect("dependency interface path should canonicalize"),
                    )
                    .expect("dependency interface path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        tower_lsp::lsp_types::Position::new(12, 8),
                        tower_lsp::lsp_types::Position::new(12, 27),
                    ),
                ),
                container_name: Some("dep".to_owned()),
            }]
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_keeps_package_and_workspace_member_modules_when_dependency_interfaces_fail()
     {
        let temp = TempDir::new("ql-lsp-workspace-symbol-open-missing-dependency");

        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["app", "tool", "dep"]
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
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
package demo.app

use demo.dep.exported as run

fn main() -> Int {
    return run(0)
}
"#,
        );
        let app_helper_path = temp.write(
            "workspace/app/src/helper.ql",
            r#"
fn app_helper() -> Int {
    return 0
}
"#,
        );
        temp.write(
            "workspace/tool/qlang.toml",
            r#"
[package]
name = "tool"
"#,
        );
        let tool_helper_path = temp.write(
            "workspace/tool/src/helper.ql",
            r#"
fn tool_helper() -> Int {
    return 1
}
"#,
        );
        temp.write(
            "workspace/dep/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        temp.write(
            "workspace/dep/src/lib.ql",
            r#"
package demo.dep

pub fn exported(value: Int) -> Int {
    return value
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let symbols = workspace_symbols_for_documents(vec![(open_uri, open_source)], "helper");

        assert_eq!(
            symbols,
            vec![
                SymbolInformation {
                    name: "app_helper".to_owned(),
                    kind: SymbolKind::FUNCTION,
                    tags: None,
                    deprecated: None,
                    location: Location::new(
                        Url::from_file_path(&app_helper_path)
                            .expect("helper path should convert to URI"),
                        tower_lsp::lsp_types::Range::new(
                            tower_lsp::lsp_types::Position::new(1, 3),
                            tower_lsp::lsp_types::Position::new(1, 13),
                        ),
                    ),
                    container_name: None,
                },
                SymbolInformation {
                    name: "tool_helper".to_owned(),
                    kind: SymbolKind::FUNCTION,
                    tags: None,
                    deprecated: None,
                    location: Location::new(
                        Url::from_file_path(&tool_helper_path)
                            .expect("helper path should convert to URI"),
                        tower_lsp::lsp_types::Range::new(
                            tower_lsp::lsp_types::Position::new(1, 3),
                            tower_lsp::lsp_types::Position::new(1, 14),
                        ),
                    ),
                    container_name: None,
                },
            ]
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_keeps_workspace_member_modules_when_member_dependency_interfaces_fail()
     {
        let temp = TempDir::new("ql-lsp-workspace-symbol-member-missing-dependency");

        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["app", "tool", "dep"]
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
fn main() -> Int {
    return 0
}
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"
"#,
        );
        temp.write(
            "workspace/tool/qlang.toml",
            r#"
[package]
name = "tool"

[references]
packages = ["../dep"]
"#,
        );
        let helper_path = temp.write(
            "workspace/tool/src/helper.ql",
            r#"
fn tool_helper() -> Int {
    return 1
}
"#,
        );
        temp.write(
            "workspace/dep/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        temp.write(
            "workspace/dep/src/lib.ql",
            r#"
package demo.dep

pub fn exported(value: Int) -> Int {
    return value
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let symbols = workspace_symbols_for_documents(vec![(open_uri, open_source)], "helper");

        assert_eq!(
            symbols,
            vec![SymbolInformation {
                name: "tool_helper".to_owned(),
                kind: SymbolKind::FUNCTION,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(&helper_path).expect("helper path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        tower_lsp::lsp_types::Position::new(1, 3),
                        tower_lsp::lsp_types::Position::new(1, 14),
                    ),
                ),
                container_name: None,
            }]
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_keeps_workspace_member_modules_for_broken_open_packages() {
        let temp = TempDir::new("ql-lsp-workspace-symbol-broken-members");

        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["app", "tool"]
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
fn main() -> Int {
    let broken: Int = "oops"
    return 0
}
"#,
        );
        temp.write(
            "workspace/tool/qlang.toml",
            r#"
[package]
name = "tool"
"#,
        );
        let helper_path = temp.write(
            "workspace/tool/src/helper.ql",
            r#"
fn tool_helper() -> Int {
    return 1
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let symbols = workspace_symbols_for_documents(vec![(open_uri, open_source)], "helper");

        assert_eq!(
            symbols,
            vec![SymbolInformation {
                name: "tool_helper".to_owned(),
                kind: SymbolKind::FUNCTION,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(&helper_path).expect("helper path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        tower_lsp::lsp_types::Position::new(1, 3),
                        tower_lsp::lsp_types::Position::new(1, 14),
                    ),
                ),
                container_name: None,
            }]
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_keeps_workspace_member_modules_for_broken_open_packages_when_dependency_interfaces_fail()
     {
        let temp = TempDir::new("ql-lsp-workspace-symbol-broken-members-missing-dependency");

        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["app", "tool", "dep"]
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
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
package demo.app

use demo.dep.exported as run

fn main() -> Int {
    let broken: Int = "oops"
    return run(0)
}
"#,
        );
        temp.write(
            "workspace/tool/qlang.toml",
            r#"
[package]
name = "tool"
"#,
        );
        let helper_path = temp.write(
            "workspace/tool/src/helper.ql",
            r#"
fn tool_helper() -> Int {
    return 1
}
"#,
        );
        temp.write(
            "workspace/dep/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        temp.write(
            "workspace/dep/src/lib.ql",
            r#"
package demo.dep

pub fn exported(value: Int) -> Int {
    return value
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let symbols = workspace_symbols_for_documents(vec![(open_uri, open_source)], "helper");

        assert_eq!(
            symbols,
            vec![SymbolInformation {
                name: "tool_helper".to_owned(),
                kind: SymbolKind::FUNCTION,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(&helper_path).expect("helper path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        tower_lsp::lsp_types::Position::new(1, 3),
                        tower_lsp::lsp_types::Position::new(1, 14),
                    ),
                ),
                container_name: None,
            }]
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_includes_dependency_interface_symbols_for_open_packages() {
        let temp = TempDir::new("ql-lsp-workspace-symbol-dependency");

        temp.write(
            "workspace/dep/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        let dependency_interface_path = temp.write(
            "workspace/dep/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub fn exported(value: Int) -> Int
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
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
package demo.app

use demo.dep.exported as run

fn main() -> Int {
    return run(1)
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let symbols = workspace_symbols_for_documents(vec![(open_uri, open_source)], "exported");

        assert_eq!(
            symbols,
            vec![SymbolInformation {
                name: "exported".to_owned(),
                kind: SymbolKind::FUNCTION,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(
                        fs::canonicalize(&dependency_interface_path)
                            .expect("dependency interface path should canonicalize"),
                    )
                    .expect("dependency interface path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        tower_lsp::lsp_types::Position::new(7, 4),
                        tower_lsp::lsp_types::Position::new(7, 34),
                    ),
                ),
                container_name: Some("dep".to_owned()),
            }]
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_prefers_local_dependency_source_symbols_for_open_packages() {
        let temp = TempDir::new("ql-lsp-workspace-symbol-local-dependency-source");

        temp.write(
            "workspace/dep/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        let dependency_source_path = temp.write(
            "workspace/dep/src/lib.ql",
            r#"
package demo.dep

pub fn exported(value: Int) -> Int {
    return value
}
"#,
        );
        temp.write(
            "workspace/dep/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub fn exported(value: Int) -> Int
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
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
package demo.app

use demo.dep.exported as run

fn main() -> Int {
    return run(1)
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");
        let dependency_source =
            fs::read_to_string(&dependency_source_path).expect("dependency source should read");

        let symbols = workspace_symbols_for_documents(vec![(open_uri, open_source)], "exported");

        assert_eq!(
            symbols,
            vec![SymbolInformation {
                name: "exported".to_owned(),
                kind: SymbolKind::FUNCTION,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(&dependency_source_path)
                        .expect("dependency source path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        offset_to_position(
                            &dependency_source,
                            nth_offset(&dependency_source, "exported", 1),
                        ),
                        offset_to_position(
                            &dependency_source,
                            nth_offset(&dependency_source, "exported", 1) + "exported".len(),
                        ),
                    ),
                ),
                container_name: None,
            }]
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_keeps_same_named_dependency_interface_symbols_for_open_packages() {
        let temp =
            TempDir::new("ql-lsp-workspace-symbol-same-name-local-dependency-interface-open");

        temp.write(
            "workspace/dep-source/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        temp.write(
            "workspace/dep-source/src/lib.ql",
            r#"
package demo.dep.source

pub fn alpha() -> Int {
    return 1
}
"#,
        );
        temp.write(
            "workspace/dep-source/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep.source

pub fn alpha() -> Int
"#,
        );
        temp.write(
            "workspace/dep-interface/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        let dependency_interface_path = temp.write(
            "workspace/dep-interface/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep.interface

pub fn beta() -> Int
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../dep-source", "../dep-interface"]
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
fn main() -> Int {
    return 0
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let symbols = workspace_symbols_for_documents(vec![(open_uri, open_source)], "beta");

        assert_single_dependency_symbol(
            symbols,
            "beta",
            SymbolKind::FUNCTION,
            &dependency_interface_path,
            7,
            4,
            20,
            "dep",
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_keeps_same_named_dependency_method_symbols_for_open_packages() {
        let temp = TempDir::new("ql-lsp-workspace-symbol-same-name-local-dependency-methods-open");
        let fixture = setup_same_named_dependency_method_symbols_fixture(&temp);
        let open_source = fs::read_to_string(&fixture.open_path).expect("open file should read");
        let open_uri =
            Url::from_file_path(&fixture.open_path).expect("open path should convert to URI");

        let get_symbols =
            workspace_symbols_for_documents(vec![(open_uri.clone(), open_source.clone())], "get");
        let trait_symbols =
            workspace_symbols_for_documents(vec![(open_uri.clone(), open_source.clone())], "poll");
        let extend_symbols =
            workspace_symbols_for_documents(vec![(open_uri, open_source)], "twice");

        assert_source_and_dependency_method_symbols(
            get_symbols,
            "get",
            &fixture.dependency_source_path,
            &fixture.dependency_source,
            1,
            &fixture.dependency_interface_path,
            12,
            8,
            27,
            "dep",
        );
        assert_source_and_dependency_method_symbols(
            trait_symbols,
            "poll",
            &fixture.dependency_source_path,
            &fixture.dependency_source,
            1,
            &fixture.dependency_interface_path,
            16,
            4,
            24,
            "dep",
        );
        assert_source_and_dependency_method_symbols(
            extend_symbols,
            "twice",
            &fixture.dependency_source_path,
            &fixture.dependency_source,
            1,
            &fixture.dependency_interface_path,
            24,
            8,
            29,
            "dep",
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_keeps_same_named_dependency_type_symbols_for_open_packages_with_local_dependencies()
     {
        let temp = TempDir::new("ql-lsp-workspace-symbol-same-name-local-dependency-types");
        let fixture = setup_same_named_dependency_method_symbols_local_fixture(&temp);
        let open_source = fs::read_to_string(&fixture.open_path).expect("open file should read");
        let open_uri =
            Url::from_file_path(&fixture.open_path).expect("open path should convert to URI");

        let config_symbols = workspace_symbols_for_documents(
            vec![(open_uri.clone(), open_source.clone())],
            "Config",
        );
        let reader_symbols = workspace_symbols_for_documents(
            vec![(open_uri.clone(), open_source.clone())],
            "Reader",
        );
        let buffer_symbols =
            workspace_symbols_for_documents(vec![(open_uri, open_source)], "Buffer");

        assert_source_and_dependency_symbols(
            config_symbols,
            "Config",
            SymbolKind::STRUCT,
            &fixture.dependency_source_path,
            &fixture.dependency_source,
            1,
            &fixture.dependency_interface_path,
            7,
            0,
            9,
            1,
            "dep",
        );
        assert_source_and_dependency_symbols(
            reader_symbols,
            "Reader",
            SymbolKind::INTERFACE,
            &fixture.dependency_source_path,
            &fixture.dependency_source,
            1,
            &fixture.dependency_interface_path,
            11,
            0,
            13,
            1,
            "dep",
        );
        assert_source_and_dependency_symbols(
            buffer_symbols,
            "Buffer",
            SymbolKind::STRUCT,
            &fixture.dependency_source_path,
            &fixture.dependency_source,
            1,
            &fixture.dependency_interface_path,
            15,
            0,
            17,
            1,
            "dep",
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_keeps_same_named_dependency_enum_symbols_for_open_packages_with_local_dependencies()
     {
        let temp = TempDir::new("ql-lsp-workspace-symbol-same-name-local-dependency-enums");
        let fixture = setup_same_named_dependency_enum_symbols_fixture(&temp);
        let open_source = fs::read_to_string(&fixture.open_path).expect("open file should read");
        let open_uri =
            Url::from_file_path(&fixture.open_path).expect("open path should convert to URI");

        let enum_symbols = workspace_symbols_for_documents(
            vec![(open_uri.clone(), open_source.clone())],
            "Command",
        );
        let variant_symbols =
            workspace_symbols_for_documents(vec![(open_uri, open_source)], "Retry");

        assert_source_and_dependency_symbols(
            enum_symbols,
            "Command",
            SymbolKind::ENUM,
            &fixture.dependency_source_path,
            &fixture.dependency_source,
            1,
            &fixture.dependency_interface_path,
            7,
            0,
            9,
            1,
            "core",
        );
        assert_source_and_dependency_symbols(
            variant_symbols,
            "Retry",
            SymbolKind::ENUM_MEMBER,
            &fixture.dependency_source_path,
            &fixture.dependency_source,
            1,
            &fixture.dependency_interface_path,
            8,
            4,
            8,
            9,
            "core",
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_prefers_local_dependency_source_methods_for_open_packages() {
        let temp = TempDir::new("ql-lsp-workspace-symbol-local-dependency-source-method");

        temp.write(
            "workspace/dep/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        let dependency_source_path = temp.write(
            "workspace/dep/src/lib.ql",
            r#"
package demo.dep

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int {
        return self.value
    }
}
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

impl Config {
    pub fn get(self) -> Int
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
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
package demo.app

use demo.dep.Config as Cfg

fn main(config: Cfg) -> Int {
    return config.get()
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");
        let dependency_source =
            fs::read_to_string(&dependency_source_path).expect("dependency source should read");

        let symbols = workspace_symbols_for_documents(vec![(open_uri, open_source)], "get");

        assert_single_source_symbol(
            symbols,
            "get",
            SymbolKind::METHOD,
            &dependency_source_path,
            &dependency_source,
            1,
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_prefers_local_dependency_source_trait_and_extend_methods_for_open_packages()
     {
        let temp =
            TempDir::new("ql-lsp-workspace-symbol-local-dependency-source-trait-extend-methods");

        temp.write(
            "workspace/dep/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        let dependency_source_path = temp.write(
            "workspace/dep/src/lib.ql",
            r#"
package demo.dep

pub trait Reader {
    fn poll(self) -> Int
}

pub struct Buffer {
    value: Int,
}

extend Buffer {
    pub fn twice(self) -> Int {
        return 2
    }
}
"#,
        );
        temp.write(
            "workspace/dep/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub trait Reader {
    fn poll(self) -> Int
}

pub struct Buffer {
    value: Int,
}

extend Buffer {
    pub fn twice(self) -> Int
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
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
fn main() -> Int {
    return 0
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");
        let dependency_source =
            fs::read_to_string(&dependency_source_path).expect("dependency source should read");

        let trait_symbols =
            workspace_symbols_for_documents(vec![(open_uri.clone(), open_source.clone())], "poll");
        let extend_symbols =
            workspace_symbols_for_documents(vec![(open_uri, open_source)], "twice");

        assert_single_source_symbol(
            trait_symbols,
            "poll",
            SymbolKind::METHOD,
            &dependency_source_path,
            &dependency_source,
            1,
        );
        assert_single_source_symbol(
            extend_symbols,
            "twice",
            SymbolKind::METHOD,
            &dependency_source_path,
            &dependency_source,
            1,
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_includes_dependency_interface_methods_for_open_packages() {
        let temp = TempDir::new("ql-lsp-workspace-symbol-dependency-method");

        temp.write(
            "workspace/dep/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        let dependency_interface_path = temp.write(
            "workspace/dep/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int
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
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
package demo.app

use demo.dep.Config as Cfg

fn main(config: Cfg) -> Int {
    return config.get()
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let symbols = workspace_symbols_for_documents(vec![(open_uri, open_source)], "get");

        assert_eq!(
            symbols,
            vec![SymbolInformation {
                name: "get".to_owned(),
                kind: SymbolKind::METHOD,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(
                        fs::canonicalize(&dependency_interface_path)
                            .expect("dependency interface path should canonicalize"),
                    )
                    .expect("dependency interface path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        tower_lsp::lsp_types::Position::new(12, 8),
                        tower_lsp::lsp_types::Position::new(12, 27),
                    ),
                ),
                container_name: Some("dep".to_owned()),
            }]
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_includes_dependency_interface_trait_and_extend_methods_for_open_packages()
     {
        let temp = TempDir::new("ql-lsp-workspace-symbol-dependency-trait-extend-methods");

        temp.write(
            "workspace/dep/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        let dependency_interface_path = temp.write(
            "workspace/dep/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub trait Reader {
    fn poll(self) -> Int
}

pub struct Buffer {
    value: Int,
}

extend Buffer {
    pub fn twice(self) -> Int
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
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
fn main() -> Int {
    return 0
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let trait_symbols =
            workspace_symbols_for_documents(vec![(open_uri.clone(), open_source.clone())], "poll");
        let extend_symbols =
            workspace_symbols_for_documents(vec![(open_uri, open_source)], "twice");

        assert_eq!(
            trait_symbols,
            vec![SymbolInformation {
                name: "poll".to_owned(),
                kind: SymbolKind::METHOD,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(
                        fs::canonicalize(&dependency_interface_path)
                            .expect("dependency interface path should canonicalize"),
                    )
                    .expect("dependency interface path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        tower_lsp::lsp_types::Position::new(8, 4),
                        tower_lsp::lsp_types::Position::new(8, 24),
                    ),
                ),
                container_name: Some("dep".to_owned()),
            }]
        );
        assert_eq!(
            extend_symbols,
            vec![SymbolInformation {
                name: "twice".to_owned(),
                kind: SymbolKind::METHOD,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(
                        fs::canonicalize(&dependency_interface_path)
                            .expect("dependency interface path should canonicalize"),
                    )
                    .expect("dependency interface path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        tower_lsp::lsp_types::Position::new(16, 8),
                        tower_lsp::lsp_types::Position::new(16, 29),
                    ),
                ),
                container_name: Some("dep".to_owned()),
            }]
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_includes_dependency_interface_symbols_for_workspace_members() {
        let temp = TempDir::new("ql-lsp-workspace-symbol-member-dependency");

        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["app", "tool", "dep"]
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
fn main() -> Int {
    return 0
}
"#,
        );
        temp.write(
            "workspace/tool/qlang.toml",
            r#"
[package]
name = "tool"

[references]
packages = ["../dep"]
"#,
        );
        temp.write(
            "workspace/tool/src/helper.ql",
            r#"
use demo.dep.exported as run

fn tool_helper() -> Int {
    return run(1)
}
"#,
        );
        temp.write(
            "workspace/dep/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        let dependency_interface_path = temp.write(
            "workspace/dep/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub fn exported(value: Int) -> Int
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let symbols = workspace_symbols_for_documents(vec![(open_uri, open_source)], "exported");

        assert_eq!(
            symbols,
            vec![SymbolInformation {
                name: "exported".to_owned(),
                kind: SymbolKind::FUNCTION,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(
                        fs::canonicalize(&dependency_interface_path)
                            .expect("dependency interface path should canonicalize"),
                    )
                    .expect("dependency interface path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        tower_lsp::lsp_types::Position::new(7, 4),
                        tower_lsp::lsp_types::Position::new(7, 34),
                    ),
                ),
                container_name: Some("dep".to_owned()),
            }]
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_includes_dependency_interface_methods_for_workspace_members() {
        let temp = TempDir::new("ql-lsp-workspace-symbol-member-dependency-method");

        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["app", "tool", "dep"]
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
fn main() -> Int {
    return 0
}
"#,
        );
        temp.write(
            "workspace/tool/qlang.toml",
            r#"
[package]
name = "tool"

[references]
packages = ["../dep"]
"#,
        );
        temp.write(
            "workspace/tool/src/helper.ql",
            r#"
use demo.dep.Config as Cfg

fn tool_helper(config: Cfg) -> Int {
    return config.get()
}
"#,
        );
        temp.write(
            "workspace/dep/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        let dependency_interface_path = temp.write(
            "workspace/dep/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let symbols = workspace_symbols_for_documents(vec![(open_uri, open_source)], "get");

        assert_eq!(
            symbols,
            vec![SymbolInformation {
                name: "get".to_owned(),
                kind: SymbolKind::METHOD,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(
                        fs::canonicalize(&dependency_interface_path)
                            .expect("dependency interface path should canonicalize"),
                    )
                    .expect("dependency interface path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        tower_lsp::lsp_types::Position::new(12, 8),
                        tower_lsp::lsp_types::Position::new(12, 27),
                    ),
                ),
                container_name: Some("dep".to_owned()),
            }]
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_keeps_available_dependency_symbols_when_one_package_interface_is_missing()
     {
        let temp = TempDir::new("ql-lsp-workspace-symbol-partial-dependency");

        temp.write(
            "workspace/good/qlang.toml",
            r#"
[package]
name = "good"
"#,
        );
        let dependency_interface_path = temp.write(
            "workspace/good/good.qi",
            r#"
// qlang interface v1
// package: good

// source: src/lib.ql
package demo.good

pub fn exported(value: Int) -> Int
"#,
        );
        temp.write(
            "workspace/bad/qlang.toml",
            r#"
[package]
name = "bad"
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../good", "../bad"]
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
fn main() -> Int {
    return 0
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let symbols = workspace_symbols_for_documents(vec![(open_uri, open_source)], "exported");

        assert_eq!(
            symbols,
            vec![SymbolInformation {
                name: "exported".to_owned(),
                kind: SymbolKind::FUNCTION,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(
                        fs::canonicalize(&dependency_interface_path)
                            .expect("dependency interface path should canonicalize"),
                    )
                    .expect("dependency interface path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        tower_lsp::lsp_types::Position::new(7, 4),
                        tower_lsp::lsp_types::Position::new(7, 34),
                    ),
                ),
                container_name: Some("good".to_owned()),
            }]
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_includes_dependency_interface_trait_and_extend_methods_for_workspace_members()
     {
        let temp = TempDir::new("ql-lsp-workspace-symbol-member-dependency-trait-extend-methods");

        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["app", "tool", "dep"]
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
fn main() -> Int {
    return 0
}
"#,
        );
        temp.write(
            "workspace/tool/qlang.toml",
            r#"
[package]
name = "tool"

[references]
packages = ["../dep"]
"#,
        );
        temp.write(
            "workspace/tool/src/helper.ql",
            r#"
fn tool_helper() -> Int {
    return 1
}
"#,
        );
        temp.write(
            "workspace/dep/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        let dependency_interface_path = temp.write(
            "workspace/dep/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub trait Reader {
    fn poll(self) -> Int
}

pub struct Buffer {
    value: Int,
}

extend Buffer {
    pub fn twice(self) -> Int
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let trait_symbols =
            workspace_symbols_for_documents(vec![(open_uri.clone(), open_source.clone())], "poll");
        let extend_symbols =
            workspace_symbols_for_documents(vec![(open_uri, open_source)], "twice");

        assert_eq!(
            trait_symbols,
            vec![SymbolInformation {
                name: "poll".to_owned(),
                kind: SymbolKind::METHOD,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(
                        fs::canonicalize(&dependency_interface_path)
                            .expect("dependency interface path should canonicalize"),
                    )
                    .expect("dependency interface path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        tower_lsp::lsp_types::Position::new(8, 4),
                        tower_lsp::lsp_types::Position::new(8, 24),
                    ),
                ),
                container_name: Some("dep".to_owned()),
            }]
        );
        assert_eq!(
            extend_symbols,
            vec![SymbolInformation {
                name: "twice".to_owned(),
                kind: SymbolKind::METHOD,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(
                        fs::canonicalize(&dependency_interface_path)
                            .expect("dependency interface path should canonicalize"),
                    )
                    .expect("dependency interface path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        tower_lsp::lsp_types::Position::new(16, 8),
                        tower_lsp::lsp_types::Position::new(16, 29),
                    ),
                ),
                container_name: Some("dep".to_owned()),
            }]
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_keeps_available_dependency_methods_when_one_package_interface_is_missing()
     {
        let temp = TempDir::new("ql-lsp-workspace-symbol-partial-dependency-method");

        temp.write(
            "workspace/good/qlang.toml",
            r#"
[package]
name = "good"
"#,
        );
        let dependency_interface_path = temp.write(
            "workspace/good/good.qi",
            r#"
// qlang interface v1
// package: good

// source: src/lib.ql
package demo.good

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int
}
"#,
        );
        temp.write(
            "workspace/bad/qlang.toml",
            r#"
[package]
name = "bad"
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../good", "../bad"]
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
fn main() -> Int {
    return 0
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let symbols = workspace_symbols_for_documents(vec![(open_uri, open_source)], "get");

        assert_eq!(
            symbols,
            vec![SymbolInformation {
                name: "get".to_owned(),
                kind: SymbolKind::METHOD,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(
                        fs::canonicalize(&dependency_interface_path)
                            .expect("dependency interface path should canonicalize"),
                    )
                    .expect("dependency interface path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        tower_lsp::lsp_types::Position::new(12, 8),
                        tower_lsp::lsp_types::Position::new(12, 27),
                    ),
                ),
                container_name: Some("good".to_owned()),
            }]
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_keeps_available_dependency_methods_when_reference_manifest_is_invalid()
     {
        let temp = TempDir::new("ql-lsp-workspace-symbol-invalid-reference-manifest-method");

        temp.write(
            "workspace/good/qlang.toml",
            r#"
[package]
name = "good"
"#,
        );
        let dependency_interface_path = temp.write(
            "workspace/good/good.qi",
            r#"
// qlang interface v1
// package: good

// source: src/lib.ql
package demo.good

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int
}
"#,
        );
        temp.write(
            "workspace/bad/qlang.toml",
            r#"
[package
name = "bad"
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../good", "../bad"]
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
fn main() -> Int {
    return 0
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let symbols = workspace_symbols_for_documents(vec![(open_uri, open_source)], "get");

        assert_eq!(
            symbols,
            vec![SymbolInformation {
                name: "get".to_owned(),
                kind: SymbolKind::METHOD,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(
                        fs::canonicalize(&dependency_interface_path)
                            .expect("dependency interface path should canonicalize"),
                    )
                    .expect("dependency interface path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        tower_lsp::lsp_types::Position::new(12, 8),
                        tower_lsp::lsp_types::Position::new(12, 27),
                    ),
                ),
                container_name: Some("good".to_owned()),
            }]
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_keeps_available_member_dependency_symbols_when_one_member_interface_is_missing()
     {
        let temp = TempDir::new("ql-lsp-workspace-symbol-partial-member-dependency");

        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["app", "tool", "good", "bad"]
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
fn main() -> Int {
    return 0
}
"#,
        );
        temp.write(
            "workspace/tool/qlang.toml",
            r#"
[package]
name = "tool"

[references]
packages = ["../good", "../bad"]
"#,
        );
        temp.write(
            "workspace/tool/src/helper.ql",
            r#"
fn tool_helper() -> Int {
    return 1
}
"#,
        );
        temp.write(
            "workspace/good/qlang.toml",
            r#"
[package]
name = "good"
"#,
        );
        let dependency_interface_path = temp.write(
            "workspace/good/good.qi",
            r#"
// qlang interface v1
// package: good

// source: src/lib.ql
package demo.good

pub fn exported(value: Int) -> Int
"#,
        );
        temp.write(
            "workspace/bad/qlang.toml",
            r#"
[package]
name = "bad"
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let symbols = workspace_symbols_for_documents(vec![(open_uri, open_source)], "exported");

        assert_eq!(
            symbols,
            vec![SymbolInformation {
                name: "exported".to_owned(),
                kind: SymbolKind::FUNCTION,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(
                        fs::canonicalize(&dependency_interface_path)
                            .expect("dependency interface path should canonicalize"),
                    )
                    .expect("dependency interface path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        tower_lsp::lsp_types::Position::new(7, 4),
                        tower_lsp::lsp_types::Position::new(7, 34),
                    ),
                ),
                container_name: Some("good".to_owned()),
            }]
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_keeps_available_member_dependency_methods_when_one_member_interface_is_missing()
     {
        let temp = TempDir::new("ql-lsp-workspace-symbol-partial-member-dependency-method");

        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["app", "tool", "good", "bad"]
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
fn main() -> Int {
    return 0
}
"#,
        );
        temp.write(
            "workspace/tool/qlang.toml",
            r#"
[package]
name = "tool"

[references]
packages = ["../good", "../bad"]
"#,
        );
        temp.write(
            "workspace/tool/src/helper.ql",
            r#"
fn tool_helper() -> Int {
    return 1
}
"#,
        );
        temp.write(
            "workspace/good/qlang.toml",
            r#"
[package]
name = "good"
"#,
        );
        let dependency_interface_path = temp.write(
            "workspace/good/good.qi",
            r#"
// qlang interface v1
// package: good

// source: src/lib.ql
package demo.good

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int
}
"#,
        );
        temp.write(
            "workspace/bad/qlang.toml",
            r#"
[package]
name = "bad"
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let symbols = workspace_symbols_for_documents(vec![(open_uri, open_source)], "get");

        assert_eq!(
            symbols,
            vec![SymbolInformation {
                name: "get".to_owned(),
                kind: SymbolKind::METHOD,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(
                        fs::canonicalize(&dependency_interface_path)
                            .expect("dependency interface path should canonicalize"),
                    )
                    .expect("dependency interface path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        tower_lsp::lsp_types::Position::new(12, 8),
                        tower_lsp::lsp_types::Position::new(12, 27),
                    ),
                ),
                container_name: Some("good".to_owned()),
            }]
        );
    }

    #[test]
    fn package_analysis_path_keeps_available_dependency_completions_when_one_interface_is_missing()
    {
        let temp = TempDir::new("ql-lsp-package-fallback-partial-dependency");

        temp.write(
            "workspace/good/qlang.toml",
            r#"
[package]
name = "good"
"#,
        );
        temp.write(
            "workspace/good/good.qi",
            r#"
// qlang interface v1
// package: good

// source: src/lib.ql
package demo.good

pub struct Buffer[T] {
    value: T,
}
"#,
        );
        temp.write(
            "workspace/bad/qlang.toml",
            r#"
[package]
name = "bad"
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
package demo.app

use demo.good.Bu

fn main() -> Int {
    return 0
}
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../good", "../bad"]
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");

        let package =
            package_analysis_for_path(&open_path).expect("fallback package analysis should exist");
        let completions = package
            .dependency_completions_at(&open_source, nth_offset(&open_source, "Bu", 1) + 2)
            .expect("dependency completions should exist");

        assert!(completions.iter().any(|item| {
            item.label == "Buffer"
                && item.kind == AnalysisSymbolKind::Struct
                && item.detail.starts_with("struct Buffer[T] {")
        }));
    }

    #[test]
    fn package_analysis_path_keeps_available_dependency_definitions_for_source_diagnostics() {
        let temp = TempDir::new("ql-lsp-package-fallback-source-diagnostics");

        temp.write(
            "workspace/good/qlang.toml",
            r#"
[package]
name = "good"
"#,
        );
        temp.write(
            "workspace/good/good.qi",
            r#"
// qlang interface v1
// package: good

// source: src/lib.ql
package demo.good

pub fn exported(value: Int) -> Int
"#,
        );
        temp.write(
            "workspace/bad/qlang.toml",
            r#"
[package]
name = "bad"
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
package demo.app

use demo.good.exported as run

fn main() -> Int {
    let value: Missing = run(1)
    return value
}
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../good", "../bad"]
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");

        let package =
            package_analysis_for_path(&open_path).expect("fallback package analysis should exist");
        let definition = package
            .dependency_definition_in_source_at(&open_source, nth_offset(&open_source, "run", 2))
            .expect("dependency definition should exist");

        assert_eq!(definition.kind, AnalysisSymbolKind::Function);
        assert_eq!(definition.name, "exported");
        assert!(definition.path.ends_with("good.qi"));
    }

    #[test]
    fn package_analysis_path_keeps_available_dependency_completions_when_one_reference_manifest_is_invalid()
     {
        let temp = TempDir::new("ql-lsp-package-fallback-invalid-reference-manifest");

        temp.write(
            "workspace/good/qlang.toml",
            r#"
[package]
name = "good"
"#,
        );
        temp.write(
            "workspace/good/good.qi",
            r#"
// qlang interface v1
// package: good

// source: src/lib.ql
package demo.good

pub struct Buffer[T] {
    value: T,
}
"#,
        );
        temp.write(
            "workspace/bad/qlang.toml",
            r#"
[package
name = "bad"
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
package demo.app

use demo.good.Bu

fn main() -> Int {
    return 0
}
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../good", "../bad"]
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");

        let package =
            package_analysis_for_path(&open_path).expect("fallback package analysis should exist");
        let completions = package
            .dependency_completions_at(&open_source, nth_offset(&open_source, "Bu", 1) + 2)
            .expect("dependency completions should exist");

        assert!(completions.iter().any(|item| {
            item.label == "Buffer"
                && item.kind == AnalysisSymbolKind::Struct
                && item.detail.starts_with("struct Buffer[T] {")
        }));
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_keeps_available_member_dependency_symbols_when_member_reference_manifest_is_invalid()
     {
        let temp = TempDir::new("ql-lsp-workspace-symbol-invalid-member-reference-manifest");

        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["app", "tool", "good", "bad"]
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
fn main() -> Int {
    return 0
}
"#,
        );
        temp.write(
            "workspace/tool/qlang.toml",
            r#"
[package]
name = "tool"

[references]
packages = ["../good", "../bad"]
"#,
        );
        temp.write(
            "workspace/tool/src/helper.ql",
            r#"
fn tool_helper() -> Int {
    return 1
}
"#,
        );
        temp.write(
            "workspace/good/qlang.toml",
            r#"
[package]
name = "good"
"#,
        );
        let dependency_interface_path = temp.write(
            "workspace/good/good.qi",
            r#"
// qlang interface v1
// package: good

// source: src/lib.ql
package demo.good

pub fn exported(value: Int) -> Int
"#,
        );
        temp.write(
            "workspace/bad/qlang.toml",
            r#"
[package
name = "bad"
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let symbols = workspace_symbols_for_documents(vec![(open_uri, open_source)], "exported");

        assert_eq!(
            symbols,
            vec![SymbolInformation {
                name: "exported".to_owned(),
                kind: SymbolKind::FUNCTION,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(
                        fs::canonicalize(&dependency_interface_path)
                            .expect("dependency interface path should canonicalize"),
                    )
                    .expect("dependency interface path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        tower_lsp::lsp_types::Position::new(7, 4),
                        tower_lsp::lsp_types::Position::new(7, 34),
                    ),
                ),
                container_name: Some("good".to_owned()),
            }]
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_keeps_available_member_dependency_methods_when_member_reference_manifest_is_invalid()
     {
        let temp = TempDir::new("ql-lsp-workspace-symbol-invalid-member-reference-manifest-method");

        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["app", "tool", "good", "bad"]
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
fn main() -> Int {
    return 0
}
"#,
        );
        temp.write(
            "workspace/tool/qlang.toml",
            r#"
[package]
name = "tool"

[references]
packages = ["../good", "../bad"]
"#,
        );
        temp.write(
            "workspace/tool/src/helper.ql",
            r#"
fn tool_helper() -> Int {
    return 1
}
"#,
        );
        temp.write(
            "workspace/good/qlang.toml",
            r#"
[package]
name = "good"
"#,
        );
        let dependency_interface_path = temp.write(
            "workspace/good/good.qi",
            r#"
// qlang interface v1
// package: good

// source: src/lib.ql
package demo.good

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int
}
"#,
        );
        temp.write(
            "workspace/bad/qlang.toml",
            r#"
[package
name = "bad"
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let symbols = workspace_symbols_for_documents(vec![(open_uri, open_source)], "get");

        assert_eq!(
            symbols,
            vec![SymbolInformation {
                name: "get".to_owned(),
                kind: SymbolKind::METHOD,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(
                        fs::canonicalize(&dependency_interface_path)
                            .expect("dependency interface path should canonicalize"),
                    )
                    .expect("dependency interface path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        tower_lsp::lsp_types::Position::new(12, 8),
                        tower_lsp::lsp_types::Position::new(12, 27),
                    ),
                ),
                container_name: Some("good".to_owned()),
            }]
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_keeps_member_dependency_trait_and_extend_methods_for_open_packages_when_member_has_source_diagnostics()
     {
        let temp =
            TempDir::new("ql-lsp-workspace-symbol-open-broken-member-trait-and-extend-methods");

        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["app", "tool", "dep"]
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
fn main() -> Int {
    return 0
}
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"
"#,
        );
        temp.write(
            "workspace/tool/qlang.toml",
            r#"
[package]
name = "tool"

[references]
packages = ["../dep"]
"#,
        );
        temp.write(
            "workspace/tool/src/helper.ql",
            r#"
fn tool_helper() -> Int {
    return 1
}
"#,
        );
        temp.write(
            "workspace/tool/src/broken.ql",
            r#"
fn broken() -> Int {
    let value: Int = "oops"
    return value
}
"#,
        );
        temp.write(
            "workspace/dep/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        let dependency_interface_path = temp.write(
            "workspace/dep/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub trait Reader {
    fn poll(self) -> Int
}

pub struct Buffer {
    value: Int,
}

extend Buffer {
    pub fn twice(self) -> Int
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let trait_symbols =
            workspace_symbols_for_documents(vec![(open_uri.clone(), open_source.clone())], "poll");
        let extend_symbols =
            workspace_symbols_for_documents(vec![(open_uri, open_source)], "twice");

        assert_single_dependency_method_symbol(
            trait_symbols,
            "poll",
            &dependency_interface_path,
            8,
            4,
            24,
            "dep",
        );
        assert_single_dependency_method_symbol(
            extend_symbols,
            "twice",
            &dependency_interface_path,
            16,
            8,
            29,
            "dep",
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_keeps_dependency_trait_and_extend_methods_for_broken_open_packages()
    {
        let temp =
            TempDir::new("ql-lsp-workspace-symbol-broken-dependency-trait-and-extend-methods");

        temp.write(
            "workspace/dep/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        let dependency_interface_path = temp.write(
            "workspace/dep/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub trait Reader {
    fn poll(self) -> Int
}

pub struct Buffer {
    value: Int,
}

extend Buffer {
    pub fn twice(self) -> Int
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
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
package demo.app

fn main() -> Int {
    let broken: Int = "oops"
    return 0
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let trait_symbols =
            workspace_symbols_for_documents(vec![(open_uri.clone(), open_source.clone())], "poll");
        let extend_symbols =
            workspace_symbols_for_documents(vec![(open_uri, open_source)], "twice");

        assert_single_dependency_method_symbol(
            trait_symbols,
            "poll",
            &dependency_interface_path,
            8,
            4,
            24,
            "dep",
        );
        assert_single_dependency_method_symbol(
            extend_symbols,
            "twice",
            &dependency_interface_path,
            16,
            8,
            29,
            "dep",
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_keeps_same_named_dependency_method_symbols_for_broken_open_packages_with_local_dependencies()
     {
        let temp =
            TempDir::new("ql-lsp-workspace-symbol-broken-same-name-local-dependency-methods");
        let fixture = setup_same_named_dependency_method_symbols_broken_fixture(&temp);
        let open_source = fs::read_to_string(&fixture.open_path).expect("open file should read");
        let open_uri =
            Url::from_file_path(&fixture.open_path).expect("open path should convert to URI");

        let get_symbols =
            workspace_symbols_for_documents(vec![(open_uri.clone(), open_source.clone())], "get");
        let trait_symbols =
            workspace_symbols_for_documents(vec![(open_uri.clone(), open_source.clone())], "poll");
        let extend_symbols =
            workspace_symbols_for_documents(vec![(open_uri, open_source)], "twice");

        assert_source_and_dependency_method_symbols(
            get_symbols,
            "get",
            &fixture.dependency_source_path,
            &fixture.dependency_source,
            1,
            &fixture.dependency_interface_path,
            12,
            8,
            27,
            "dep",
        );
        assert_source_and_dependency_method_symbols(
            trait_symbols,
            "poll",
            &fixture.dependency_source_path,
            &fixture.dependency_source,
            1,
            &fixture.dependency_interface_path,
            16,
            4,
            24,
            "dep",
        );
        assert_source_and_dependency_method_symbols(
            extend_symbols,
            "twice",
            &fixture.dependency_source_path,
            &fixture.dependency_source,
            1,
            &fixture.dependency_interface_path,
            24,
            8,
            29,
            "dep",
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_keeps_same_named_dependency_interface_symbols_for_broken_open_packages_with_local_dependencies()
     {
        let temp =
            TempDir::new("ql-lsp-workspace-symbol-broken-same-name-local-dependency-interface");
        let fixture = setup_same_named_dependency_interface_symbols_broken_fixture(&temp);
        let open_source = fs::read_to_string(&fixture.open_path).expect("open file should read");
        let open_uri =
            Url::from_file_path(&fixture.open_path).expect("open path should convert to URI");

        let symbols = workspace_symbols_for_documents(vec![(open_uri, open_source)], "beta");

        assert_single_dependency_symbol(
            symbols,
            "beta",
            SymbolKind::FUNCTION,
            &fixture.dependency_interface_path,
            7,
            4,
            20,
            "dep",
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_keeps_same_named_dependency_type_symbols_for_broken_open_packages_with_local_dependencies()
     {
        let temp = TempDir::new("ql-lsp-workspace-symbol-broken-same-name-local-dependency-types");
        let fixture = setup_same_named_dependency_method_symbols_broken_fixture(&temp);
        let open_source = fs::read_to_string(&fixture.open_path).expect("open file should read");
        let open_uri =
            Url::from_file_path(&fixture.open_path).expect("open path should convert to URI");

        let config_symbols = workspace_symbols_for_documents(
            vec![(open_uri.clone(), open_source.clone())],
            "Config",
        );
        let reader_symbols = workspace_symbols_for_documents(
            vec![(open_uri.clone(), open_source.clone())],
            "Reader",
        );
        let buffer_symbols =
            workspace_symbols_for_documents(vec![(open_uri, open_source)], "Buffer");

        assert_source_and_dependency_symbols(
            config_symbols,
            "Config",
            SymbolKind::STRUCT,
            &fixture.dependency_source_path,
            &fixture.dependency_source,
            1,
            &fixture.dependency_interface_path,
            7,
            0,
            9,
            1,
            "dep",
        );
        assert_source_and_dependency_symbols(
            reader_symbols,
            "Reader",
            SymbolKind::INTERFACE,
            &fixture.dependency_source_path,
            &fixture.dependency_source,
            1,
            &fixture.dependency_interface_path,
            15,
            0,
            17,
            1,
            "dep",
        );
        assert_source_and_dependency_symbols(
            buffer_symbols,
            "Buffer",
            SymbolKind::STRUCT,
            &fixture.dependency_source_path,
            &fixture.dependency_source,
            1,
            &fixture.dependency_interface_path,
            19,
            0,
            21,
            1,
            "dep",
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_keeps_same_named_dependency_enum_symbols_for_broken_open_packages_with_local_dependencies()
     {
        let temp = TempDir::new("ql-lsp-workspace-symbol-broken-same-name-local-dependency-enums");
        let fixture = setup_same_named_dependency_enum_symbols_broken_fixture(&temp);
        let open_source = fs::read_to_string(&fixture.open_path).expect("open file should read");
        let open_uri =
            Url::from_file_path(&fixture.open_path).expect("open path should convert to URI");

        let enum_symbols = workspace_symbols_for_documents(
            vec![(open_uri.clone(), open_source.clone())],
            "Command",
        );
        let variant_symbols =
            workspace_symbols_for_documents(vec![(open_uri, open_source)], "Retry");

        assert_source_and_dependency_symbols(
            enum_symbols,
            "Command",
            SymbolKind::ENUM,
            &fixture.dependency_source_path,
            &fixture.dependency_source,
            1,
            &fixture.dependency_interface_path,
            7,
            0,
            9,
            1,
            "core",
        );
        assert_source_and_dependency_symbols(
            variant_symbols,
            "Retry",
            SymbolKind::ENUM_MEMBER,
            &fixture.dependency_source_path,
            &fixture.dependency_source,
            1,
            &fixture.dependency_interface_path,
            8,
            4,
            8,
            9,
            "core",
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_keeps_available_dependency_trait_and_extend_methods_when_one_package_interface_is_missing()
     {
        let temp =
            TempDir::new("ql-lsp-workspace-symbol-partial-dependency-trait-and-extend-methods");

        temp.write(
            "workspace/good/qlang.toml",
            r#"
[package]
name = "good"
"#,
        );
        let dependency_interface_path = temp.write(
            "workspace/good/good.qi",
            r#"
// qlang interface v1
// package: good

// source: src/lib.ql
package demo.good

pub trait Reader {
    fn poll(self) -> Int
}

pub struct Buffer {
    value: Int,
}

extend Buffer {
    pub fn twice(self) -> Int
}
"#,
        );
        temp.write(
            "workspace/bad/qlang.toml",
            r#"
[package]
name = "bad"
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../good", "../bad"]
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
fn main() -> Int {
    return 0
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let trait_symbols =
            workspace_symbols_for_documents(vec![(open_uri.clone(), open_source.clone())], "poll");
        let extend_symbols =
            workspace_symbols_for_documents(vec![(open_uri, open_source)], "twice");

        assert_single_dependency_method_symbol(
            trait_symbols,
            "poll",
            &dependency_interface_path,
            8,
            4,
            24,
            "good",
        );
        assert_single_dependency_method_symbol(
            extend_symbols,
            "twice",
            &dependency_interface_path,
            16,
            8,
            29,
            "good",
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_keeps_available_dependency_trait_and_extend_methods_when_reference_manifest_is_invalid()
     {
        let temp = TempDir::new(
            "ql-lsp-workspace-symbol-invalid-reference-manifest-trait-and-extend-methods",
        );

        temp.write(
            "workspace/good/qlang.toml",
            r#"
[package]
name = "good"
"#,
        );
        let dependency_interface_path = temp.write(
            "workspace/good/good.qi",
            r#"
// qlang interface v1
// package: good

// source: src/lib.ql
package demo.good

pub trait Reader {
    fn poll(self) -> Int
}

pub struct Buffer {
    value: Int,
}

extend Buffer {
    pub fn twice(self) -> Int
}
"#,
        );
        temp.write(
            "workspace/bad/qlang.toml",
            r#"
[package
name = "bad"
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../good", "../bad"]
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
fn main() -> Int {
    return 0
}
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let trait_symbols =
            workspace_symbols_for_documents(vec![(open_uri.clone(), open_source.clone())], "poll");
        let extend_symbols =
            workspace_symbols_for_documents(vec![(open_uri, open_source)], "twice");

        assert_single_dependency_method_symbol(
            trait_symbols,
            "poll",
            &dependency_interface_path,
            8,
            4,
            24,
            "good",
        );
        assert_single_dependency_method_symbol(
            extend_symbols,
            "twice",
            &dependency_interface_path,
            16,
            8,
            29,
            "good",
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_keeps_available_member_dependency_trait_and_extend_methods_when_one_member_interface_is_missing()
     {
        let temp = TempDir::new(
            "ql-lsp-workspace-symbol-partial-member-dependency-trait-and-extend-methods",
        );

        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["app", "tool", "good", "bad"]
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
fn main() -> Int {
    return 0
}
"#,
        );
        temp.write(
            "workspace/tool/qlang.toml",
            r#"
[package]
name = "tool"

[references]
packages = ["../good", "../bad"]
"#,
        );
        temp.write(
            "workspace/tool/src/helper.ql",
            r#"
fn tool_helper() -> Int {
    return 1
}
"#,
        );
        temp.write(
            "workspace/good/qlang.toml",
            r#"
[package]
name = "good"
"#,
        );
        let dependency_interface_path = temp.write(
            "workspace/good/good.qi",
            r#"
// qlang interface v1
// package: good

// source: src/lib.ql
package demo.good

pub trait Reader {
    fn poll(self) -> Int
}

pub struct Buffer {
    value: Int,
}

extend Buffer {
    pub fn twice(self) -> Int
}
"#,
        );
        temp.write(
            "workspace/bad/qlang.toml",
            r#"
[package]
name = "bad"
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let trait_symbols =
            workspace_symbols_for_documents(vec![(open_uri.clone(), open_source.clone())], "poll");
        let extend_symbols =
            workspace_symbols_for_documents(vec![(open_uri, open_source)], "twice");

        assert_single_dependency_method_symbol(
            trait_symbols,
            "poll",
            &dependency_interface_path,
            8,
            4,
            24,
            "good",
        );
        assert_single_dependency_method_symbol(
            extend_symbols,
            "twice",
            &dependency_interface_path,
            16,
            8,
            29,
            "good",
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_keeps_available_member_dependency_trait_and_extend_methods_when_member_reference_manifest_is_invalid()
     {
        let temp = TempDir::new(
            "ql-lsp-workspace-symbol-invalid-member-reference-manifest-trait-and-extend-methods",
        );

        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["app", "tool", "good", "bad"]
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"
"#,
        );
        let open_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
fn main() -> Int {
    return 0
}
"#,
        );
        temp.write(
            "workspace/tool/qlang.toml",
            r#"
[package]
name = "tool"

[references]
packages = ["../good", "../bad"]
"#,
        );
        temp.write(
            "workspace/tool/src/helper.ql",
            r#"
fn tool_helper() -> Int {
    return 1
}
"#,
        );
        temp.write(
            "workspace/good/qlang.toml",
            r#"
[package]
name = "good"
"#,
        );
        let dependency_interface_path = temp.write(
            "workspace/good/good.qi",
            r#"
// qlang interface v1
// package: good

// source: src/lib.ql
package demo.good

pub trait Reader {
    fn poll(self) -> Int
}

pub struct Buffer {
    value: Int,
}

extend Buffer {
    pub fn twice(self) -> Int
}
"#,
        );
        temp.write(
            "workspace/bad/qlang.toml",
            r#"
[package
name = "bad"
"#,
        );
        let open_source = fs::read_to_string(&open_path).expect("open file should read");
        let open_uri = Url::from_file_path(&open_path).expect("open path should convert to URI");

        let trait_symbols =
            workspace_symbols_for_documents(vec![(open_uri.clone(), open_source.clone())], "poll");
        let extend_symbols =
            workspace_symbols_for_documents(vec![(open_uri, open_source)], "twice");

        assert_single_dependency_method_symbol(
            trait_symbols,
            "poll",
            &dependency_interface_path,
            8,
            4,
            24,
            "good",
        );
        assert_single_dependency_method_symbol(
            extend_symbols,
            "twice",
            &dependency_interface_path,
            16,
            8,
            29,
            "good",
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_uses_workspace_roots_without_open_documents() {
        let temp = TempDir::new("ql-lsp-workspace-symbol-roots");
        let workspace_root = temp.path().join("workspace");
        let helper_path = temp.write(
            "workspace/packages/tool/src/helper.ql",
            r#"
package demo.tool

pub fn helper() -> Int {
    return 1
}
"#,
        );
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
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

pub fn main() -> Int {
    return 0
}
"#,
        );
        temp.write(
            "workspace/packages/tool/qlang.toml",
            r#"
[package]
name = "tool"
"#,
        );

        let symbols =
            workspace_symbols_for_documents_and_roots(Vec::new(), &[workspace_root], "helper");

        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "helper");
        assert_eq!(
            symbols[0]
                .location
                .uri
                .to_file_path()
                .expect("workspace symbol path should convert")
                .canonicalize()
                .expect("workspace symbol path should canonicalize"),
            helper_path
                .canonicalize()
                .expect("helper path should canonicalize"),
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_uses_workspace_roots_and_prefers_local_dependency_source_symbols() {
        let temp = TempDir::new("ql-lsp-workspace-symbol-root-local-dependency-source");
        let workspace_root = temp.path().join("workspace");
        let dependency_source_path = temp.write(
            "workspace/vendor/dep/src/lib.ql",
            r#"
package demo.dep

pub fn exported(value: Int) -> Int {
    return value
}
"#,
        );
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
dep = { path = "../../vendor/dep" }
"#,
        );
        temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.dep.exported as run

pub fn main() -> Int {
    return run(1)
}
"#,
        );
        temp.write(
            "workspace/vendor/dep/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        temp.write(
            "workspace/vendor/dep/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub fn exported(value: Int) -> Int
"#,
        );

        let dependency_source =
            fs::read_to_string(&dependency_source_path).expect("dependency source should read");
        let symbols =
            workspace_symbols_for_documents_and_roots(Vec::new(), &[workspace_root], "exported");

        assert_eq!(
            symbols,
            vec![SymbolInformation {
                name: "exported".to_owned(),
                kind: SymbolKind::FUNCTION,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(&dependency_source_path)
                        .expect("dependency source path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        offset_to_position(
                            &dependency_source,
                            nth_offset(&dependency_source, "exported", 1),
                        ),
                        offset_to_position(
                            &dependency_source,
                            nth_offset(&dependency_source, "exported", 1) + "exported".len(),
                        ),
                    ),
                ),
                container_name: None,
            }]
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_uses_workspace_roots_and_keeps_same_named_dependency_interface_symbols()
     {
        let temp =
            TempDir::new("ql-lsp-workspace-symbol-root-same-name-local-dependency-interface");
        let workspace_root = temp.path().join("workspace");

        temp.write(
            "workspace/vendor/dep-source/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        temp.write(
            "workspace/vendor/dep-source/src/lib.ql",
            r#"
package demo.dep.source

pub fn alpha() -> Int {
    return 1
}
"#,
        );
        temp.write(
            "workspace/vendor/dep-source/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep.source

pub fn alpha() -> Int
"#,
        );
        temp.write(
            "workspace/vendor/dep-interface/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        let dependency_interface_path = temp.write(
            "workspace/vendor/dep-interface/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep.interface

pub fn beta() -> Int
"#,
        );
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

[references]
packages = ["../../vendor/dep-source", "../../vendor/dep-interface"]
"#,
        );
        temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
pub fn main() -> Int {
    return 0
}
"#,
        );

        let symbols =
            workspace_symbols_for_documents_and_roots(Vec::new(), &[workspace_root], "beta");

        assert_single_dependency_symbol(
            symbols,
            "beta",
            SymbolKind::FUNCTION,
            &dependency_interface_path,
            7,
            4,
            20,
            "dep",
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_uses_workspace_roots_and_keeps_same_named_dependency_method_symbols()
    {
        let temp = TempDir::new("ql-lsp-workspace-symbol-root-same-name-local-dependency-methods");
        let workspace_root = temp.path().join("workspace");

        temp.write(
            "workspace/vendor/dep-source/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        let dependency_source_path = temp.write(
            "workspace/vendor/dep-source/src/lib.ql",
            r#"
package demo.dep.source

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int {
        return self.value
    }
}

pub trait Reader {
    fn poll(self) -> Int
}

pub struct Buffer {
    value: Int,
}

extend Buffer {
    pub fn twice(self) -> Int {
        return 2
    }
}
"#,
        );
        temp.write(
            "workspace/vendor/dep-source/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep.source

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int
}

pub trait Reader {
    fn poll(self) -> Int
}

pub struct Buffer {
    value: Int,
}

extend Buffer {
    pub fn twice(self) -> Int
}
"#,
        );
        temp.write(
            "workspace/vendor/dep-interface/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        let dependency_interface_path = temp.write(
            "workspace/vendor/dep-interface/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep.interface

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int
}

pub trait Reader {
    fn poll(self) -> Int
}

pub struct Buffer {
    value: Int,
}

extend Buffer {
    pub fn twice(self) -> Int
}
"#,
        );
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

[references]
packages = ["../../vendor/dep-source", "../../vendor/dep-interface"]
"#,
        );
        temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
pub fn main() -> Int {
    return 0
}
"#,
        );

        let dependency_source =
            fs::read_to_string(&dependency_source_path).expect("dependency source should read");
        let get_symbols =
            workspace_symbols_for_documents_and_roots(Vec::new(), &[workspace_root.clone()], "get");
        let trait_symbols = workspace_symbols_for_documents_and_roots(
            Vec::new(),
            &[workspace_root.clone()],
            "poll",
        );
        let extend_symbols =
            workspace_symbols_for_documents_and_roots(Vec::new(), &[workspace_root], "twice");

        assert_source_and_dependency_method_symbols(
            get_symbols,
            "get",
            &dependency_source_path,
            &dependency_source,
            1,
            &dependency_interface_path,
            12,
            8,
            27,
            "dep",
        );
        assert_source_and_dependency_method_symbols(
            trait_symbols,
            "poll",
            &dependency_source_path,
            &dependency_source,
            1,
            &dependency_interface_path,
            16,
            4,
            24,
            "dep",
        );
        assert_source_and_dependency_method_symbols(
            extend_symbols,
            "twice",
            &dependency_source_path,
            &dependency_source,
            1,
            &dependency_interface_path,
            24,
            8,
            29,
            "dep",
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_uses_workspace_roots_and_keeps_same_named_dependency_type_symbols_with_local_dependencies()
     {
        let temp = TempDir::new("ql-lsp-workspace-symbol-root-same-name-local-dependency-types");
        let fixture = setup_same_named_dependency_method_symbols_local_fixture(&temp);

        let config_symbols = workspace_symbols_for_documents_and_roots(
            Vec::new(),
            &[fixture.workspace_root.clone()],
            "Config",
        );
        let reader_symbols = workspace_symbols_for_documents_and_roots(
            Vec::new(),
            &[fixture.workspace_root.clone()],
            "Reader",
        );
        let buffer_symbols = workspace_symbols_for_documents_and_roots(
            Vec::new(),
            &[fixture.workspace_root],
            "Buffer",
        );

        assert_source_and_dependency_symbols(
            config_symbols,
            "Config",
            SymbolKind::STRUCT,
            &fixture.dependency_source_path,
            &fixture.dependency_source,
            1,
            &fixture.dependency_interface_path,
            7,
            0,
            9,
            1,
            "dep",
        );
        assert_source_and_dependency_symbols(
            reader_symbols,
            "Reader",
            SymbolKind::INTERFACE,
            &fixture.dependency_source_path,
            &fixture.dependency_source,
            1,
            &fixture.dependency_interface_path,
            11,
            0,
            13,
            1,
            "dep",
        );
        assert_source_and_dependency_symbols(
            buffer_symbols,
            "Buffer",
            SymbolKind::STRUCT,
            &fixture.dependency_source_path,
            &fixture.dependency_source,
            1,
            &fixture.dependency_interface_path,
            15,
            0,
            17,
            1,
            "dep",
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_uses_workspace_roots_and_keeps_same_named_dependency_enum_symbols_with_local_dependencies()
     {
        let temp = TempDir::new("ql-lsp-workspace-symbol-root-same-name-local-dependency-enums");
        let fixture = setup_same_named_dependency_enum_symbols_fixture(&temp);

        let enum_symbols = workspace_symbols_for_documents_and_roots(
            Vec::new(),
            &[fixture.workspace_root.clone()],
            "Command",
        );
        let variant_symbols = workspace_symbols_for_documents_and_roots(
            Vec::new(),
            &[fixture.workspace_root],
            "Retry",
        );

        assert_source_and_dependency_symbols(
            enum_symbols,
            "Command",
            SymbolKind::ENUM,
            &fixture.dependency_source_path,
            &fixture.dependency_source,
            1,
            &fixture.dependency_interface_path,
            7,
            0,
            9,
            1,
            "core",
        );
        assert_source_and_dependency_symbols(
            variant_symbols,
            "Retry",
            SymbolKind::ENUM_MEMBER,
            &fixture.dependency_source_path,
            &fixture.dependency_source,
            1,
            &fixture.dependency_interface_path,
            8,
            4,
            8,
            9,
            "core",
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_uses_workspace_roots_and_prefers_local_dependency_source_methods() {
        let temp = TempDir::new("ql-lsp-workspace-symbol-root-local-dependency-source-method");
        let workspace_root = temp.path().join("workspace");
        let dependency_source_path = temp.write(
            "workspace/vendor/dep/src/lib.ql",
            r#"
package demo.dep

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int {
        return self.value
    }
}
"#,
        );
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
dep = { path = "../../vendor/dep" }
"#,
        );
        temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.dep.Config as Cfg

pub fn main(config: Cfg) -> Int {
    return config.get()
}
"#,
        );
        temp.write(
            "workspace/vendor/dep/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        temp.write(
            "workspace/vendor/dep/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int
}
"#,
        );

        let dependency_source =
            fs::read_to_string(&dependency_source_path).expect("dependency source should read");
        let symbols =
            workspace_symbols_for_documents_and_roots(Vec::new(), &[workspace_root], "get");

        assert_single_source_symbol(
            symbols,
            "get",
            SymbolKind::METHOD,
            &dependency_source_path,
            &dependency_source,
            1,
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_uses_workspace_roots_and_prefers_local_dependency_source_trait_and_extend_methods()
     {
        let temp = TempDir::new(
            "ql-lsp-workspace-symbol-root-local-dependency-source-trait-extend-methods",
        );
        let workspace_root = temp.path().join("workspace");
        let dependency_source_path = temp.write(
            "workspace/vendor/dep/src/lib.ql",
            r#"
package demo.dep

pub trait Reader {
    fn poll(self) -> Int
}

pub struct Buffer {
    value: Int,
}

extend Buffer {
    pub fn twice(self) -> Int {
        return 2
    }
}
"#,
        );
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
dep = { path = "../../vendor/dep" }
"#,
        );
        temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
pub fn main() -> Int {
    return 0
}
"#,
        );
        temp.write(
            "workspace/vendor/dep/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        temp.write(
            "workspace/vendor/dep/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub trait Reader {
    fn poll(self) -> Int
}

pub struct Buffer {
    value: Int,
}

extend Buffer {
    pub fn twice(self) -> Int
}
"#,
        );

        let dependency_source =
            fs::read_to_string(&dependency_source_path).expect("dependency source should read");
        let trait_symbols = workspace_symbols_for_documents_and_roots(
            Vec::new(),
            &[workspace_root.clone()],
            "poll",
        );
        let extend_symbols =
            workspace_symbols_for_documents_and_roots(Vec::new(), &[workspace_root], "twice");

        assert_single_source_symbol(
            trait_symbols,
            "poll",
            SymbolKind::METHOD,
            &dependency_source_path,
            &dependency_source,
            1,
        );
        assert_single_source_symbol(
            extend_symbols,
            "twice",
            SymbolKind::METHOD,
            &dependency_source_path,
            &dependency_source,
            1,
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_uses_workspace_roots_and_prefers_local_dependency_source_symbols_for_broken_members()
     {
        let temp = TempDir::new("ql-lsp-workspace-symbol-root-broken-local-dependency-source");
        let workspace_root = temp.path().join("workspace");
        let dependency_source_path = temp.write(
            "workspace/vendor/dep/src/lib.ql",
            r#"
package demo.dep

pub fn exported(value: Int) -> Int {
    return value
}
"#,
        );
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
dep = { path = "../../vendor/dep" }
"#,
        );
        temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.dep.exported as run

pub fn main() -> Int {
    let broken: Int = "oops"
    return run(1)
}
"#,
        );
        temp.write(
            "workspace/vendor/dep/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        temp.write(
            "workspace/vendor/dep/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub fn exported(value: Int) -> Int
"#,
        );

        let dependency_source =
            fs::read_to_string(&dependency_source_path).expect("dependency source should read");
        let symbols =
            workspace_symbols_for_documents_and_roots(Vec::new(), &[workspace_root], "exported");

        assert_eq!(
            symbols,
            vec![SymbolInformation {
                name: "exported".to_owned(),
                kind: SymbolKind::FUNCTION,
                tags: None,
                deprecated: None,
                location: Location::new(
                    Url::from_file_path(&dependency_source_path)
                        .expect("dependency source path should convert to URI"),
                    tower_lsp::lsp_types::Range::new(
                        offset_to_position(
                            &dependency_source,
                            nth_offset(&dependency_source, "exported", 1),
                        ),
                        offset_to_position(
                            &dependency_source,
                            nth_offset(&dependency_source, "exported", 1) + "exported".len(),
                        ),
                    ),
                ),
                container_name: None,
            }]
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_uses_workspace_roots_and_keeps_same_named_dependency_method_symbols_for_broken_members_with_local_dependencies()
     {
        let temp =
            TempDir::new("ql-lsp-workspace-symbol-root-broken-same-name-local-dependency-methods");
        let fixture = setup_same_named_dependency_method_symbols_broken_fixture(&temp);

        let get_symbols = workspace_symbols_for_documents_and_roots(
            Vec::new(),
            &[fixture.workspace_root.clone()],
            "get",
        );
        let trait_symbols = workspace_symbols_for_documents_and_roots(
            Vec::new(),
            &[fixture.workspace_root.clone()],
            "poll",
        );
        let extend_symbols = workspace_symbols_for_documents_and_roots(
            Vec::new(),
            &[fixture.workspace_root],
            "twice",
        );

        assert_source_and_dependency_method_symbols(
            get_symbols,
            "get",
            &fixture.dependency_source_path,
            &fixture.dependency_source,
            1,
            &fixture.dependency_interface_path,
            12,
            8,
            27,
            "dep",
        );
        assert_source_and_dependency_method_symbols(
            trait_symbols,
            "poll",
            &fixture.dependency_source_path,
            &fixture.dependency_source,
            1,
            &fixture.dependency_interface_path,
            16,
            4,
            24,
            "dep",
        );
        assert_source_and_dependency_method_symbols(
            extend_symbols,
            "twice",
            &fixture.dependency_source_path,
            &fixture.dependency_source,
            1,
            &fixture.dependency_interface_path,
            24,
            8,
            29,
            "dep",
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_uses_workspace_roots_and_keeps_same_named_dependency_interface_symbols_for_broken_members_with_local_dependencies()
     {
        let temp = TempDir::new(
            "ql-lsp-workspace-symbol-root-broken-same-name-local-dependency-interface",
        );
        let fixture = setup_same_named_dependency_interface_symbols_broken_fixture(&temp);

        let symbols = workspace_symbols_for_documents_and_roots(
            Vec::new(),
            &[fixture.workspace_root],
            "beta",
        );

        assert_single_dependency_symbol(
            symbols,
            "beta",
            SymbolKind::FUNCTION,
            &fixture.dependency_interface_path,
            7,
            4,
            20,
            "dep",
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_uses_workspace_roots_and_keeps_same_named_dependency_type_symbols_for_broken_members_with_local_dependencies()
     {
        let temp =
            TempDir::new("ql-lsp-workspace-symbol-root-broken-same-name-local-dependency-types");
        let fixture = setup_same_named_dependency_method_symbols_broken_fixture(&temp);

        let config_symbols = workspace_symbols_for_documents_and_roots(
            Vec::new(),
            &[fixture.workspace_root.clone()],
            "Config",
        );
        let reader_symbols = workspace_symbols_for_documents_and_roots(
            Vec::new(),
            &[fixture.workspace_root.clone()],
            "Reader",
        );
        let buffer_symbols = workspace_symbols_for_documents_and_roots(
            Vec::new(),
            &[fixture.workspace_root],
            "Buffer",
        );

        assert_source_and_dependency_symbols(
            config_symbols,
            "Config",
            SymbolKind::STRUCT,
            &fixture.dependency_source_path,
            &fixture.dependency_source,
            1,
            &fixture.dependency_interface_path,
            7,
            0,
            9,
            1,
            "dep",
        );
        assert_source_and_dependency_symbols(
            reader_symbols,
            "Reader",
            SymbolKind::INTERFACE,
            &fixture.dependency_source_path,
            &fixture.dependency_source,
            1,
            &fixture.dependency_interface_path,
            15,
            0,
            17,
            1,
            "dep",
        );
        assert_source_and_dependency_symbols(
            buffer_symbols,
            "Buffer",
            SymbolKind::STRUCT,
            &fixture.dependency_source_path,
            &fixture.dependency_source,
            1,
            &fixture.dependency_interface_path,
            19,
            0,
            21,
            1,
            "dep",
        );
    }

    #[allow(deprecated)]
    #[test]
    fn workspace_symbol_search_uses_workspace_roots_and_keeps_same_named_dependency_enum_symbols_for_broken_members_with_local_dependencies()
     {
        let temp =
            TempDir::new("ql-lsp-workspace-symbol-root-broken-same-name-local-dependency-enums");
        let fixture = setup_same_named_dependency_enum_symbols_broken_fixture(&temp);

        let enum_symbols = workspace_symbols_for_documents_and_roots(
            Vec::new(),
            &[fixture.workspace_root.clone()],
            "Command",
        );
        let variant_symbols = workspace_symbols_for_documents_and_roots(
            Vec::new(),
            &[fixture.workspace_root],
            "Retry",
        );

        assert_source_and_dependency_symbols(
            enum_symbols,
            "Command",
            SymbolKind::ENUM,
            &fixture.dependency_source_path,
            &fixture.dependency_source,
            1,
            &fixture.dependency_interface_path,
            7,
            0,
            9,
            1,
            "core",
        );
        assert_source_and_dependency_symbols(
            variant_symbols,
            "Retry",
            SymbolKind::ENUM_MEMBER,
            &fixture.dependency_source_path,
            &fixture.dependency_source,
            1,
            &fixture.dependency_interface_path,
            8,
            4,
            8,
            9,
            "core",
        );
    }

    #[test]
    fn workspace_import_definition_prefers_workspace_member_source_over_interface_artifact() {
        let temp = TempDir::new("ql-lsp-workspace-import-source-definition");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.exported as run

pub fn main() -> Int {
    return run(1)
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn exported(value: Int) -> Int {
    return value
}

pub fn wrapper(value: Int) -> Int {
    return exported(value)
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn exported(value: Int) -> Int
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        let analysis = analyze_source(&source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");

        let definition = workspace_source_definition_for_import(
            &uri,
            &source,
            &analysis,
            &package,
            offset_to_position(&source, nth_offset(&source, "run", 2)),
        )
        .expect("workspace import definition should exist");

        let GotoDefinitionResponse::Scalar(location) = definition else {
            panic!("workspace import definition should resolve to one location")
        };
        assert_eq!(
            location
                .uri
                .to_file_path()
                .expect("definition URI should convert to a file path")
                .canonicalize()
                .expect("definition path should canonicalize"),
            core_source_path
                .canonicalize()
                .expect("core source path should canonicalize"),
        );
    }

    #[test]
    fn local_dependency_import_definition_prefers_dependency_source_over_interface_artifact() {
        let temp = TempDir::new("ql-lsp-local-dependency-import-source-definition");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.dep.exported as run

pub fn main() -> Int {
    return run(1)
}
"#,
        );
        let dep_source_path = temp.write(
            "workspace/vendor/dep/src/lib.ql",
            r#"
package demo.dep

pub fn exported(value: Int) -> Int {
    return value
}

pub fn wrapper(value: Int) -> Int {
    return exported(value)
}
"#,
        );
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

[references]
packages = ["../../vendor/dep"]
"#,
        );
        temp.write(
            "workspace/vendor/dep/qlang.toml",
            r#"
[package]
name = "dep"
"#,
        );
        temp.write(
            "workspace/vendor/dep/dep.qi",
            r#"
// qlang interface v1
// package: dep

// source: src/lib.ql
package demo.dep

pub fn exported(value: Int) -> Int
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        let analysis = analyze_source(&source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");

        let definition = workspace_source_definition_for_import(
            &uri,
            &source,
            &analysis,
            &package,
            offset_to_position(&source, nth_offset(&source, "run", 2)),
        )
        .expect("local dependency import definition should exist");

        let GotoDefinitionResponse::Scalar(location) = definition else {
            panic!("local dependency import definition should resolve to one location")
        };
        assert_eq!(
            location
                .uri
                .to_file_path()
                .expect("definition URI should convert to a file path")
                .canonicalize()
                .expect("definition path should canonicalize"),
            dep_source_path
                .canonicalize()
                .expect("dependency source path should canonicalize"),
        );
    }

    #[test]
    fn workspace_import_definition_prefers_open_workspace_source() {
        let temp = TempDir::new("ql-lsp-workspace-import-source-definition-open-docs");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.measure as run

pub fn main() -> Int {
    return run(1)
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn helper() -> Int {
    return 0
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn measure(value: Int) -> Int
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        let analysis = analyze_source(&source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let core_uri =
            Url::from_file_path(&core_source_path).expect("core path should convert to URI");
        let open_core_source = r#"
package demo.core

pub fn helper() -> Int {
    return 0
}

pub fn measure(value: Int) -> Int {
    return value
}
"#
        .to_owned();

        assert_eq!(
            workspace_source_definition_for_import(
                &uri,
                &source,
                &analysis,
                &package,
                offset_to_position(&source, nth_offset(&source, "run", 2)),
            ),
            None,
            "disk-only definition should miss unsaved workspace source",
        );

        let definition = workspace_source_definition_for_import_with_open_docs(
            &uri,
            &source,
            &analysis,
            &package,
            &file_open_documents(vec![
                (uri.clone(), source.clone()),
                (core_uri, open_core_source),
            ]),
            offset_to_position(&source, nth_offset(&source, "run", 2)),
        )
        .expect("workspace import definition should use open workspace source");

        let GotoDefinitionResponse::Scalar(location) = definition else {
            panic!("workspace import definition should resolve to one location")
        };
        assert_eq!(
            location
                .uri
                .to_file_path()
                .expect("definition URI should convert to a file path")
                .canonicalize()
                .expect("definition path should canonicalize"),
            core_source_path
                .canonicalize()
                .expect("core source path should canonicalize"),
        );
    }

    #[test]
    fn workspace_import_hover_prefers_workspace_member_source_over_interface_artifact() {
        let temp = TempDir::new("ql-lsp-workspace-import-source-hover");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.exported as run

pub fn main() -> Int {
    return 0
}
"#,
        );
        temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn exported(value: Int, extra: Int) -> Int {
    return value + extra
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn exported(value: Int) -> Int
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        let analysis = analyze_source(&source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");

        let hover = workspace_source_hover_for_import(
            &uri,
            &source,
            &analysis,
            &package,
            offset_to_position(&source, nth_offset(&source, "run", 1)),
        )
        .expect("workspace import hover should exist");
        let HoverContents::Markup(markup) = hover.contents else {
            panic!("hover should use markdown")
        };
        assert!(
            markup
                .value
                .contains("fn exported(value: Int, extra: Int) -> Int")
        );
        assert!(!markup.value.contains("fn exported(value: Int) -> Int"));
    }

    #[test]
    fn workspace_import_hover_prefers_open_workspace_source() {
        let temp = TempDir::new("ql-lsp-workspace-import-source-hover-open-docs");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.measure as run

pub fn main() -> Int {
    return 0
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn helper() -> Int {
    return 0
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn measure(value: Int) -> Int
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        let analysis = analyze_source(&source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let core_uri =
            Url::from_file_path(&core_source_path).expect("core path should convert to URI");
        let open_core_source = r#"
package demo.core

pub fn helper() -> Int {
    return 0
}

pub fn measure(value: Int, extra: Int) -> Int {
    return value + extra
}
"#
        .to_owned();

        assert_eq!(
            workspace_source_hover_for_import(
                &uri,
                &source,
                &analysis,
                &package,
                offset_to_position(&source, nth_offset(&source, "run", 1)),
            ),
            None,
            "disk-only hover should miss unsaved workspace source",
        );

        let hover = workspace_source_hover_for_import_with_open_docs(
            &uri,
            &source,
            &analysis,
            &package,
            &file_open_documents(vec![
                (uri.clone(), source.clone()),
                (core_uri, open_core_source),
            ]),
            offset_to_position(&source, nth_offset(&source, "run", 1)),
        )
        .expect("workspace import hover should use open workspace source");
        let HoverContents::Markup(markup) = hover.contents else {
            panic!("hover should use markdown")
        };
        assert!(
            markup
                .value
                .contains("fn measure(value: Int, extra: Int) -> Int")
        );
        assert!(!markup.value.contains("fn measure(value: Int) -> Int"));
    }

    #[test]
    fn local_dependency_import_semantic_tokens_prefer_dependency_symbol_kinds() {
        let temp = TempDir::new("ql-lsp-local-dependency-import-semantic-tokens");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.Command as Cmd
use demo.core.Config as Cfg

pub fn main(config: Cfg) -> Int {
    let built = Cfg { value: 1, limit: 2 }
    let command = Cmd.Retry(1)
    return built.value + config.value + command.unwrap_or(0)
}
"#,
        );
        temp.write(
            "workspace/vendor/core/src/lib.ql",
            r#"
package demo.core

pub enum Command {
    Retry(Int),
}

impl Command {
    pub fn unwrap_or(self, fallback: Int) -> Int {
        match self {
            Command.Retry(value) => value,
        }
    }
}

pub struct Config {
    value: Int,
    limit: Int,
}
"#,
        );
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
core = { path = "../../vendor/core" }
"#,
        );
        temp.write(
            "workspace/vendor/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/vendor/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub enum Command {
    Retry(Int),
}

pub struct Config {
    value: Int,
    limit: Int,
}
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        let analysis = analyze_source(&source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");

        let SemanticTokensResult::Tokens(tokens) =
            semantic_tokens_for_workspace_package_analysis(&uri, &source, &analysis, &package)
        else {
            panic!("expected full semantic tokens")
        };
        let decoded = decode_semantic_tokens(&tokens.data);
        let legend = semantic_tokens_legend();
        let namespace_type = legend
            .token_types
            .iter()
            .position(|token_type| *token_type == SemanticTokenType::NAMESPACE)
            .expect("namespace legend entry should exist") as u32;
        let class_type = legend
            .token_types
            .iter()
            .position(|token_type| *token_type == SemanticTokenType::CLASS)
            .expect("class legend entry should exist") as u32;
        let enum_type = legend
            .token_types
            .iter()
            .position(|token_type| *token_type == SemanticTokenType::ENUM)
            .expect("enum legend entry should exist") as u32;

        for (needle, occurrence, token_type) in [
            ("Cfg", 1usize, class_type),
            ("Cfg", 2usize, class_type),
            ("Cfg", 3usize, class_type),
            ("Cmd", 1usize, enum_type),
            ("Cmd", 2usize, enum_type),
        ] {
            let span = Span::new(
                nth_offset(&source, needle, occurrence),
                nth_offset(&source, needle, occurrence) + needle.len(),
            );
            let range = span_to_range(&source, span);
            assert!(decoded.contains(&(
                range.start.line,
                range.start.character,
                range.end.character - range.start.character,
                token_type,
            )));
        }

        for (needle, occurrence) in [("Cfg", 1usize), ("Cmd", 1usize)] {
            let span = Span::new(
                nth_offset(&source, needle, occurrence),
                nth_offset(&source, needle, occurrence) + needle.len(),
            );
            let range = span_to_range(&source, span);
            assert!(!decoded.contains(&(
                range.start.line,
                range.start.character,
                range.end.character - range.start.character,
                namespace_type,
            )));
        }
    }

    #[test]
    fn workspace_import_semantic_tokens_prefer_workspace_member_symbol_kinds() {
        let temp = TempDir::new("ql-lsp-workspace-import-semantic-tokens");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.Command as Cmd
use demo.core.Config as Cfg

pub fn main(config: Cfg) -> Int {
    let built = Cfg { value: 1, limit: 2 }
    let command = Cmd.Retry(1)
    return built.value
}
"#,
        );
        temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub enum Command {
    Retry(Int),
}

pub struct Config {
    value: Int,
    limit: Int,
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub enum Command {
    Retry(Int),
}

pub struct Config {
    value: Int,
    limit: Int,
}
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        let analysis = analyze_source(&source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");

        let SemanticTokensResult::Tokens(tokens) =
            semantic_tokens_for_workspace_package_analysis(&uri, &source, &analysis, &package)
        else {
            panic!("expected full semantic tokens")
        };
        let decoded = decode_semantic_tokens(&tokens.data);
        let legend = semantic_tokens_legend();
        let namespace_type = legend
            .token_types
            .iter()
            .position(|token_type| *token_type == SemanticTokenType::NAMESPACE)
            .expect("namespace legend entry should exist") as u32;
        let class_type = legend
            .token_types
            .iter()
            .position(|token_type| *token_type == SemanticTokenType::CLASS)
            .expect("class legend entry should exist") as u32;
        let enum_type = legend
            .token_types
            .iter()
            .position(|token_type| *token_type == SemanticTokenType::ENUM)
            .expect("enum legend entry should exist") as u32;

        for (needle, occurrence, token_type) in [
            ("Cfg", 1usize, class_type),
            ("Cfg", 2usize, class_type),
            ("Cfg", 3usize, class_type),
            ("Cmd", 1usize, enum_type),
            ("Cmd", 2usize, enum_type),
        ] {
            let span = Span::new(
                nth_offset(&source, needle, occurrence),
                nth_offset(&source, needle, occurrence) + needle.len(),
            );
            let range = span_to_range(&source, span);
            assert!(decoded.contains(&(
                range.start.line,
                range.start.character,
                range.end.character - range.start.character,
                token_type,
            )));
        }

        for (needle, occurrence) in [("Cfg", 1usize), ("Cmd", 1usize)] {
            let span = Span::new(
                nth_offset(&source, needle, occurrence),
                nth_offset(&source, needle, occurrence) + needle.len(),
            );
            let range = span_to_range(&source, span);
            assert!(!decoded.contains(&(
                range.start.line,
                range.start.character,
                range.end.character - range.start.character,
                namespace_type,
            )));
        }
    }

    #[test]
    fn workspace_import_semantic_tokens_prefer_open_workspace_source_symbol_kinds() {
        let temp = TempDir::new("ql-lsp-workspace-import-semantic-tokens-open-docs");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.Command as Cmd
use demo.core.Config as Cfg

pub fn main(config: Cfg) -> Int {
    let built = Cfg { value: 1, limit: 2 }
    let command = Cmd.Retry(1)
    return built.value + config.value + command.unwrap_or(0)
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn helper() -> Int {
    return 0
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        let analysis = analyze_source(&source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let core_uri =
            Url::from_file_path(&core_source_path).expect("core source path should convert to URI");
        let open_core_source = r#"
package demo.core

pub enum Command {
    Retry(Int),
}

impl Command {
    pub fn unwrap_or(self, fallback: Int) -> Int {
        match self {
            Command.Retry(value) => value,
        }
    }
}

pub struct Config {
    value: Int,
    limit: Int,
}
"#
        .to_owned();

        let SemanticTokensResult::Tokens(disk_tokens) =
            semantic_tokens_for_workspace_package_analysis(&uri, &source, &analysis, &package)
        else {
            panic!("expected full semantic tokens")
        };
        let disk_decoded = decode_semantic_tokens(&disk_tokens.data);

        let SemanticTokensResult::Tokens(tokens) =
            semantic_tokens_for_workspace_package_analysis_with_open_docs(
                &uri,
                &source,
                &analysis,
                &package,
                &file_open_documents(vec![
                    (uri.clone(), source.clone()),
                    (core_uri, open_core_source),
                ]),
            )
        else {
            panic!("expected full semantic tokens")
        };
        let decoded = decode_semantic_tokens(&tokens.data);
        let legend = semantic_tokens_legend();
        let class_type = legend
            .token_types
            .iter()
            .position(|token_type| *token_type == SemanticTokenType::CLASS)
            .expect("class legend entry should exist") as u32;
        let enum_type = legend
            .token_types
            .iter()
            .position(|token_type| *token_type == SemanticTokenType::ENUM)
            .expect("enum legend entry should exist") as u32;

        for (needle, occurrence, token_type) in [
            ("Cfg", 1usize, class_type),
            ("Cfg", 2usize, class_type),
            ("Cfg", 3usize, class_type),
            ("Cmd", 1usize, enum_type),
            ("Cmd", 2usize, enum_type),
        ] {
            let span = Span::new(
                nth_offset(&source, needle, occurrence),
                nth_offset(&source, needle, occurrence) + needle.len(),
            );
            let range = span_to_range(&source, span);
            assert!(!disk_decoded.contains(&(
                range.start.line,
                range.start.character,
                range.end.character - range.start.character,
                token_type,
            )));
            assert!(decoded.contains(&(
                range.start.line,
                range.start.character,
                range.end.character - range.start.character,
                token_type,
            )));
        }
    }

    #[test]
    fn workspace_import_semantic_tokens_survive_parse_errors() {
        let temp = TempDir::new("ql-lsp-workspace-import-semantic-tokens-parse-errors");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.Command as Cmd
use demo.core.Config as Cfg

pub fn main(config: Cfg) -> Int {
    let built = Cfg { value: 1, limit: 2
    let command = Cmd.Retry(1)
    return built.value
"#,
        );
        temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub enum Command {
    Retry(Int),
}

pub struct Config {
    value: Int,
    limit: Int,
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub enum Command {
    Retry(Int),
}

pub struct Config {
    value: Int,
    limit: Int,
}
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&source).is_err());
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");

        let SemanticTokensResult::Tokens(tokens) =
            semantic_tokens_for_workspace_dependency_fallback(&uri, &source, &package)
        else {
            panic!("expected full semantic tokens")
        };
        let decoded = decode_semantic_tokens(&tokens.data);
        let legend = semantic_tokens_legend();
        let class_type = legend
            .token_types
            .iter()
            .position(|token_type| *token_type == SemanticTokenType::CLASS)
            .expect("class legend entry should exist") as u32;
        let enum_type = legend
            .token_types
            .iter()
            .position(|token_type| *token_type == SemanticTokenType::ENUM)
            .expect("enum legend entry should exist") as u32;

        for (needle, occurrence, token_type) in [
            ("Cfg", 1usize, class_type),
            ("Cfg", 2usize, class_type),
            ("Cfg", 3usize, class_type),
            ("Cmd", 1usize, enum_type),
            ("Cmd", 2usize, enum_type),
        ] {
            let span = Span::new(
                nth_offset(&source, needle, occurrence),
                nth_offset(&source, needle, occurrence) + needle.len(),
            );
            let range = span_to_range(&source, span);
            assert!(decoded.contains(&(
                range.start.line,
                range.start.character,
                range.end.character - range.start.character,
                token_type,
            )));
        }
    }

    #[test]
    fn workspace_import_semantic_tokens_survive_parse_errors_with_open_workspace_source() {
        let temp = TempDir::new("ql-lsp-workspace-import-semantic-tokens-parse-errors-open-docs");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.Command as Cmd
use demo.core.Config as Cfg

pub fn main(config: Cfg) -> Int {
    let built = Cfg { value: 1, limit: 2
    let command = Cmd.Retry(1)
    return built.value
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn helper() -> Int {
    return 0
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&source).is_err());
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let core_uri =
            Url::from_file_path(&core_source_path).expect("core source path should convert to URI");
        let open_core_source = r#"
package demo.core

pub enum Command {
    Retry(Int),
}

pub struct Config {
    value: Int,
    limit: Int,
}
"#
        .to_owned();

        let SemanticTokensResult::Tokens(disk_tokens) =
            semantic_tokens_for_workspace_dependency_fallback(&uri, &source, &package)
        else {
            panic!("expected full semantic tokens")
        };
        let disk_decoded = decode_semantic_tokens(&disk_tokens.data);

        let SemanticTokensResult::Tokens(tokens) =
            semantic_tokens_for_workspace_dependency_fallback_with_open_docs(
                &uri,
                &source,
                &package,
                &file_open_documents(vec![
                    (uri.clone(), source.clone()),
                    (core_uri, open_core_source),
                ]),
            )
        else {
            panic!("expected full semantic tokens")
        };
        let decoded = decode_semantic_tokens(&tokens.data);
        let legend = semantic_tokens_legend();
        let class_type = legend
            .token_types
            .iter()
            .position(|token_type| *token_type == SemanticTokenType::CLASS)
            .expect("class legend entry should exist") as u32;
        let enum_type = legend
            .token_types
            .iter()
            .position(|token_type| *token_type == SemanticTokenType::ENUM)
            .expect("enum legend entry should exist") as u32;

        for (needle, occurrence, token_type) in [
            ("Cfg", 1usize, class_type),
            ("Cfg", 2usize, class_type),
            ("Cfg", 3usize, class_type),
            ("Cmd", 1usize, enum_type),
            ("Cmd", 2usize, enum_type),
        ] {
            let span = Span::new(
                nth_offset(&source, needle, occurrence),
                nth_offset(&source, needle, occurrence) + needle.len(),
            );
            let range = span_to_range(&source, span);
            assert!(!disk_decoded.contains(&(
                range.start.line,
                range.start.character,
                range.end.character - range.start.character,
                token_type,
            )));
            assert!(decoded.contains(&(
                range.start.line,
                range.start.character,
                range.end.character - range.start.character,
                token_type,
            )));
        }
    }

    #[test]
    fn workspace_type_import_type_definition_prefers_workspace_member_source_over_interface_artifact()
     {
        let temp = TempDir::new("ql-lsp-workspace-type-import-source-type-definition");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.Config

pub fn main(value: Config) -> Config {
    return value
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub struct Config {
    value: Int,
    extra: Int,
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub struct Config {
    value: Int,
}
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        let analysis = analyze_source(&source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");

        let definition = workspace_source_type_definition_for_import(
            &uri,
            &source,
            &analysis,
            &package,
            offset_to_position(&source, nth_offset(&source, "Config", 2)),
        )
        .expect("workspace import type definition should exist");

        let GotoTypeDefinitionResponse::Scalar(location) = definition else {
            panic!("workspace import type definition should resolve to one location")
        };
        assert_eq!(
            location
                .uri
                .to_file_path()
                .expect("definition URI should convert to a file path")
                .canonicalize()
                .expect("definition path should canonicalize"),
            core_source_path
                .canonicalize()
                .expect("core source path should canonicalize"),
        );
    }

    #[test]
    fn workspace_type_import_type_definition_prefers_open_workspace_source() {
        let temp = TempDir::new("ql-lsp-workspace-type-import-source-type-definition-open-docs");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.Config

pub fn main(value: Config) -> Config {
    return value
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn helper() -> Int {
    return 0
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub struct Config {
    value: Int,
}
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        let analysis = analyze_source(&source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let core_uri =
            Url::from_file_path(&core_source_path).expect("core path should convert to URI");
        let open_core_source = r#"
package demo.core

pub fn helper() -> Int {
    return 0
}

pub struct Config {
    value: Int,
    extra: Int,
}
"#
        .to_owned();

        assert_eq!(
            workspace_source_type_definition_for_import(
                &uri,
                &source,
                &analysis,
                &package,
                offset_to_position(&source, nth_offset(&source, "Config", 2)),
            ),
            None,
            "disk-only type definition should miss unsaved workspace source",
        );

        let definition = workspace_source_type_definition_for_import_with_open_docs(
            &uri,
            &source,
            &analysis,
            &package,
            &file_open_documents(vec![
                (uri.clone(), source.clone()),
                (core_uri, open_core_source),
            ]),
            offset_to_position(&source, nth_offset(&source, "Config", 2)),
        )
        .expect("workspace import type definition should use open workspace source");

        let GotoTypeDefinitionResponse::Scalar(location) = definition else {
            panic!("workspace import type definition should resolve to one location")
        };
        assert_eq!(
            location
                .uri
                .to_file_path()
                .expect("definition URI should convert to a file path")
                .canonicalize()
                .expect("definition path should canonicalize"),
            core_source_path
                .canonicalize()
                .expect("core source path should canonicalize"),
        );
    }

    #[test]
    fn workspace_import_definition_survives_parse_errors_and_prefers_workspace_member_source() {
        let temp = TempDir::new("ql-lsp-workspace-import-source-definition-parse-errors");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.exported as run

pub fn main() -> Int {
    let next = run(1)
    return next
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn exported(value: Int) -> Int {
    return value
}

pub fn wrapper(value: Int) -> Int {
    return exported(value)
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn exported(value: Int) -> Int
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&source).is_err());
        let package = package_analysis_for_path(&app_path)
            .expect("package analysis should survive parse errors");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");

        let definition = workspace_source_definition_for_import_in_broken_source(
            &uri,
            &source,
            &package,
            offset_to_position(&source, nth_offset(&source, "run", 2)),
        )
        .expect("broken-source workspace import definition should exist");

        let GotoDefinitionResponse::Scalar(location) = definition else {
            panic!("workspace import definition should resolve to one location")
        };
        assert_eq!(
            location
                .uri
                .to_file_path()
                .expect("definition URI should convert to a file path")
                .canonicalize()
                .expect("definition path should canonicalize"),
            core_source_path
                .canonicalize()
                .expect("core source path should canonicalize"),
        );
    }

    #[test]
    fn workspace_import_definition_in_broken_source_prefers_open_workspace_member_source() {
        let temp = TempDir::new("ql-lsp-workspace-import-source-definition-open-docs");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.exported as run

pub fn main() -> Int {
    let next = run(1)
    return next
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn exported(value: Int) -> Int {
    return value
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn exported(value: Int) -> Int
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&source).is_err());
        let package = package_analysis_for_path(&app_path)
            .expect("package analysis should survive parse errors");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let core_uri =
            Url::from_file_path(&core_source_path).expect("core source path should convert to URI");
        let disk_core_source =
            fs::read_to_string(&core_source_path).expect("core source should read from disk");
        let open_core_source = r#"
package demo.core

pub fn helper() -> Int {
    return 0
}

pub fn exported(value: Int) -> Int {
    return value
}
"#
        .to_owned();

        let definition = workspace_source_definition_for_import_in_broken_source_with_open_docs(
            &uri,
            &source,
            &package,
            &file_open_documents(vec![(core_uri.clone(), open_core_source.clone())]),
            offset_to_position(&source, nth_offset(&source, "run", 2)),
        )
        .expect("broken-source workspace import definition should exist");

        let GotoDefinitionResponse::Scalar(location) = definition else {
            panic!("workspace import definition should resolve to one location")
        };
        assert_eq!(location.uri, core_uri);
        assert_eq!(
            location.range.start,
            offset_to_position(
                &open_core_source,
                nth_offset(&open_core_source, "exported", 1)
            ),
        );
        assert_ne!(
            location.range.start,
            offset_to_position(
                &disk_core_source,
                nth_offset(&disk_core_source, "exported", 1)
            ),
        );
    }

    #[test]
    fn workspace_import_definition_in_broken_source_recognizes_trait_impl_headers() {
        let temp = TempDir::new("ql-lsp-workspace-import-definition-broken-trait-impl-header");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.Runner

struct AppWorker {}

impl Runner for AppWorker {
    fn run(self) -> Int {
        return 1
    }
}

pub fn broken() -> Int {
    return AppWorker {
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub trait Runner {
    fn run(self) -> Int
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub trait Runner {
    fn run(self) -> Int
}
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&source).is_err());
        let package = package_analysis_for_path(&app_path)
            .expect("package analysis should survive parse errors");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");

        let definition = workspace_source_definition_for_import_in_broken_source(
            &uri,
            &source,
            &package,
            offset_to_position(&source, nth_offset(&source, "Runner", 2)),
        )
        .expect("broken-source workspace trait import definition should exist");

        let GotoDefinitionResponse::Scalar(location) = definition else {
            panic!("workspace import definition should resolve to one location")
        };
        assert_eq!(
            location
                .uri
                .to_file_path()
                .expect("definition URI should convert to a file path")
                .canonicalize()
                .expect("definition path should canonicalize"),
            core_source_path
                .canonicalize()
                .expect("core source path should canonicalize"),
        );
    }

    #[test]
    fn workspace_type_import_type_definition_survives_parse_errors_and_prefers_workspace_member_source()
     {
        let temp = TempDir::new("ql-lsp-workspace-type-import-source-type-definition-parse-errors");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.Config

pub fn main(value: Config) -> Config {
    return value
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub struct Config {
    value: Int,
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub struct Config {
    value: Int,
}
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&source).is_err());
        let package = package_analysis_for_path(&app_path)
            .expect("package analysis should survive parse errors");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");

        let definition = workspace_source_type_definition_for_import_in_broken_source(
            &uri,
            &source,
            &package,
            offset_to_position(&source, nth_offset(&source, "Config", 2)),
        )
        .expect("broken-source workspace import type definition should exist");

        let GotoTypeDefinitionResponse::Scalar(location) = definition else {
            panic!("workspace import type definition should resolve to one location")
        };
        assert_eq!(
            location
                .uri
                .to_file_path()
                .expect("definition URI should convert to a file path")
                .canonicalize()
                .expect("definition path should canonicalize"),
            core_source_path
                .canonicalize()
                .expect("core source path should canonicalize"),
        );
    }

    #[test]
    fn workspace_type_import_definition_survives_parse_errors_and_keeps_type_context() {
        let temp = TempDir::new("ql-lsp-workspace-type-import-source-definition-parse-errors");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.Config

pub fn main(value: Config) -> Config {
    return Config { value
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub struct Config {
    value: Int,
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub struct Config {
    value: Int,
}
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&source).is_err());
        let package = package_analysis_for_path(&app_path)
            .expect("package analysis should survive parse errors");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");

        let definition = workspace_source_definition_for_import_in_broken_source(
            &uri,
            &source,
            &package,
            offset_to_position(&source, nth_offset(&source, "Config", 2)),
        )
        .expect("broken-source workspace type import definition should exist");

        let GotoDefinitionResponse::Scalar(location) = definition else {
            panic!("workspace import definition should resolve to one location")
        };
        assert_eq!(
            location
                .uri
                .to_file_path()
                .expect("definition URI should convert to a file path")
                .canonicalize()
                .expect("definition path should canonicalize"),
            core_source_path
                .canonicalize()
                .expect("core source path should canonicalize"),
        );
    }

    #[test]
    fn workspace_import_hover_survives_parse_errors_and_prefers_workspace_member_source() {
        let temp = TempDir::new("ql-lsp-workspace-import-source-hover-parse-errors");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.exported as run

pub fn main() -> Int {
    let next = run(1)
    return next
"#,
        );
        temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn exported(value: Int, extra: Int) -> Int {
    return value + extra
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn exported(value: Int) -> Int
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&source).is_err());
        let package = package_analysis_for_path(&app_path)
            .expect("package analysis should survive parse errors");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");

        let hover = workspace_source_hover_for_import_in_broken_source(
            &uri,
            &source,
            &package,
            offset_to_position(&source, nth_offset(&source, "run", 2)),
        )
        .expect("broken-source workspace import hover should exist");
        let HoverContents::Markup(markup) = hover.contents else {
            panic!("hover should use markdown")
        };
        assert!(
            markup
                .value
                .contains("fn exported(value: Int, extra: Int) -> Int")
        );
        assert!(!markup.value.contains("fn exported(value: Int) -> Int"));
    }

    #[test]
    fn workspace_import_hover_in_broken_source_prefers_open_workspace_member_source() {
        let temp = TempDir::new("ql-lsp-workspace-import-source-hover-open-docs");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.exported as run

pub fn main() -> Int {
    let next = run(1)
    return next
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn exported(value: Int) -> Int {
    return value
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn exported(value: Int) -> Int
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&source).is_err());
        let package = package_analysis_for_path(&app_path)
            .expect("package analysis should survive parse errors");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let core_uri =
            Url::from_file_path(&core_source_path).expect("core source path should convert to URI");
        let open_core_source = r#"
package demo.core

pub fn helper() -> Int {
    return 0
}

pub fn exported(value: Int, extra: Int) -> Int {
    return value + extra
}
"#
        .to_owned();

        let hover = workspace_source_hover_for_import_in_broken_source_with_open_docs(
            &uri,
            &source,
            &package,
            &file_open_documents(vec![(core_uri, open_core_source)]),
            offset_to_position(&source, nth_offset(&source, "run", 2)),
        )
        .expect("broken-source workspace import hover should exist");
        let HoverContents::Markup(markup) = hover.contents else {
            panic!("hover should use markdown")
        };
        assert!(
            markup
                .value
                .contains("fn exported(value: Int, extra: Int) -> Int")
        );
        assert!(!markup.value.contains("fn exported(value: Int) -> Int"));
    }

    #[test]
    fn workspace_import_references_prefer_workspace_member_source_over_interface_artifact() {
        let temp = TempDir::new("ql-lsp-workspace-import-source-references");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.exported as run

pub fn main() -> Int {
    return run(1)
}
"#,
        );
        let task_path = temp.write(
            "workspace/packages/app/src/task.ql",
            r#"
package demo.app

use demo.core.exported as call

pub fn task() -> Int {
    return call(2)
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn exported(value: Int) -> Int {
    return value
}

pub fn wrapper(value: Int) -> Int {
    return exported(value)
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn exported(value: Int) -> Int
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        let analysis = analyze_source(&source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let core_source =
            fs::read_to_string(&core_source_path).expect("core source should read for assertions");
        let task_source = fs::read_to_string(&task_path).expect("task source should read");
        let task_uri = Url::from_file_path(&task_path).expect("task path should convert to URI");

        let references = workspace_source_references_for_import(
            &uri,
            &source,
            &analysis,
            &package,
            offset_to_position(&source, nth_offset(&source, "run", 2)),
            true,
        )
        .expect("workspace import references should exist");
        assert_eq!(references.len(), 6);
        assert_eq!(
            references[0]
                .uri
                .to_file_path()
                .expect("definition URI should convert to a file path")
                .canonicalize()
                .expect("definition path should canonicalize"),
            core_source_path
                .canonicalize()
                .expect("core source path should canonicalize"),
        );
        assert_eq!(
            references[0].range.start,
            offset_to_position(&core_source, nth_offset(&core_source, "exported", 1)),
        );
        assert_eq!(references[1].uri, uri);
        assert_eq!(
            references[1].range.start,
            offset_to_position(&source, nth_offset(&source, "run", 1)),
        );
        assert_eq!(references[2].uri, uri);
        assert_eq!(
            references[2].range.start,
            offset_to_position(&source, nth_offset(&source, "run", 2)),
        );
        assert_eq!(
            references[3]
                .uri
                .to_file_path()
                .expect("source reference URI should convert to a file path")
                .canonicalize()
                .expect("source reference path should canonicalize"),
            core_source_path
                .canonicalize()
                .expect("core source path should canonicalize"),
        );
        assert_eq!(
            references[3].range.start,
            offset_to_position(&core_source, nth_offset(&core_source, "exported", 2)),
        );
        assert_eq!(references[4].uri, task_uri);
        assert_eq!(
            references[4].range.start,
            offset_to_position(&task_source, nth_offset(&task_source, "call", 1)),
        );
        assert_eq!(references[5].uri, task_uri);
        assert_eq!(
            references[5].range.start,
            offset_to_position(&task_source, nth_offset(&task_source, "call", 2)),
        );
    }

    #[test]
    fn local_dependency_import_references_prefer_dependency_source_over_interface_artifact() {
        let temp = TempDir::new("ql-lsp-local-dependency-import-source-references");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.exported as run

pub fn main() -> Int {
    return run(1)
}
"#,
        );
        let task_path = temp.write(
            "workspace/packages/app/src/task.ql",
            r#"
package demo.app

use demo.core.exported as call

pub fn task() -> Int {
    return call(2)
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/vendor/core/src/lib.ql",
            r#"
package demo.core

pub fn exported(value: Int) -> Int {
    return value
}

pub fn wrapper(value: Int) -> Int {
    return exported(value)
}
"#,
        );
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
core = { path = "../../vendor/core" }
"#,
        );
        temp.write(
            "workspace/vendor/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/vendor/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn exported(value: Int) -> Int
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        let analysis = analyze_source(&source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let core_source =
            fs::read_to_string(&core_source_path).expect("core source should read for assertions");
        let task_source = fs::read_to_string(&task_path).expect("task source should read");
        let task_uri = Url::from_file_path(&task_path).expect("task path should convert to URI");

        let references = workspace_source_references_for_import(
            &uri,
            &source,
            &analysis,
            &package,
            offset_to_position(&source, nth_offset(&source, "run", 2)),
            true,
        )
        .expect("local dependency import references should exist");
        assert_eq!(references.len(), 6);
        assert_eq!(
            references[0]
                .uri
                .to_file_path()
                .expect("definition URI should convert to a file path")
                .canonicalize()
                .expect("definition path should canonicalize"),
            core_source_path
                .canonicalize()
                .expect("core source path should canonicalize"),
        );
        assert_eq!(
            references[0].range.start,
            offset_to_position(&core_source, nth_offset(&core_source, "exported", 1)),
        );
        assert_eq!(references[1].uri, uri);
        assert_eq!(
            references[1].range.start,
            offset_to_position(&source, nth_offset(&source, "run", 1)),
        );
        assert_eq!(references[2].uri, uri);
        assert_eq!(
            references[2].range.start,
            offset_to_position(&source, nth_offset(&source, "run", 2)),
        );
        assert_eq!(
            references[3]
                .uri
                .to_file_path()
                .expect("source reference URI should convert to a file path")
                .canonicalize()
                .expect("source reference path should canonicalize"),
            core_source_path
                .canonicalize()
                .expect("core source path should canonicalize"),
        );
        assert_eq!(
            references[3].range.start,
            offset_to_position(&core_source, nth_offset(&core_source, "exported", 2)),
        );
        assert_eq!(references[4].uri, task_uri);
        assert_eq!(
            references[4].range.start,
            offset_to_position(&task_source, nth_offset(&task_source, "call", 1)),
        );
        assert_eq!(references[5].uri, task_uri);
        assert_eq!(
            references[5].range.start,
            offset_to_position(&task_source, nth_offset(&task_source, "call", 2)),
        );
    }

    #[test]
    fn workspace_root_function_definition_references_include_workspace_imports() {
        let temp = TempDir::new("ql-lsp-workspace-root-function-definition-import-references");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.exported as run

pub fn main() -> Int {
    return run(1)
}
"#,
        );
        let task_path = temp.write(
            "workspace/packages/app/src/task.ql",
            r#"
package demo.app

use demo.core.exported as call

pub fn task() -> Int {
    return call(2)
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn exported(value: Int) -> Int {
    return value
}

pub fn wrapper(value: Int) -> Int {
    return exported(value)
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn exported(value: Int) -> Int
"#,
        );

        let core_source = fs::read_to_string(&core_source_path).expect("core source should read");
        let core_analysis = analyze_source(&core_source).expect("core source should analyze");
        let package =
            package_analysis_for_path(&core_source_path).expect("package analysis should succeed");
        let core_uri =
            Url::from_file_path(&core_source_path).expect("core source path should convert to URI");
        let app_source = fs::read_to_string(&app_path).expect("app source should read");
        let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let task_source = fs::read_to_string(&task_path).expect("task source should read");
        let task_uri = Url::from_file_path(&task_path).expect("task path should convert to URI");

        let references = workspace_source_references_for_root_symbol_with_open_docs(
            &core_uri,
            &core_source,
            &core_analysis,
            &package,
            &file_open_documents(vec![
                (core_uri.clone(), core_source.clone()),
                (app_uri.clone(), app_source.clone()),
                (task_uri.clone(), task_source.clone()),
            ]),
            offset_to_position(&core_source, nth_offset(&core_source, "exported", 1)),
            true,
        )
        .expect("workspace root definition references should exist");

        assert_eq!(references.len(), 6);
        assert_eq!(references[0].uri, core_uri);
        assert_eq!(
            references[0].range.start,
            offset_to_position(&core_source, nth_offset(&core_source, "exported", 1)),
        );
        assert_eq!(references[1].uri, core_uri);
        assert_eq!(
            references[1].range.start,
            offset_to_position(&core_source, nth_offset(&core_source, "exported", 2)),
        );
        assert_eq!(references[2].uri, app_uri);
        assert_eq!(
            references[2].range.start,
            offset_to_position(&app_source, nth_offset(&app_source, "run", 1)),
        );
        assert_eq!(references[3].uri, app_uri);
        assert_eq!(
            references[3].range.start,
            offset_to_position(&app_source, nth_offset(&app_source, "run", 2)),
        );
        assert_eq!(references[4].uri, task_uri);
        assert_eq!(
            references[4].range.start,
            offset_to_position(&task_source, nth_offset(&task_source, "call", 1)),
        );
        assert_eq!(references[5].uri, task_uri);
        assert_eq!(
            references[5].range.start,
            offset_to_position(&task_source, nth_offset(&task_source, "call", 2)),
        );
    }

    #[test]
    fn workspace_root_function_usage_references_include_workspace_imports() {
        let temp = TempDir::new("ql-lsp-workspace-root-function-usage-import-references");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.exported as run

pub fn main() -> Int {
    return run(1)
}
"#,
        );
        let task_path = temp.write(
            "workspace/packages/app/src/task.ql",
            r#"
package demo.app

use demo.core.exported as call

pub fn task() -> Int {
    return call(2)
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn exported(value: Int) -> Int {
    return value
}

pub fn wrapper(value: Int) -> Int {
    return exported(value)
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn exported(value: Int) -> Int
"#,
        );

        let core_source = fs::read_to_string(&core_source_path).expect("core source should read");
        let core_analysis = analyze_source(&core_source).expect("core source should analyze");
        let package =
            package_analysis_for_path(&core_source_path).expect("package analysis should succeed");
        let core_uri =
            Url::from_file_path(&core_source_path).expect("core source path should convert to URI");
        let app_source = fs::read_to_string(&app_path).expect("app source should read");
        let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let task_source = fs::read_to_string(&task_path).expect("task source should read");
        let task_uri = Url::from_file_path(&task_path).expect("task path should convert to URI");

        let references = workspace_source_references_for_root_symbol_with_open_docs(
            &core_uri,
            &core_source,
            &core_analysis,
            &package,
            &file_open_documents(vec![
                (core_uri.clone(), core_source.clone()),
                (app_uri.clone(), app_source.clone()),
                (task_uri.clone(), task_source.clone()),
            ]),
            offset_to_position(&core_source, nth_offset(&core_source, "exported", 2)),
            true,
        )
        .expect("workspace root usage references should exist");

        assert_eq!(references.len(), 6);
        assert_eq!(references[0].uri, core_uri);
        assert_eq!(
            references[0].range.start,
            offset_to_position(&core_source, nth_offset(&core_source, "exported", 1)),
        );
        assert_eq!(references[1].uri, core_uri);
        assert_eq!(
            references[1].range.start,
            offset_to_position(&core_source, nth_offset(&core_source, "exported", 2)),
        );
        assert_eq!(references[2].uri, app_uri);
        assert_eq!(
            references[2].range.start,
            offset_to_position(&app_source, nth_offset(&app_source, "run", 1)),
        );
        assert_eq!(references[3].uri, app_uri);
        assert_eq!(
            references[3].range.start,
            offset_to_position(&app_source, nth_offset(&app_source, "run", 2)),
        );
        assert_eq!(references[4].uri, task_uri);
        assert_eq!(
            references[4].range.start,
            offset_to_position(&task_source, nth_offset(&task_source, "call", 1)),
        );
        assert_eq!(references[5].uri, task_uri);
        assert_eq!(
            references[5].range.start,
            offset_to_position(&task_source, nth_offset(&task_source, "call", 2)),
        );
    }

    #[test]
    fn workspace_root_references_use_open_workspace_import_consumers() {
        let temp = TempDir::new("ql-lsp-workspace-root-import-references-open-consumers");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.exported as run

pub fn main() -> Int {
    return run(1)
}
"#,
        );
        let task_path = temp.write(
            "workspace/packages/app/src/task.ql",
            r#"
package demo.app

use demo.core.exported as call

pub fn task() -> Int {
    return call(2)
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn exported(value: Int) -> Int {
    return value
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn exported(value: Int) -> Int
"#,
        );

        let app_source = fs::read_to_string(&app_path).expect("app source should read");
        let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let task_uri = Url::from_file_path(&task_path).expect("task path should convert to URI");
        let core_uri =
            Url::from_file_path(&core_source_path).expect("core source path should convert to URI");
        let disk_task_source = fs::read_to_string(&task_path).expect("task source should read");
        let open_task_source = r#"
package demo.app


use demo.core.exported as ship

pub fn task() -> Int {
    let current = ship(2)
    return ship(current)
}
"#
        .to_owned();
        let open_core_source = r#"
package demo.core

pub fn helper() -> Int {
    return 0
}

pub fn exported(value: Int) -> Int {
    return value
}

pub fn wrapper(value: Int) -> Int {
    return exported(value)
}
"#
        .to_owned();
        let open_core_analysis =
            analyze_source(&open_core_source).expect("open core source should analyze");
        let package =
            package_analysis_for_path(&core_source_path).expect("package analysis should succeed");

        let references = workspace_source_references_for_root_symbol_with_open_docs(
            &core_uri,
            &open_core_source,
            &open_core_analysis,
            &package,
            &file_open_documents(vec![
                (core_uri.clone(), open_core_source.clone()),
                (app_uri.clone(), app_source.clone()),
                (task_uri.clone(), open_task_source.clone()),
            ]),
            offset_to_position(
                &open_core_source,
                nth_offset(&open_core_source, "exported", 1),
            ),
            true,
        )
        .expect("workspace root references should use open import consumers");

        let contains = |uri: &Url, source: &str, needle: &str, occurrence: usize| {
            references.iter().any(|location| {
                location.uri == *uri
                    && location.range.start
                        == offset_to_position(source, nth_offset(source, needle, occurrence))
            })
        };

        assert_eq!(references.len(), 7);
        assert!(contains(&core_uri, &open_core_source, "exported", 1));
        assert!(contains(&core_uri, &open_core_source, "exported", 2));
        assert!(contains(&app_uri, &app_source, "run", 1));
        assert!(contains(&app_uri, &app_source, "run", 2));
        assert!(contains(&task_uri, &open_task_source, "ship", 1));
        assert!(contains(&task_uri, &open_task_source, "ship", 2));
        assert!(contains(&task_uri, &open_task_source, "ship", 3));
        assert!(
            !references.iter().any(|location| {
                location.uri == task_uri
                    && location.range.start
                        == offset_to_position(
                            &disk_task_source,
                            nth_offset(&disk_task_source, "call", 1),
                        )
            }),
            "references should not keep stale disk task import aliases",
        );
    }

    #[test]
    fn workspace_root_function_definition_references_include_broken_consumers() {
        let temp =
            TempDir::new("ql-lsp-workspace-root-function-definition-import-references-broken");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.exported as run

pub fn main() -> Int {
    return run(1)
"#,
        );
        let jobs_path = temp.write(
            "workspace/packages/jobs/src/job.ql",
            r#"
package demo.jobs

use demo.core.exported as exec

pub fn job() -> Int {
    return exec(2)
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn exported(value: Int) -> Int {
    return value
}

pub fn wrapper(value: Int) -> Int {
    return exported(value)
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/jobs", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/jobs/qlang.toml",
            r#"
[package]
name = "jobs"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn exported(value: Int) -> Int
"#,
        );

        let core_source = fs::read_to_string(&core_source_path).expect("core source should read");
        let core_analysis = analyze_source(&core_source).expect("core source should analyze");
        let package =
            package_analysis_for_path(&core_source_path).expect("package analysis should succeed");
        let core_uri =
            Url::from_file_path(&core_source_path).expect("core source path should convert to URI");
        let app_source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&app_source).is_err());
        let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let jobs_source = fs::read_to_string(&jobs_path).expect("jobs source should read");
        assert!(analyze_source(&jobs_source).is_err());
        let jobs_uri = Url::from_file_path(&jobs_path).expect("jobs path should convert to URI");

        let references = workspace_source_references_for_root_symbol_with_open_docs(
            &core_uri,
            &core_source,
            &core_analysis,
            &package,
            &file_open_documents(vec![
                (core_uri.clone(), core_source.clone()),
                (app_uri.clone(), app_source.clone()),
                (jobs_uri.clone(), jobs_source.clone()),
            ]),
            offset_to_position(&core_source, nth_offset(&core_source, "exported", 1)),
            true,
        )
        .expect("workspace root definition references should exist");

        let contains = |uri: &Url, source: &str, needle: &str, occurrence: usize| {
            references.iter().any(|location| {
                location.uri == *uri
                    && location.range.start
                        == offset_to_position(source, nth_offset(source, needle, occurrence))
            })
        };

        assert_eq!(references.len(), 6);
        assert!(contains(&core_uri, &core_source, "exported", 1));
        assert!(contains(&core_uri, &core_source, "exported", 2));
        assert!(contains(&app_uri, &app_source, "run", 1));
        assert!(contains(&app_uri, &app_source, "run", 2));
        assert!(contains(&jobs_uri, &jobs_source, "exec", 1));
        assert!(contains(&jobs_uri, &jobs_source, "exec", 2));
    }

    #[test]
    fn workspace_root_function_usage_references_include_broken_consumers() {
        let temp = TempDir::new("ql-lsp-workspace-root-function-usage-import-references-broken");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.exported as run

pub fn main() -> Int {
    return run(1)
"#,
        );
        let jobs_path = temp.write(
            "workspace/packages/jobs/src/job.ql",
            r#"
package demo.jobs

use demo.core.exported as exec

pub fn job() -> Int {
    return exec(2)
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn exported(value: Int) -> Int {
    return value
}

pub fn wrapper(value: Int) -> Int {
    return exported(value)
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/jobs", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/jobs/qlang.toml",
            r#"
[package]
name = "jobs"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn exported(value: Int) -> Int
"#,
        );

        let core_source = fs::read_to_string(&core_source_path).expect("core source should read");
        let core_analysis = analyze_source(&core_source).expect("core source should analyze");
        let package =
            package_analysis_for_path(&core_source_path).expect("package analysis should succeed");
        let core_uri =
            Url::from_file_path(&core_source_path).expect("core source path should convert to URI");
        let app_source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&app_source).is_err());
        let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let jobs_source = fs::read_to_string(&jobs_path).expect("jobs source should read");
        assert!(analyze_source(&jobs_source).is_err());
        let jobs_uri = Url::from_file_path(&jobs_path).expect("jobs path should convert to URI");

        let references = workspace_source_references_for_root_symbol_with_open_docs(
            &core_uri,
            &core_source,
            &core_analysis,
            &package,
            &file_open_documents(vec![
                (core_uri.clone(), core_source.clone()),
                (app_uri.clone(), app_source.clone()),
                (jobs_uri.clone(), jobs_source.clone()),
            ]),
            offset_to_position(&core_source, nth_offset(&core_source, "exported", 2)),
            true,
        )
        .expect("workspace root usage references should exist");

        let contains = |uri: &Url, source: &str, needle: &str, occurrence: usize| {
            references.iter().any(|location| {
                location.uri == *uri
                    && location.range.start
                        == offset_to_position(source, nth_offset(source, needle, occurrence))
            })
        };

        assert_eq!(references.len(), 6);
        assert!(contains(&core_uri, &core_source, "exported", 1));
        assert!(contains(&core_uri, &core_source, "exported", 2));
        assert!(contains(&app_uri, &app_source, "run", 1));
        assert!(contains(&app_uri, &app_source, "run", 2));
        assert!(contains(&jobs_uri, &jobs_source, "exec", 1));
        assert!(contains(&jobs_uri, &jobs_source, "exec", 2));
    }

    #[test]
    fn workspace_type_import_references_include_other_workspace_uses() {
        let temp = TempDir::new("ql-lsp-workspace-type-import-references");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.Config as Cfg

pub fn main(config: Cfg) -> Int {
    return config.value
}
"#,
        );
        let task_path = temp.write(
            "workspace/packages/app/src/task.ql",
            r#"
package demo.app

use demo.core.Config as OtherCfg

pub fn task(config: OtherCfg) -> Int {
    return config.value
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub struct Config {
    value: Int,
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub struct Config {
    value: Int,
}
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        let analysis = analyze_source(&source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let core_source =
            fs::read_to_string(&core_source_path).expect("core source should read for assertions");
        let task_source = fs::read_to_string(&task_path).expect("task source should read");
        let task_uri = Url::from_file_path(&task_path).expect("task path should convert to URI");

        let references = workspace_source_references_for_import(
            &uri,
            &source,
            &analysis,
            &package,
            offset_to_position(&source, nth_offset(&source, "Cfg", 2)),
            true,
        )
        .expect("workspace type import references should exist");

        assert_eq!(references.len(), 5);
        assert_eq!(
            references[0]
                .uri
                .to_file_path()
                .expect("definition URI should convert to a file path")
                .canonicalize()
                .expect("definition path should canonicalize"),
            core_source_path
                .canonicalize()
                .expect("core source path should canonicalize"),
        );
        assert_eq!(
            references[0].range.start,
            offset_to_position(&core_source, nth_offset(&core_source, "Config", 1)),
        );
        assert_eq!(references[1].uri, uri);
        assert_eq!(
            references[1].range.start,
            offset_to_position(&source, nth_offset(&source, "Cfg", 1)),
        );
        assert_eq!(references[2].uri, uri);
        assert_eq!(
            references[2].range.start,
            offset_to_position(&source, nth_offset(&source, "Cfg", 2)),
        );
        assert_eq!(references[3].uri, task_uri);
        assert_eq!(
            references[3].range.start,
            offset_to_position(&task_source, nth_offset(&task_source, "OtherCfg", 1)),
        );
        assert_eq!(references[4].uri, task_uri);
        assert_eq!(
            references[4].range.start,
            offset_to_position(&task_source, nth_offset(&task_source, "OtherCfg", 2)),
        );
    }

    #[test]
    fn workspace_root_struct_usage_references_include_workspace_type_imports() {
        let temp = TempDir::new("ql-lsp-workspace-root-struct-usage-references");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.Config as Cfg

pub fn main(config: Cfg) -> Int {
    return config.value
}
"#,
        );
        let task_path = temp.write(
            "workspace/packages/app/src/task.ql",
            r#"
package demo.app

use demo.core.Config as OtherCfg

pub fn task(config: OtherCfg) -> Int {
    return config.value
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub struct Config {
    value: Int,
}

pub fn copy(config: Config) -> Config {
    return config
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub struct Config {
    value: Int,
}
"#,
        );

        let core_source = fs::read_to_string(&core_source_path).expect("core source should read");
        let core_analysis = analyze_source(&core_source).expect("core source should analyze");
        let package =
            package_analysis_for_path(&core_source_path).expect("package analysis should succeed");
        let core_uri =
            Url::from_file_path(&core_source_path).expect("core source path should convert to URI");
        let app_source = fs::read_to_string(&app_path).expect("app source should read");
        let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let task_source = fs::read_to_string(&task_path).expect("task source should read");
        let task_uri = Url::from_file_path(&task_path).expect("task path should convert to URI");

        let references = workspace_source_references_for_root_symbol_with_open_docs(
            &core_uri,
            &core_source,
            &core_analysis,
            &package,
            &file_open_documents(vec![
                (core_uri.clone(), core_source.clone()),
                (app_uri.clone(), app_source.clone()),
                (task_uri.clone(), task_source.clone()),
            ]),
            offset_to_position(&core_source, nth_offset(&core_source, "Config", 2)),
            true,
        )
        .expect("workspace root struct usage references should exist");

        assert_eq!(references.len(), 7);
        assert_eq!(references[0].uri, core_uri);
        assert_eq!(
            references[0].range.start,
            offset_to_position(&core_source, nth_offset(&core_source, "Config", 1)),
        );
        assert_eq!(references[1].uri, core_uri);
        assert_eq!(
            references[1].range.start,
            offset_to_position(&core_source, nth_offset(&core_source, "Config", 2)),
        );
        assert_eq!(references[2].uri, core_uri);
        assert_eq!(
            references[2].range.start,
            offset_to_position(&core_source, nth_offset(&core_source, "Config", 3)),
        );
        assert_eq!(references[3].uri, app_uri);
        assert_eq!(
            references[3].range.start,
            offset_to_position(&app_source, nth_offset(&app_source, "Cfg", 1)),
        );
        assert_eq!(references[4].uri, app_uri);
        assert_eq!(
            references[4].range.start,
            offset_to_position(&app_source, nth_offset(&app_source, "Cfg", 2)),
        );
        assert_eq!(references[5].uri, task_uri);
        assert_eq!(
            references[5].range.start,
            offset_to_position(&task_source, nth_offset(&task_source, "OtherCfg", 1)),
        );
        assert_eq!(references[6].uri, task_uri);
        assert_eq!(
            references[6].range.start,
            offset_to_position(&task_source, nth_offset(&task_source, "OtherCfg", 2)),
        );
    }

    #[test]
    fn workspace_root_member_references_include_visible_workspace_consumers() {
        let temp = TempDir::new("ql-lsp-workspace-root-member-references");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.Command as Cmd
use demo.core.Config as Cfg

pub fn main(config: Cfg) -> Int {
    let command = Cmd.Retry(1)
    match command {
        Cmd.Retry(count) => count + config.get() + config.value,
        Cmd.Stop => 0,
    }
}
"#,
        );
        let task_path = temp.write(
            "workspace/packages/app/src/task.ql",
            r#"
package demo.app

use demo.core.Command as Cmd
use demo.core.Config as OtherCfg

pub fn task(config: OtherCfg) -> Int {
    let command = Cmd.Retry(2)
    match command {
        Cmd.Retry(count) => count + config.get() + config.value,
        Cmd.Stop => 0,
    }
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub enum Command {
    Retry(Int),
    Stop,
}

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int {
        return self.value
    }
}

pub fn build() -> Command {
    return Command.Retry(0)
}

pub fn read(config: Config) -> Int {
    return config.get() + config.value
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub enum Command {
    Retry(Int),
    Stop,
}

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int
}
"#,
        );

        let core_source = fs::read_to_string(&core_source_path).expect("core source should read");
        let core_analysis = analyze_source(&core_source).expect("core source should analyze");
        let package =
            package_analysis_for_path(&core_source_path).expect("package analysis should succeed");
        let core_uri =
            Url::from_file_path(&core_source_path).expect("core source path should convert to URI");
        let app_source = fs::read_to_string(&app_path).expect("app source should read");
        let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let task_source = fs::read_to_string(&task_path).expect("task source should read");
        let task_uri = Url::from_file_path(&task_path).expect("task path should convert to URI");
        let open_docs = file_open_documents(vec![
            (core_uri.clone(), core_source.clone()),
            (app_uri.clone(), app_source.clone()),
            (task_uri.clone(), task_source.clone()),
        ]);

        let variant_references = workspace_source_references_for_root_symbol_with_open_docs(
            &core_uri,
            &core_source,
            &core_analysis,
            &package,
            &open_docs,
            offset_to_position(&core_source, nth_offset(&core_source, "Retry", 2)),
            true,
        )
        .expect("workspace root variant references should exist");

        assert_eq!(variant_references.len(), 6);
        assert!(variant_references.iter().any(|reference| {
            reference.uri == core_uri
                && reference.range.start
                    == offset_to_position(&core_source, nth_offset(&core_source, "Retry", 1))
        }));
        assert!(variant_references.iter().any(|reference| {
            reference.uri == core_uri
                && reference.range.start
                    == offset_to_position(&core_source, nth_offset(&core_source, "Retry", 2))
        }));
        assert!(variant_references.iter().any(|reference| {
            reference.uri == app_uri
                && reference.range.start
                    == offset_to_position(&app_source, nth_offset(&app_source, "Retry", 1))
        }));
        assert!(variant_references.iter().any(|reference| {
            reference.uri == app_uri
                && reference.range.start
                    == offset_to_position(&app_source, nth_offset(&app_source, "Retry", 2))
        }));
        assert!(variant_references.iter().any(|reference| {
            reference.uri == task_uri
                && reference.range.start
                    == offset_to_position(&task_source, nth_offset(&task_source, "Retry", 1))
        }));
        assert!(variant_references.iter().any(|reference| {
            reference.uri == task_uri
                && reference.range.start
                    == offset_to_position(&task_source, nth_offset(&task_source, "Retry", 2))
        }));

        let method_references = workspace_source_references_for_root_symbol_with_open_docs(
            &core_uri,
            &core_source,
            &core_analysis,
            &package,
            &open_docs,
            offset_to_position(&core_source, nth_offset(&core_source, "get", 2)),
            true,
        )
        .expect("workspace root method references should exist");

        assert_eq!(method_references.len(), 4);
        assert!(method_references.iter().any(|reference| {
            reference.uri == core_uri
                && reference.range.start
                    == offset_to_position(&core_source, nth_offset(&core_source, "get", 1))
        }));
        assert!(method_references.iter().any(|reference| {
            reference.uri == core_uri
                && reference.range.start
                    == offset_to_position(&core_source, nth_offset(&core_source, "get", 2))
        }));
        assert!(method_references.iter().any(|reference| {
            reference.uri == app_uri
                && reference.range.start
                    == offset_to_position(&app_source, nth_offset(&app_source, "get", 1))
        }));
        assert!(method_references.iter().any(|reference| {
            reference.uri == task_uri
                && reference.range.start
                    == offset_to_position(&task_source, nth_offset(&task_source, "get", 1))
        }));

        let field_references = workspace_source_references_for_root_symbol_with_open_docs(
            &core_uri,
            &core_source,
            &core_analysis,
            &package,
            &open_docs,
            offset_to_position(&core_source, nth_offset(&core_source, "value", 3)),
            true,
        )
        .expect("workspace root field references should exist");

        assert_eq!(field_references.len(), 5);
        assert!(field_references.iter().any(|reference| {
            reference.uri == core_uri
                && reference.range.start
                    == offset_to_position(&core_source, nth_offset(&core_source, "value", 1))
        }));
        assert!(field_references.iter().any(|reference| {
            reference.uri == core_uri
                && reference.range.start
                    == offset_to_position(&core_source, nth_offset(&core_source, "value", 2))
        }));
        assert!(field_references.iter().any(|reference| {
            reference.uri == core_uri
                && reference.range.start
                    == offset_to_position(&core_source, nth_offset(&core_source, "value", 3))
        }));
        assert!(field_references.iter().any(|reference| {
            reference.uri == app_uri
                && reference.range.start
                    == offset_to_position(&app_source, nth_offset(&app_source, "value", 1))
        }));
        assert!(field_references.iter().any(|reference| {
            reference.uri == task_uri
                && reference.range.start
                    == offset_to_position(&task_source, nth_offset(&task_source, "value", 1))
        }));
    }

    #[test]
    fn workspace_root_member_references_include_visible_broken_consumers() {
        let temp = TempDir::new("ql-lsp-workspace-root-member-references-broken");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.Command as Cmd
use demo.core.Config as Cfg

pub fn main(config: Cfg) -> Int {
    let command = Cmd.Retry(1)
    match command {
        Cmd.Retry(count) => count + config.get() + config.value,
        Cmd.Stop => 0,
"#,
        );
        let jobs_path = temp.write(
            "workspace/packages/jobs/src/job.ql",
            r#"
package demo.jobs

use demo.core.Command as Cmd
use demo.core.Config as JobCfg

pub fn job(config: JobCfg) -> Int {
    let command = Cmd.Retry(2)
    match command {
        Cmd.Retry(count) => count + config.get() + config.value,
        Cmd.Stop => 0,
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub enum Command {
    Retry(Int),
    Stop,
}

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int {
        return self.value
    }
}

pub fn build() -> Command {
    return Command.Retry(0)
}

pub fn read(config: Config) -> Int {
    return config.get() + config.value
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/jobs", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/jobs/qlang.toml",
            r#"
[package]
name = "jobs"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub enum Command {
    Retry(Int),
    Stop,
}

pub struct Config {
    value: Int,
}

impl Config {
    pub fn get(self) -> Int
}
"#,
        );

        let core_source = fs::read_to_string(&core_source_path).expect("core source should read");
        let core_analysis = analyze_source(&core_source).expect("core source should analyze");
        let package =
            package_analysis_for_path(&core_source_path).expect("package analysis should succeed");
        let core_uri =
            Url::from_file_path(&core_source_path).expect("core source path should convert to URI");
        let app_source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&app_source).is_err());
        let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let jobs_source = fs::read_to_string(&jobs_path).expect("jobs source should read");
        assert!(analyze_source(&jobs_source).is_err());
        let jobs_uri = Url::from_file_path(&jobs_path).expect("jobs path should convert to URI");
        let open_docs = file_open_documents(vec![
            (core_uri.clone(), core_source.clone()),
            (app_uri.clone(), app_source.clone()),
            (jobs_uri.clone(), jobs_source.clone()),
        ]);

        let variant_references = workspace_source_references_for_root_symbol_with_open_docs(
            &core_uri,
            &core_source,
            &core_analysis,
            &package,
            &open_docs,
            offset_to_position(&core_source, nth_offset(&core_source, "Retry", 2)),
            true,
        )
        .expect("workspace root variant references should exist");

        assert_eq!(variant_references.len(), 6);
        assert!(variant_references.iter().any(|reference| {
            reference.uri == core_uri
                && reference.range.start
                    == offset_to_position(&core_source, nth_offset(&core_source, "Retry", 1))
        }));
        assert!(variant_references.iter().any(|reference| {
            reference.uri == core_uri
                && reference.range.start
                    == offset_to_position(&core_source, nth_offset(&core_source, "Retry", 2))
        }));
        assert!(variant_references.iter().any(|reference| {
            reference.uri == app_uri
                && reference.range.start
                    == offset_to_position(&app_source, nth_offset(&app_source, "Retry", 1))
        }));
        assert!(variant_references.iter().any(|reference| {
            reference.uri == app_uri
                && reference.range.start
                    == offset_to_position(&app_source, nth_offset(&app_source, "Retry", 2))
        }));
        assert!(variant_references.iter().any(|reference| {
            reference.uri == jobs_uri
                && reference.range.start
                    == offset_to_position(&jobs_source, nth_offset(&jobs_source, "Retry", 1))
        }));
        assert!(variant_references.iter().any(|reference| {
            reference.uri == jobs_uri
                && reference.range.start
                    == offset_to_position(&jobs_source, nth_offset(&jobs_source, "Retry", 2))
        }));

        let method_references = workspace_source_references_for_root_symbol_with_open_docs(
            &core_uri,
            &core_source,
            &core_analysis,
            &package,
            &open_docs,
            offset_to_position(&core_source, nth_offset(&core_source, "get", 2)),
            true,
        )
        .expect("workspace root method references should exist");

        assert_eq!(method_references.len(), 4);
        assert!(method_references.iter().any(|reference| {
            reference.uri == core_uri
                && reference.range.start
                    == offset_to_position(&core_source, nth_offset(&core_source, "get", 1))
        }));
        assert!(method_references.iter().any(|reference| {
            reference.uri == core_uri
                && reference.range.start
                    == offset_to_position(&core_source, nth_offset(&core_source, "get", 2))
        }));
        assert!(method_references.iter().any(|reference| {
            reference.uri == app_uri
                && reference.range.start
                    == offset_to_position(&app_source, nth_offset(&app_source, "get", 1))
        }));
        assert!(method_references.iter().any(|reference| {
            reference.uri == jobs_uri
                && reference.range.start
                    == offset_to_position(&jobs_source, nth_offset(&jobs_source, "get", 1))
        }));

        let field_references = workspace_source_references_for_root_symbol_with_open_docs(
            &core_uri,
            &core_source,
            &core_analysis,
            &package,
            &open_docs,
            offset_to_position(&core_source, nth_offset(&core_source, "value", 3)),
            true,
        )
        .expect("workspace root field references should exist");

        assert_eq!(field_references.len(), 5);
        assert!(field_references.iter().any(|reference| {
            reference.uri == core_uri
                && reference.range.start
                    == offset_to_position(&core_source, nth_offset(&core_source, "value", 1))
        }));
        assert!(field_references.iter().any(|reference| {
            reference.uri == core_uri
                && reference.range.start
                    == offset_to_position(&core_source, nth_offset(&core_source, "value", 2))
        }));
        assert!(field_references.iter().any(|reference| {
            reference.uri == core_uri
                && reference.range.start
                    == offset_to_position(&core_source, nth_offset(&core_source, "value", 3))
        }));
        assert!(field_references.iter().any(|reference| {
            reference.uri == app_uri
                && reference.range.start
                    == offset_to_position(&app_source, nth_offset(&app_source, "value", 1))
        }));
        assert!(field_references.iter().any(|reference| {
            reference.uri == jobs_uri
                && reference.range.start
                    == offset_to_position(&jobs_source, nth_offset(&jobs_source, "value", 1))
        }));
    }

    #[test]
    fn workspace_root_field_rename_updates_broken_consumers_without_touching_same_named_root_imports()
     {
        let temp = TempDir::new("ql-lsp-workspace-root-field-rename-broken-consumers");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.Config as Cfg
use demo.core.value

pub fn main(config: Cfg) -> Int {
    let current = config.value
    return value(current) + config.
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub struct Config {
    value: Int,
}

pub fn value(current: Int) -> Int {
    return current
}

impl Config {
    pub fn total(self) -> Int {
        return self.value + value(self.value)
    }
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub struct Config {
    value: Int,
}

pub fn value(current: Int) -> Int
"#,
        );

        let core_source = fs::read_to_string(&core_source_path).expect("core source should read");
        let core_analysis = analyze_source(&core_source).expect("core source should analyze");
        let package =
            package_analysis_for_path(&core_source_path).expect("package analysis should succeed");
        let core_uri =
            Url::from_file_path(&core_source_path).expect("core source path should convert to URI");
        let app_source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&app_source).is_err());
        let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");

        let edit = rename_for_workspace_source_root_symbol_with_open_docs(
            &core_uri,
            &core_source,
            &core_analysis,
            &package,
            &file_open_documents(vec![
                (core_uri.clone(), core_source.clone()),
                (app_uri.clone(), app_source.clone()),
            ]),
            offset_to_position(&core_source, nth_offset(&core_source, "value", 1)),
            "count",
        )
        .expect("rename should succeed")
        .expect("rename should return workspace edits");

        assert_workspace_edit_changes(
            edit,
            vec![
                (
                    core_uri,
                    vec![
                        TextEdit::new(
                            span_to_range(
                                &core_source,
                                Span::new(
                                    nth_offset(&core_source, "value", 1),
                                    nth_offset(&core_source, "value", 1) + "value".len(),
                                ),
                            ),
                            "count".to_owned(),
                        ),
                        TextEdit::new(
                            span_to_range(
                                &core_source,
                                Span::new(
                                    nth_offset(&core_source, "value", 3),
                                    nth_offset(&core_source, "value", 3) + "value".len(),
                                ),
                            ),
                            "count".to_owned(),
                        ),
                        TextEdit::new(
                            span_to_range(
                                &core_source,
                                Span::new(
                                    nth_offset(&core_source, "value", 5),
                                    nth_offset(&core_source, "value", 5) + "value".len(),
                                ),
                            ),
                            "count".to_owned(),
                        ),
                    ],
                ),
                (
                    app_uri,
                    vec![TextEdit::new(
                        span_to_range(
                            &app_source,
                            Span::new(
                                nth_offset(&app_source, "value", 2),
                                nth_offset(&app_source, "value", 2) + "value".len(),
                            ),
                        ),
                        "count".to_owned(),
                    )],
                ),
            ],
        );
    }

    #[test]
    fn workspace_root_variant_rename_updates_consumers_without_touching_same_named_root_imports() {
        let temp = TempDir::new("ql-lsp-workspace-root-variant-rename");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.Command as Cmd
use demo.core.Retry as retry_fn

pub fn main() -> Int {
    let command = Cmd.Retry(1)
    match command {
        Cmd.Retry(count) => retry_fn(count),
        Cmd.Stop => 0,
    }
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub enum Command {
    Retry(Int),
    Stop,
}

pub fn Retry(current: Int) -> Int {
    return current
}

pub fn build() -> Command {
    return Command.Retry(0)
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub enum Command {
    Retry(Int),
    Stop,
}

pub fn Retry(current: Int) -> Int
"#,
        );

        let core_source = fs::read_to_string(&core_source_path).expect("core source should read");
        let core_analysis = analyze_source(&core_source).expect("core source should analyze");
        let package =
            package_analysis_for_path(&core_source_path).expect("package analysis should succeed");
        let core_uri =
            Url::from_file_path(&core_source_path).expect("core source path should convert to URI");
        let app_source = fs::read_to_string(&app_path).expect("app source should read");
        let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");

        let edit = rename_for_workspace_source_root_symbol_with_open_docs(
            &core_uri,
            &core_source,
            &core_analysis,
            &package,
            &file_open_documents(vec![
                (core_uri.clone(), core_source.clone()),
                (app_uri.clone(), app_source.clone()),
            ]),
            offset_to_position(&core_source, nth_offset(&core_source, "Retry", 1)),
            "Again",
        )
        .expect("rename should succeed")
        .expect("rename should return workspace edits");

        assert_workspace_edit_changes(
            edit,
            vec![
                (
                    core_uri,
                    vec![
                        TextEdit::new(
                            span_to_range(
                                &core_source,
                                Span::new(
                                    nth_offset(&core_source, "Retry", 1),
                                    nth_offset(&core_source, "Retry", 1) + "Retry".len(),
                                ),
                            ),
                            "Again".to_owned(),
                        ),
                        TextEdit::new(
                            span_to_range(
                                &core_source,
                                Span::new(
                                    nth_offset(&core_source, "Retry", 3),
                                    nth_offset(&core_source, "Retry", 3) + "Retry".len(),
                                ),
                            ),
                            "Again".to_owned(),
                        ),
                    ],
                ),
                (
                    app_uri,
                    vec![
                        TextEdit::new(
                            span_to_range(
                                &app_source,
                                Span::new(
                                    nth_offset(&app_source, "Retry", 2),
                                    nth_offset(&app_source, "Retry", 2) + "Retry".len(),
                                ),
                            ),
                            "Again".to_owned(),
                        ),
                        TextEdit::new(
                            span_to_range(
                                &app_source,
                                Span::new(
                                    nth_offset(&app_source, "Retry", 3),
                                    nth_offset(&app_source, "Retry", 3) + "Retry".len(),
                                ),
                            ),
                            "Again".to_owned(),
                        ),
                    ],
                ),
            ],
        );
    }

    #[test]
    fn workspace_root_function_rename_updates_workspace_import_paths_and_direct_uses() {
        let temp = TempDir::new("ql-lsp-workspace-root-function-rename");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.measure as compute

pub fn main() -> Int {
    return compute(1)
}
"#,
        );
        let task_path = temp.write(
            "workspace/packages/app/src/task.ql",
            r#"
package demo.app

use demo.core.measure

pub fn task() -> Int {
    return measure(2)
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn measure(value: Int) -> Int {
    return value
}

pub fn wrap(value: Int) -> Int {
    return measure(value)
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn measure(value: Int) -> Int
"#,
        );

        let core_source = fs::read_to_string(&core_source_path).expect("core source should read");
        let core_analysis = analyze_source(&core_source).expect("core source should analyze");
        let package =
            package_analysis_for_path(&core_source_path).expect("package analysis should succeed");
        let core_uri =
            Url::from_file_path(&core_source_path).expect("core source path should convert to URI");
        let app_source = fs::read_to_string(&app_path).expect("app source should read");
        let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let task_source = fs::read_to_string(&task_path).expect("task source should read");
        let task_uri = Url::from_file_path(&task_path).expect("task path should convert to URI");

        let edit = rename_for_workspace_source_root_symbol_with_open_docs(
            &core_uri,
            &core_source,
            &core_analysis,
            &package,
            &file_open_documents(vec![
                (core_uri.clone(), core_source.clone()),
                (app_uri.clone(), app_source.clone()),
                (task_uri.clone(), task_source.clone()),
            ]),
            offset_to_position(&core_source, nth_offset(&core_source, "measure", 2)),
            "score",
        )
        .expect("rename should succeed")
        .expect("rename should return workspace edits");

        assert_workspace_edit_changes(
            edit,
            vec![
                (
                    core_uri,
                    vec![
                        TextEdit::new(
                            span_to_range(
                                &core_source,
                                Span::new(
                                    nth_offset(&core_source, "measure", 1),
                                    nth_offset(&core_source, "measure", 1) + "measure".len(),
                                ),
                            ),
                            "score".to_owned(),
                        ),
                        TextEdit::new(
                            span_to_range(
                                &core_source,
                                Span::new(
                                    nth_offset(&core_source, "measure", 2),
                                    nth_offset(&core_source, "measure", 2) + "measure".len(),
                                ),
                            ),
                            "score".to_owned(),
                        ),
                    ],
                ),
                (
                    app_uri,
                    vec![TextEdit::new(
                        span_to_range(
                            &app_source,
                            Span::new(
                                nth_offset(&app_source, "measure", 1),
                                nth_offset(&app_source, "measure", 1) + "measure".len(),
                            ),
                        ),
                        "score".to_owned(),
                    )],
                ),
                (
                    task_uri,
                    vec![
                        TextEdit::new(
                            span_to_range(
                                &task_source,
                                Span::new(
                                    nth_offset(&task_source, "measure", 1),
                                    nth_offset(&task_source, "measure", 1) + "measure".len(),
                                ),
                            ),
                            "score".to_owned(),
                        ),
                        TextEdit::new(
                            span_to_range(
                                &task_source,
                                Span::new(
                                    nth_offset(&task_source, "measure", 2),
                                    nth_offset(&task_source, "measure", 2) + "measure".len(),
                                ),
                            ),
                            "score".to_owned(),
                        ),
                    ],
                ),
            ],
        );
    }

    #[test]
    fn workspace_root_function_rename_updates_visible_broken_consumers() {
        let temp = TempDir::new("ql-lsp-workspace-root-function-rename-visible-broken-consumers");
        let broken_core_path = temp.write(
            "workspace/packages/core/src/broken.ql",
            r#"
package demo.core

use demo.core.measure as run

pub fn broken_local() -> Int {
    return run(1)
"#,
        );
        let jobs_path = temp.write(
            "workspace/packages/jobs/src/job.ql",
            r#"
package demo.jobs

use demo.core.measure

pub fn job() -> Int {
    let first = measure(2)
    return measure(first)
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn measure(value: Int) -> Int {
    return value
}

pub fn wrap(value: Int) -> Int {
    return measure(value)
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/core", "packages/jobs"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/jobs/qlang.toml",
            r#"
[package]
name = "jobs"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn measure(value: Int) -> Int
"#,
        );

        let core_source = fs::read_to_string(&core_source_path).expect("core source should read");
        let core_analysis = analyze_source(&core_source).expect("core source should analyze");
        let package =
            package_analysis_for_path(&core_source_path).expect("package analysis should succeed");
        let core_uri =
            Url::from_file_path(&core_source_path).expect("core source path should convert to URI");
        let broken_core_source =
            fs::read_to_string(&broken_core_path).expect("broken core source should read");
        assert!(analyze_source(&broken_core_source).is_err());
        let broken_core_uri =
            Url::from_file_path(&broken_core_path).expect("broken core path should convert to URI");
        let jobs_source = fs::read_to_string(&jobs_path).expect("jobs source should read");
        assert!(analyze_source(&jobs_source).is_err());
        let jobs_uri = Url::from_file_path(&jobs_path).expect("jobs path should convert to URI");

        let edit = rename_for_workspace_source_root_symbol_with_open_docs(
            &core_uri,
            &core_source,
            &core_analysis,
            &package,
            &file_open_documents(vec![
                (core_uri.clone(), core_source.clone()),
                (broken_core_uri.clone(), broken_core_source.clone()),
                (jobs_uri.clone(), jobs_source.clone()),
            ]),
            offset_to_position(&core_source, nth_offset(&core_source, "measure", 1)),
            "score",
        )
        .expect("rename should succeed")
        .expect("rename should return workspace edits");

        assert_workspace_edit_changes(
            edit,
            vec![
                (
                    core_uri,
                    vec![
                        TextEdit::new(
                            span_to_range(
                                &core_source,
                                Span::new(
                                    nth_offset(&core_source, "measure", 1),
                                    nth_offset(&core_source, "measure", 1) + "measure".len(),
                                ),
                            ),
                            "score".to_owned(),
                        ),
                        TextEdit::new(
                            span_to_range(
                                &core_source,
                                Span::new(
                                    nth_offset(&core_source, "measure", 2),
                                    nth_offset(&core_source, "measure", 2) + "measure".len(),
                                ),
                            ),
                            "score".to_owned(),
                        ),
                    ],
                ),
                (
                    broken_core_uri,
                    vec![TextEdit::new(
                        span_to_range(
                            &broken_core_source,
                            Span::new(
                                nth_offset(&broken_core_source, "measure", 1),
                                nth_offset(&broken_core_source, "measure", 1) + "measure".len(),
                            ),
                        ),
                        "score".to_owned(),
                    )],
                ),
                (
                    jobs_uri,
                    vec![
                        TextEdit::new(
                            span_to_range(
                                &jobs_source,
                                Span::new(
                                    nth_offset(&jobs_source, "measure", 1),
                                    nth_offset(&jobs_source, "measure", 1) + "measure".len(),
                                ),
                            ),
                            "score".to_owned(),
                        ),
                        TextEdit::new(
                            span_to_range(
                                &jobs_source,
                                Span::new(
                                    nth_offset(&jobs_source, "measure", 2),
                                    nth_offset(&jobs_source, "measure", 2) + "measure".len(),
                                ),
                            ),
                            "score".to_owned(),
                        ),
                        TextEdit::new(
                            span_to_range(
                                &jobs_source,
                                Span::new(
                                    nth_offset(&jobs_source, "measure", 3),
                                    nth_offset(&jobs_source, "measure", 3) + "measure".len(),
                                ),
                            ),
                            "score".to_owned(),
                        ),
                    ],
                ),
            ],
        );
    }

    #[test]
    fn workspace_root_struct_rename_updates_type_import_paths_and_direct_type_uses() {
        let temp = TempDir::new("ql-lsp-workspace-root-struct-rename");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.Config as Cfg

pub fn main(config: Cfg) -> Int {
    return config.value
}
"#,
        );
        let task_path = temp.write(
            "workspace/packages/app/src/task.ql",
            r#"
package demo.app

use demo.core.Config

pub fn task(config: Config) -> Config {
    return config
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub struct Config {
    value: Int,
}

pub fn copy(config: Config) -> Config {
    return config
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub struct Config {
    value: Int,
}
"#,
        );

        let core_source = fs::read_to_string(&core_source_path).expect("core source should read");
        let core_analysis = analyze_source(&core_source).expect("core source should analyze");
        let package =
            package_analysis_for_path(&core_source_path).expect("package analysis should succeed");
        let core_uri =
            Url::from_file_path(&core_source_path).expect("core source path should convert to URI");
        let app_source = fs::read_to_string(&app_path).expect("app source should read");
        let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let task_source = fs::read_to_string(&task_path).expect("task source should read");
        let task_uri = Url::from_file_path(&task_path).expect("task path should convert to URI");

        let edit = rename_for_workspace_source_root_symbol_with_open_docs(
            &core_uri,
            &core_source,
            &core_analysis,
            &package,
            &file_open_documents(vec![
                (core_uri.clone(), core_source.clone()),
                (app_uri.clone(), app_source.clone()),
                (task_uri.clone(), task_source.clone()),
            ]),
            offset_to_position(&core_source, nth_offset(&core_source, "Config", 2)),
            "Settings",
        )
        .expect("rename should succeed")
        .expect("rename should return workspace edits");

        assert_workspace_edit_changes(
            edit,
            vec![
                (
                    core_uri,
                    vec![
                        TextEdit::new(
                            span_to_range(
                                &core_source,
                                Span::new(
                                    nth_offset(&core_source, "Config", 1),
                                    nth_offset(&core_source, "Config", 1) + "Config".len(),
                                ),
                            ),
                            "Settings".to_owned(),
                        ),
                        TextEdit::new(
                            span_to_range(
                                &core_source,
                                Span::new(
                                    nth_offset(&core_source, "Config", 2),
                                    nth_offset(&core_source, "Config", 2) + "Config".len(),
                                ),
                            ),
                            "Settings".to_owned(),
                        ),
                        TextEdit::new(
                            span_to_range(
                                &core_source,
                                Span::new(
                                    nth_offset(&core_source, "Config", 3),
                                    nth_offset(&core_source, "Config", 3) + "Config".len(),
                                ),
                            ),
                            "Settings".to_owned(),
                        ),
                    ],
                ),
                (
                    app_uri,
                    vec![TextEdit::new(
                        span_to_range(
                            &app_source,
                            Span::new(
                                nth_offset(&app_source, "Config", 1),
                                nth_offset(&app_source, "Config", 1) + "Config".len(),
                            ),
                        ),
                        "Settings".to_owned(),
                    )],
                ),
                (
                    task_uri,
                    vec![
                        TextEdit::new(
                            span_to_range(
                                &task_source,
                                Span::new(
                                    nth_offset(&task_source, "Config", 1),
                                    nth_offset(&task_source, "Config", 1) + "Config".len(),
                                ),
                            ),
                            "Settings".to_owned(),
                        ),
                        TextEdit::new(
                            span_to_range(
                                &task_source,
                                Span::new(
                                    nth_offset(&task_source, "Config", 2),
                                    nth_offset(&task_source, "Config", 2) + "Config".len(),
                                ),
                            ),
                            "Settings".to_owned(),
                        ),
                        TextEdit::new(
                            span_to_range(
                                &task_source,
                                Span::new(
                                    nth_offset(&task_source, "Config", 3),
                                    nth_offset(&task_source, "Config", 3) + "Config".len(),
                                ),
                            ),
                            "Settings".to_owned(),
                        ),
                    ],
                ),
            ],
        );
    }

    #[test]
    fn workspace_root_function_prepare_rename_from_direct_import_use_prefers_root_symbol() {
        let temp = TempDir::new("ql-lsp-workspace-root-function-prepare-rename-from-import-use");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.measure

pub fn main() -> Int {
    return measure(1)
}
"#,
        );
        let _core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn measure(value: Int) -> Int {
    return value
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn measure(value: Int) -> Int
"#,
        );

        let app_source = fs::read_to_string(&app_path).expect("app source should read");
        let app_analysis = analyze_source(&app_source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");

        assert_eq!(
            prepare_rename_for_workspace_source_root_symbol_from_import(
                &app_uri,
                &app_source,
                &app_analysis,
                &package,
                offset_to_position(&app_source, nth_offset(&app_source, "measure", 2)),
            ),
            Some(PrepareRenameResponse::RangeWithPlaceholder {
                range: span_to_range(
                    &app_source,
                    Span::new(
                        nth_offset(&app_source, "measure", 2),
                        nth_offset(&app_source, "measure", 2) + "measure".len(),
                    ),
                ),
                placeholder: "measure".to_owned(),
            }),
        );
    }

    #[test]
    fn workspace_root_function_prepare_rename_from_aliased_import_use_prefers_root_symbol() {
        let temp =
            TempDir::new("ql-lsp-workspace-root-function-prepare-rename-from-aliased-import-use");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.measure as run

pub fn main() -> Int {
    return run(1)
}
"#,
        );
        let _core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn measure(value: Int) -> Int {
    return value
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn measure(value: Int) -> Int
"#,
        );

        let app_source = fs::read_to_string(&app_path).expect("app source should read");
        let app_analysis = analyze_source(&app_source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");

        assert_eq!(
            prepare_rename_for_workspace_source_root_symbol_from_import(
                &app_uri,
                &app_source,
                &app_analysis,
                &package,
                offset_to_position(&app_source, nth_offset(&app_source, "run", 2)),
            ),
            Some(PrepareRenameResponse::RangeWithPlaceholder {
                range: span_to_range(
                    &app_source,
                    Span::new(
                        nth_offset(&app_source, "run", 2),
                        nth_offset(&app_source, "run", 2) + "run".len(),
                    ),
                ),
                placeholder: "run".to_owned(),
            }),
        );
    }

    #[test]
    fn workspace_root_function_prepare_rename_from_import_use_prefers_open_workspace_source() {
        let temp =
            TempDir::new("ql-lsp-workspace-root-function-prepare-rename-from-import-use-open-docs");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.measure

pub fn main() -> Int {
    return measure(1)
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn helper() -> Int {
    return 0
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn measure(value: Int) -> Int
"#,
        );

        let app_source = fs::read_to_string(&app_path).expect("app source should read");
        let app_analysis = analyze_source(&app_source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let core_uri =
            Url::from_file_path(&core_source_path).expect("core source path should convert to URI");
        let open_core_source = r#"
package demo.core

pub fn helper() -> Int {
    return 0
}

pub fn measure(value: Int) -> Int {
    return value
}
"#
        .to_owned();
        let use_offset = nth_offset(&app_source, "measure", 2);

        assert_eq!(
            prepare_rename_for_workspace_source_root_symbol_from_import(
                &app_uri,
                &app_source,
                &app_analysis,
                &package,
                offset_to_position(&app_source, use_offset),
            ),
            None,
            "disk-only prepare rename should miss unsaved workspace source",
        );
        assert_eq!(
            prepare_rename_for_workspace_source_root_symbol_from_import_with_open_docs(
                &app_uri,
                &app_source,
                &app_analysis,
                &package,
                &file_open_documents(vec![
                    (app_uri.clone(), app_source.clone()),
                    (core_uri, open_core_source),
                ]),
                offset_to_position(&app_source, use_offset),
            ),
            Some(PrepareRenameResponse::RangeWithPlaceholder {
                range: span_to_range(
                    &app_source,
                    Span::new(use_offset, use_offset + "measure".len()),
                ),
                placeholder: "measure".to_owned(),
            }),
        );
    }

    #[test]
    fn workspace_root_function_rename_from_direct_import_use_updates_workspace() {
        let temp = TempDir::new("ql-lsp-workspace-root-function-rename-from-import-use");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.measure

pub fn main() -> Int {
    return measure(1)
}
"#,
        );
        let task_path = temp.write(
            "workspace/packages/app/src/task.ql",
            r#"
package demo.app

use demo.core.measure

pub fn task() -> Int {
    return measure(2)
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn measure(value: Int) -> Int {
    return value
}

pub fn wrap(value: Int) -> Int {
    return measure(value)
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn measure(value: Int) -> Int
"#,
        );

        let app_source = fs::read_to_string(&app_path).expect("app source should read");
        let app_analysis = analyze_source(&app_source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let task_source = fs::read_to_string(&task_path).expect("task source should read");
        let task_uri = Url::from_file_path(&task_path).expect("task path should convert to URI");
        let core_source = fs::read_to_string(&core_source_path).expect("core source should read");
        let core_uri =
            Url::from_file_path(&core_source_path).expect("core source path should convert to URI");

        let edit = rename_for_workspace_source_root_symbol_from_import_with_open_docs(
            &app_uri,
            &app_source,
            &app_analysis,
            &package,
            &file_open_documents(vec![
                (app_uri.clone(), app_source.clone()),
                (task_uri.clone(), task_source.clone()),
                (core_uri.clone(), core_source.clone()),
            ]),
            offset_to_position(&app_source, nth_offset(&app_source, "measure", 2)),
            "score",
        )
        .expect("rename should succeed")
        .expect("rename should return workspace edits");

        assert_workspace_edit_changes(
            edit,
            vec![
                (
                    app_uri,
                    vec![
                        TextEdit::new(
                            span_to_range(
                                &app_source,
                                Span::new(
                                    nth_offset(&app_source, "measure", 1),
                                    nth_offset(&app_source, "measure", 1) + "measure".len(),
                                ),
                            ),
                            "score".to_owned(),
                        ),
                        TextEdit::new(
                            span_to_range(
                                &app_source,
                                Span::new(
                                    nth_offset(&app_source, "measure", 2),
                                    nth_offset(&app_source, "measure", 2) + "measure".len(),
                                ),
                            ),
                            "score".to_owned(),
                        ),
                    ],
                ),
                (
                    task_uri,
                    vec![
                        TextEdit::new(
                            span_to_range(
                                &task_source,
                                Span::new(
                                    nth_offset(&task_source, "measure", 1),
                                    nth_offset(&task_source, "measure", 1) + "measure".len(),
                                ),
                            ),
                            "score".to_owned(),
                        ),
                        TextEdit::new(
                            span_to_range(
                                &task_source,
                                Span::new(
                                    nth_offset(&task_source, "measure", 2),
                                    nth_offset(&task_source, "measure", 2) + "measure".len(),
                                ),
                            ),
                            "score".to_owned(),
                        ),
                    ],
                ),
                (
                    core_uri,
                    vec![
                        TextEdit::new(
                            span_to_range(
                                &core_source,
                                Span::new(
                                    nth_offset(&core_source, "measure", 1),
                                    nth_offset(&core_source, "measure", 1) + "measure".len(),
                                ),
                            ),
                            "score".to_owned(),
                        ),
                        TextEdit::new(
                            span_to_range(
                                &core_source,
                                Span::new(
                                    nth_offset(&core_source, "measure", 2),
                                    nth_offset(&core_source, "measure", 2) + "measure".len(),
                                ),
                            ),
                            "score".to_owned(),
                        ),
                    ],
                ),
            ],
        );
    }

    #[test]
    fn workspace_root_function_rename_from_aliased_import_use_updates_workspace() {
        let temp = TempDir::new("ql-lsp-workspace-root-function-rename-from-aliased-import-use");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.measure as run

pub fn main() -> Int {
    return run(1)
}
"#,
        );
        let task_path = temp.write(
            "workspace/packages/app/src/task.ql",
            r#"
package demo.app

use demo.core.measure

pub fn task() -> Int {
    return measure(2)
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn measure(value: Int) -> Int {
    return value
}

pub fn wrap(value: Int) -> Int {
    return measure(value)
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn measure(value: Int) -> Int
"#,
        );

        let app_source = fs::read_to_string(&app_path).expect("app source should read");
        let app_analysis = analyze_source(&app_source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let task_source = fs::read_to_string(&task_path).expect("task source should read");
        let task_uri = Url::from_file_path(&task_path).expect("task path should convert to URI");
        let core_source = fs::read_to_string(&core_source_path).expect("core source should read");
        let core_uri =
            Url::from_file_path(&core_source_path).expect("core source path should convert to URI");

        let edit = rename_for_workspace_source_root_symbol_from_import_with_open_docs(
            &app_uri,
            &app_source,
            &app_analysis,
            &package,
            &file_open_documents(vec![
                (app_uri.clone(), app_source.clone()),
                (task_uri.clone(), task_source.clone()),
                (core_uri.clone(), core_source.clone()),
            ]),
            offset_to_position(&app_source, nth_offset(&app_source, "run", 2)),
            "score",
        )
        .expect("rename should succeed")
        .expect("rename should return workspace edits");

        assert_workspace_edit_changes(
            edit,
            vec![
                (
                    app_uri,
                    vec![TextEdit::new(
                        span_to_range(
                            &app_source,
                            Span::new(
                                nth_offset(&app_source, "measure", 1),
                                nth_offset(&app_source, "measure", 1) + "measure".len(),
                            ),
                        ),
                        "score".to_owned(),
                    )],
                ),
                (
                    task_uri,
                    vec![
                        TextEdit::new(
                            span_to_range(
                                &task_source,
                                Span::new(
                                    nth_offset(&task_source, "measure", 1),
                                    nth_offset(&task_source, "measure", 1) + "measure".len(),
                                ),
                            ),
                            "score".to_owned(),
                        ),
                        TextEdit::new(
                            span_to_range(
                                &task_source,
                                Span::new(
                                    nth_offset(&task_source, "measure", 2),
                                    nth_offset(&task_source, "measure", 2) + "measure".len(),
                                ),
                            ),
                            "score".to_owned(),
                        ),
                    ],
                ),
                (
                    core_uri,
                    vec![
                        TextEdit::new(
                            span_to_range(
                                &core_source,
                                Span::new(
                                    nth_offset(&core_source, "measure", 1),
                                    nth_offset(&core_source, "measure", 1) + "measure".len(),
                                ),
                            ),
                            "score".to_owned(),
                        ),
                        TextEdit::new(
                            span_to_range(
                                &core_source,
                                Span::new(
                                    nth_offset(&core_source, "measure", 2),
                                    nth_offset(&core_source, "measure", 2) + "measure".len(),
                                ),
                            ),
                            "score".to_owned(),
                        ),
                    ],
                ),
            ],
        );
    }

    #[test]
    fn workspace_root_struct_rename_from_direct_type_import_use_updates_workspace() {
        let temp = TempDir::new("ql-lsp-workspace-root-struct-rename-from-type-import-use");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.Config

pub fn main(config: Config) -> Int {
    return config.value
}
"#,
        );
        let task_path = temp.write(
            "workspace/packages/app/src/task.ql",
            r#"
package demo.app

use demo.core.Config

pub fn task(config: Config) -> Config {
    return config
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub struct Config {
    value: Int,
}

pub fn copy(config: Config) -> Config {
    return config
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub struct Config {
    value: Int,
}
"#,
        );

        let app_source = fs::read_to_string(&app_path).expect("app source should read");
        let app_analysis = analyze_source(&app_source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let task_source = fs::read_to_string(&task_path).expect("task source should read");
        let task_uri = Url::from_file_path(&task_path).expect("task path should convert to URI");
        let core_source = fs::read_to_string(&core_source_path).expect("core source should read");
        let core_uri =
            Url::from_file_path(&core_source_path).expect("core source path should convert to URI");

        let edit = rename_for_workspace_source_root_symbol_from_import_with_open_docs(
            &app_uri,
            &app_source,
            &app_analysis,
            &package,
            &file_open_documents(vec![
                (app_uri.clone(), app_source.clone()),
                (task_uri.clone(), task_source.clone()),
                (core_uri.clone(), core_source.clone()),
            ]),
            offset_to_position(&app_source, nth_offset(&app_source, "Config", 2)),
            "Settings",
        )
        .expect("rename should succeed")
        .expect("rename should return workspace edits");

        assert_workspace_edit_changes(
            edit,
            vec![
                (
                    app_uri,
                    vec![
                        TextEdit::new(
                            span_to_range(
                                &app_source,
                                Span::new(
                                    nth_offset(&app_source, "Config", 1),
                                    nth_offset(&app_source, "Config", 1) + "Config".len(),
                                ),
                            ),
                            "Settings".to_owned(),
                        ),
                        TextEdit::new(
                            span_to_range(
                                &app_source,
                                Span::new(
                                    nth_offset(&app_source, "Config", 2),
                                    nth_offset(&app_source, "Config", 2) + "Config".len(),
                                ),
                            ),
                            "Settings".to_owned(),
                        ),
                    ],
                ),
                (
                    task_uri,
                    vec![
                        TextEdit::new(
                            span_to_range(
                                &task_source,
                                Span::new(
                                    nth_offset(&task_source, "Config", 1),
                                    nth_offset(&task_source, "Config", 1) + "Config".len(),
                                ),
                            ),
                            "Settings".to_owned(),
                        ),
                        TextEdit::new(
                            span_to_range(
                                &task_source,
                                Span::new(
                                    nth_offset(&task_source, "Config", 2),
                                    nth_offset(&task_source, "Config", 2) + "Config".len(),
                                ),
                            ),
                            "Settings".to_owned(),
                        ),
                        TextEdit::new(
                            span_to_range(
                                &task_source,
                                Span::new(
                                    nth_offset(&task_source, "Config", 3),
                                    nth_offset(&task_source, "Config", 3) + "Config".len(),
                                ),
                            ),
                            "Settings".to_owned(),
                        ),
                    ],
                ),
                (
                    core_uri,
                    vec![
                        TextEdit::new(
                            span_to_range(
                                &core_source,
                                Span::new(
                                    nth_offset(&core_source, "Config", 1),
                                    nth_offset(&core_source, "Config", 1) + "Config".len(),
                                ),
                            ),
                            "Settings".to_owned(),
                        ),
                        TextEdit::new(
                            span_to_range(
                                &core_source,
                                Span::new(
                                    nth_offset(&core_source, "Config", 2),
                                    nth_offset(&core_source, "Config", 2) + "Config".len(),
                                ),
                            ),
                            "Settings".to_owned(),
                        ),
                        TextEdit::new(
                            span_to_range(
                                &core_source,
                                Span::new(
                                    nth_offset(&core_source, "Config", 3),
                                    nth_offset(&core_source, "Config", 3) + "Config".len(),
                                ),
                            ),
                            "Settings".to_owned(),
                        ),
                    ],
                ),
            ],
        );
    }

    #[test]
    fn workspace_root_struct_rename_from_aliased_type_import_use_updates_workspace() {
        let temp = TempDir::new("ql-lsp-workspace-root-struct-rename-from-aliased-type-import-use");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.Config as Cfg

pub fn main(config: Cfg) -> Int {
    return config.value
}
"#,
        );
        let task_path = temp.write(
            "workspace/packages/app/src/task.ql",
            r#"
package demo.app

use demo.core.Config

pub fn task(config: Config) -> Config {
    return config
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub struct Config {
    value: Int,
}

pub fn copy(config: Config) -> Config {
    return config
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub struct Config {
    value: Int,
}
"#,
        );

        let app_source = fs::read_to_string(&app_path).expect("app source should read");
        let app_analysis = analyze_source(&app_source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let task_source = fs::read_to_string(&task_path).expect("task source should read");
        let task_uri = Url::from_file_path(&task_path).expect("task path should convert to URI");
        let core_source = fs::read_to_string(&core_source_path).expect("core source should read");
        let core_uri =
            Url::from_file_path(&core_source_path).expect("core source path should convert to URI");

        let edit = rename_for_workspace_source_root_symbol_from_import_with_open_docs(
            &app_uri,
            &app_source,
            &app_analysis,
            &package,
            &file_open_documents(vec![
                (app_uri.clone(), app_source.clone()),
                (task_uri.clone(), task_source.clone()),
                (core_uri.clone(), core_source.clone()),
            ]),
            offset_to_position(&app_source, nth_offset(&app_source, "Cfg", 2)),
            "Settings",
        )
        .expect("rename should succeed")
        .expect("rename should return workspace edits");

        assert_workspace_edit_changes(
            edit,
            vec![
                (
                    app_uri,
                    vec![TextEdit::new(
                        span_to_range(
                            &app_source,
                            Span::new(
                                nth_offset(&app_source, "Config", 1),
                                nth_offset(&app_source, "Config", 1) + "Config".len(),
                            ),
                        ),
                        "Settings".to_owned(),
                    )],
                ),
                (
                    task_uri,
                    vec![
                        TextEdit::new(
                            span_to_range(
                                &task_source,
                                Span::new(
                                    nth_offset(&task_source, "Config", 1),
                                    nth_offset(&task_source, "Config", 1) + "Config".len(),
                                ),
                            ),
                            "Settings".to_owned(),
                        ),
                        TextEdit::new(
                            span_to_range(
                                &task_source,
                                Span::new(
                                    nth_offset(&task_source, "Config", 2),
                                    nth_offset(&task_source, "Config", 2) + "Config".len(),
                                ),
                            ),
                            "Settings".to_owned(),
                        ),
                        TextEdit::new(
                            span_to_range(
                                &task_source,
                                Span::new(
                                    nth_offset(&task_source, "Config", 3),
                                    nth_offset(&task_source, "Config", 3) + "Config".len(),
                                ),
                            ),
                            "Settings".to_owned(),
                        ),
                    ],
                ),
                (
                    core_uri,
                    vec![
                        TextEdit::new(
                            span_to_range(
                                &core_source,
                                Span::new(
                                    nth_offset(&core_source, "Config", 1),
                                    nth_offset(&core_source, "Config", 1) + "Config".len(),
                                ),
                            ),
                            "Settings".to_owned(),
                        ),
                        TextEdit::new(
                            span_to_range(
                                &core_source,
                                Span::new(
                                    nth_offset(&core_source, "Config", 2),
                                    nth_offset(&core_source, "Config", 2) + "Config".len(),
                                ),
                            ),
                            "Settings".to_owned(),
                        ),
                        TextEdit::new(
                            span_to_range(
                                &core_source,
                                Span::new(
                                    nth_offset(&core_source, "Config", 3),
                                    nth_offset(&core_source, "Config", 3) + "Config".len(),
                                ),
                            ),
                            "Settings".to_owned(),
                        ),
                    ],
                ),
            ],
        );
    }

    #[test]
    fn workspace_import_references_without_declaration_include_other_workspace_uses() {
        let temp = TempDir::new("ql-lsp-workspace-import-source-references-no-decl");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.exported as run

pub fn main() -> Int {
    return run(1)
}
"#,
        );
        let task_path = temp.write(
            "workspace/packages/app/src/task.ql",
            r#"
package demo.app

use demo.core.exported as call

pub fn task() -> Int {
    return call(2)
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn exported(value: Int) -> Int {
    return value
}

pub fn wrapper(value: Int) -> Int {
    return exported(value)
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn exported(value: Int) -> Int
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        let analysis = analyze_source(&source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let core_source =
            fs::read_to_string(&core_source_path).expect("core source should read for assertions");
        let task_source = fs::read_to_string(&task_path).expect("task source should read");
        let task_uri = Url::from_file_path(&task_path).expect("task path should convert to URI");

        let references = workspace_source_references_for_import(
            &uri,
            &source,
            &analysis,
            &package,
            offset_to_position(&source, nth_offset(&source, "run", 2)),
            false,
        )
        .expect("workspace import references without declaration should exist");

        assert_eq!(references.len(), 3);
        assert_eq!(references[0].uri, uri);
        assert_eq!(
            references[0].range.start,
            offset_to_position(&source, nth_offset(&source, "run", 2)),
        );
        assert_eq!(
            references[1]
                .uri
                .to_file_path()
                .expect("source reference URI should convert to a file path")
                .canonicalize()
                .expect("source reference path should canonicalize"),
            core_source_path
                .canonicalize()
                .expect("core source path should canonicalize"),
        );
        assert_eq!(
            references[1].range.start,
            offset_to_position(&core_source, nth_offset(&core_source, "exported", 2)),
        );
        assert_eq!(references[2].uri, task_uri);
        assert_eq!(
            references[2].range.start,
            offset_to_position(&task_source, nth_offset(&task_source, "call", 2)),
        );
    }

    #[test]
    fn workspace_import_references_use_open_workspace_sources_and_consumers() {
        let temp = TempDir::new("ql-lsp-workspace-import-source-references-open-consumers");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.exported as run

pub fn main() -> Int {
    return run(1)
}
"#,
        );
        let task_path = temp.write(
            "workspace/packages/app/src/task.ql",
            r#"
package demo.app

use demo.core.exported as call

pub fn task() -> Int {
    return call(2)
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn exported(value: Int) -> Int {
    return value
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn exported(value: Int) -> Int
"#,
        );

        let app_source = fs::read_to_string(&app_path).expect("app source should read");
        let app_analysis = analyze_source(&app_source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let task_uri = Url::from_file_path(&task_path).expect("task path should convert to URI");
        let core_uri =
            Url::from_file_path(&core_source_path).expect("core source path should convert to URI");
        let disk_task_source = fs::read_to_string(&task_path).expect("task source should read");
        let disk_core_source =
            fs::read_to_string(&core_source_path).expect("core source should read");
        let open_task_source = r#"
package demo.app


use demo.core.exported as ship

pub fn task() -> Int {
    let current = ship(2)
    return ship(current)
}
"#
        .to_owned();
        let open_core_source = r#"
package demo.core

pub fn helper() -> Int {
    return 0
}

pub fn exported(value: Int) -> Int {
    return value
}

pub fn wrapper(value: Int) -> Int {
    return exported(value)
}
"#
        .to_owned();

        let references = workspace_source_references_for_import_with_open_docs(
            &app_uri,
            &app_source,
            &app_analysis,
            &package,
            &file_open_documents(vec![
                (app_uri.clone(), app_source.clone()),
                (task_uri.clone(), open_task_source.clone()),
                (core_uri.clone(), open_core_source.clone()),
            ]),
            offset_to_position(&app_source, nth_offset(&app_source, "run", 2)),
            true,
        )
        .expect("workspace import references should use open sources and consumers");

        let contains = |uri: &Url, source: &str, needle: &str, occurrence: usize| {
            references.iter().any(|location| {
                location.uri == *uri
                    && location.range.start
                        == offset_to_position(source, nth_offset(source, needle, occurrence))
            })
        };

        assert_eq!(references.len(), 7);
        assert!(contains(&core_uri, &open_core_source, "exported", 1));
        assert!(contains(&core_uri, &open_core_source, "exported", 2));
        assert!(contains(&app_uri, &app_source, "run", 1));
        assert!(contains(&app_uri, &app_source, "run", 2));
        assert!(contains(&task_uri, &open_task_source, "ship", 1));
        assert!(contains(&task_uri, &open_task_source, "ship", 2));
        assert!(contains(&task_uri, &open_task_source, "ship", 3));
        assert!(
            !references.iter().any(|location| {
                location.uri == task_uri
                    && location.range.start
                        == offset_to_position(
                            &disk_task_source,
                            nth_offset(&disk_task_source, "call", 1),
                        )
            }),
            "references should not keep stale disk task import aliases",
        );
        assert!(
            !references.iter().any(|location| {
                location.uri == core_uri
                    && location.range.start
                        == offset_to_position(
                            &disk_core_source,
                            nth_offset(&disk_core_source, "exported", 1),
                        )
            }),
            "references should not keep stale disk source definition positions",
        );
    }

    #[test]
    fn workspace_import_references_survive_parse_errors_and_prefer_workspace_member_source() {
        let temp = TempDir::new("ql-lsp-workspace-import-source-references-parse-errors");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.exported as run

pub fn main() -> Int {
    let first = run(1)
    let second = run(first)
    return second
"#,
        );
        temp.write(
            "workspace/packages/app/src/task.ql",
            r#"
package demo.app

use demo.core.exported as call

pub fn task() -> Int {
    return call(2)
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn exported(value: Int) -> Int {
    return value
}

pub fn wrapper(value: Int) -> Int {
    return exported(value)
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn exported(value: Int) -> Int
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        let core_source =
            fs::read_to_string(&core_source_path).expect("core source should read for assertions");
        assert!(analyze_source(&source).is_err());
        let package = package_analysis_for_path(&app_path)
            .expect("package analysis should survive parse errors");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");

        let references = workspace_source_references_for_import_in_broken_source(
            &uri,
            &source,
            &package,
            offset_to_position(&source, nth_offset(&source, "run", 2)),
            true,
        )
        .expect("broken-source workspace import references should exist");

        assert_eq!(references.len(), 4);
        assert_eq!(
            references[0]
                .uri
                .to_file_path()
                .expect("definition URI should convert to a file path")
                .canonicalize()
                .expect("definition path should canonicalize"),
            core_source_path
                .canonicalize()
                .expect("core source path should canonicalize"),
        );
        assert_eq!(
            references[0].range.start,
            offset_to_position(&core_source, nth_offset(&core_source, "exported", 1)),
        );
        assert_eq!(references[1].uri, uri);
        assert_eq!(
            references[1].range.start,
            offset_to_position(&source, nth_offset(&source, "run", 2)),
        );
        assert_eq!(references[2].uri, uri);
        assert_eq!(
            references[2].range.start,
            offset_to_position(&source, nth_offset(&source, "run", 3)),
        );
        assert_eq!(
            references[3]
                .uri
                .to_file_path()
                .expect("source reference URI should convert to a file path")
                .canonicalize()
                .expect("source reference path should canonicalize"),
            core_source_path
                .canonicalize()
                .expect("core source path should canonicalize"),
        );
        assert_eq!(
            references[3].range.start,
            offset_to_position(&core_source, nth_offset(&core_source, "exported", 2)),
        );
    }

    #[test]
    fn local_dependency_import_references_survive_parse_errors_and_prefer_dependency_source() {
        let temp = TempDir::new("ql-lsp-local-dependency-import-source-references-parse-errors");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.exported as run

pub fn main() -> Int {
    let first = run(1)
    let second = run(first)
    return second
"#,
        );
        let core_source_path = temp.write(
            "workspace/vendor/core/src/lib.ql",
            r#"
package demo.core

pub fn exported(value: Int) -> Int {
    return value
}

pub fn wrapper(value: Int) -> Int {
    return exported(value)
}
"#,
        );
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
core = { path = "../../vendor/core" }
"#,
        );
        temp.write(
            "workspace/vendor/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/vendor/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn exported(value: Int) -> Int
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        let core_source =
            fs::read_to_string(&core_source_path).expect("core source should read for assertions");
        assert!(analyze_source(&source).is_err());
        let package = package_analysis_for_path(&app_path)
            .expect("package analysis should survive parse errors");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");

        let references = workspace_source_references_for_import_in_broken_source(
            &uri,
            &source,
            &package,
            offset_to_position(&source, nth_offset(&source, "run", 2)),
            true,
        )
        .expect("broken-source local dependency import references should exist");

        assert_eq!(references.len(), 4);
        assert_eq!(
            references[0]
                .uri
                .to_file_path()
                .expect("definition URI should convert to a file path")
                .canonicalize()
                .expect("definition path should canonicalize"),
            core_source_path
                .canonicalize()
                .expect("core source path should canonicalize"),
        );
        assert_eq!(
            references[0].range.start,
            offset_to_position(&core_source, nth_offset(&core_source, "exported", 1)),
        );
        assert_eq!(references[1].uri, uri);
        assert_eq!(
            references[1].range.start,
            offset_to_position(&source, nth_offset(&source, "run", 2)),
        );
        assert_eq!(references[2].uri, uri);
        assert_eq!(
            references[2].range.start,
            offset_to_position(&source, nth_offset(&source, "run", 3)),
        );
        assert_eq!(
            references[3]
                .uri
                .to_file_path()
                .expect("source reference URI should convert to a file path")
                .canonicalize()
                .expect("source reference path should canonicalize"),
            core_source_path
                .canonicalize()
                .expect("core source path should canonicalize"),
        );
        assert_eq!(
            references[3].range.start,
            offset_to_position(&core_source, nth_offset(&core_source, "exported", 2)),
        );
    }

    #[test]
    fn workspace_import_references_in_broken_source_prefer_open_workspace_member_source() {
        let temp = TempDir::new("ql-lsp-workspace-import-source-references-open-docs");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.exported as run

pub fn main() -> Int {
    let first = run(1)
    return run(first)
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn exported(value: Int) -> Int {
    return value
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn exported(value: Int) -> Int
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&source).is_err());
        let package = package_analysis_for_path(&app_path)
            .expect("package analysis should survive parse errors");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let core_uri =
            Url::from_file_path(&core_source_path).expect("core source path should convert to URI");
        let disk_core_source =
            fs::read_to_string(&core_source_path).expect("core source should read from disk");
        let open_core_source = r#"
package demo.core

pub fn helper() -> Int {
    return 0
}

pub fn exported(value: Int) -> Int {
    return value
}

pub fn wrapper(value: Int) -> Int {
    return exported(value)
}
"#
        .to_owned();

        let references = workspace_source_references_for_import_in_broken_source_with_open_docs(
            &uri,
            &source,
            &package,
            &file_open_documents(vec![(core_uri.clone(), open_core_source.clone())]),
            offset_to_position(&source, nth_offset(&source, "run", 2)),
            true,
        )
        .expect("broken-source workspace import references should exist");

        assert!(
            references.iter().any(|reference| {
                reference.uri == core_uri
                    && reference.range.start
                        == offset_to_position(
                            &open_core_source,
                            nth_offset(&open_core_source, "exported", 1),
                        )
            }),
            "references should include open workspace source definition",
        );
        assert!(
            references.iter().any(|reference| {
                reference.uri == core_uri
                    && reference.range.start
                        == offset_to_position(
                            &open_core_source,
                            nth_offset(&open_core_source, "exported", 2),
                        )
            }),
            "references should include open workspace source use",
        );
        assert!(
            references.iter().any(|reference| {
                reference.uri == uri
                    && reference.range.start
                        == offset_to_position(&source, nth_offset(&source, "run", 2))
            }),
            "references should include broken-source local use",
        );
        assert!(
            references.iter().any(|reference| {
                reference.uri == uri
                    && reference.range.start
                        == offset_to_position(&source, nth_offset(&source, "run", 3))
            }),
            "references should include second broken-source local use",
        );
        assert!(
            !references.iter().any(|reference| {
                reference.uri == core_uri
                    && reference.range.start
                        == offset_to_position(
                            &disk_core_source,
                            nth_offset(&disk_core_source, "exported", 1),
                        )
            }),
            "references should not fall back to disk definition position",
        );
    }

    #[test]
    fn workspace_import_references_without_declaration_survive_parse_errors() {
        let temp = TempDir::new("ql-lsp-workspace-import-source-references-parse-errors-no-decl");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.Config

pub fn main(value: Config) -> Config {
    let current: Config = Config { value: 1
    return current
"#,
        );
        temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub struct Config {
    value: Int,
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub struct Config {
    value: Int,
}
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&source).is_err());
        let package = package_analysis_for_path(&app_path)
            .expect("package analysis should survive parse errors");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");

        let references = workspace_source_references_for_import_in_broken_source(
            &uri,
            &source,
            &package,
            offset_to_position(&source, nth_offset(&source, "Config", 2)),
            false,
        )
        .expect("broken-source workspace import references without declaration should exist");

        assert_eq!(references.len(), 4);
        assert!(references.iter().all(|location| location.uri == uri));
        assert_eq!(
            references[0].range.start,
            offset_to_position(&source, nth_offset(&source, "Config", 2)),
        );
        assert_eq!(
            references[1].range.start,
            offset_to_position(&source, nth_offset(&source, "Config", 3)),
        );
        assert_eq!(
            references[2].range.start,
            offset_to_position(&source, nth_offset(&source, "Config", 4)),
        );
        assert_eq!(
            references[3].range.start,
            offset_to_position(&source, nth_offset(&source, "Config", 5)),
        );
    }

    #[test]
    fn workspace_import_references_include_other_broken_consumers_in_workspace() {
        let temp =
            TempDir::new("ql-lsp-workspace-import-source-references-parse-errors-broken-peers");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.measure

pub fn main() -> Int {
    let first = measure(1)
    let second = measure(first)
    return second
"#,
        );
        let task_path = temp.write(
            "workspace/packages/app/src/task.ql",
            r#"
package demo.app

use demo.core.measure

pub fn task() -> Int {
    return measure(2)
"#,
        );
        let jobs_path = temp.write(
            "workspace/packages/jobs/src/job.ql",
            r#"
package demo.jobs

use demo.core.measure

pub fn job() -> Int {
    return measure(3)
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn measure(value: Int) -> Int {
    return value
}

pub fn wrap(value: Int) -> Int {
    return measure(value)
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/jobs", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/jobs/qlang.toml",
            r#"
[package]
name = "jobs"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn measure(value: Int) -> Int
"#,
        );

        let app_source = fs::read_to_string(&app_path).expect("app source should read");
        let task_source = fs::read_to_string(&task_path).expect("task source should read");
        let jobs_source = fs::read_to_string(&jobs_path).expect("jobs source should read");
        let core_source = fs::read_to_string(&core_source_path).expect("core source should read");
        assert!(analyze_source(&app_source).is_err());
        assert!(analyze_source(&task_source).is_err());
        assert!(analyze_source(&jobs_source).is_err());
        let package = package_analysis_for_path(&app_path)
            .expect("package analysis should survive parse errors");
        let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let task_uri = Url::from_file_path(&task_path).expect("task path should convert to URI");
        let jobs_uri = Url::from_file_path(&jobs_path).expect("jobs path should convert to URI");
        let core_uri =
            Url::from_file_path(&core_source_path).expect("core source path should convert to URI");

        let references = workspace_source_references_for_import_in_broken_source(
            &app_uri,
            &app_source,
            &package,
            offset_to_position(&app_source, nth_offset(&app_source, "measure", 2)),
            true,
        )
        .expect("broken-source workspace import references should exist");

        let contains = |uri: &Url, source: &str, needle: &str, occurrence: usize| {
            references.iter().any(|location| {
                location.uri == *uri
                    && location.range.start
                        == offset_to_position(source, nth_offset(source, needle, occurrence))
            })
        };

        assert_eq!(references.len(), 8);
        assert!(contains(&core_uri, &core_source, "measure", 1));
        assert!(contains(&app_uri, &app_source, "measure", 2));
        assert!(contains(&app_uri, &app_source, "measure", 3));
        assert!(contains(&core_uri, &core_source, "measure", 2));
        assert!(contains(&task_uri, &task_source, "measure", 1));
        assert!(contains(&task_uri, &task_source, "measure", 2));
        assert!(contains(&jobs_uri, &jobs_source, "measure", 1));
        assert!(contains(&jobs_uri, &jobs_source, "measure", 2));
    }

    #[test]
    fn workspace_import_references_include_broken_local_dependency_consumers() {
        let temp = TempDir::new(
            "ql-lsp-workspace-import-source-references-parse-errors-broken-local-deps",
        );
        let app_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
package demo.app

use demo.core.measure

pub fn main() -> Int {
    return measure(1)
"#,
        );
        let helper_path = temp.write(
            "workspace/vendor/helper/src/lib.ql",
            r#"
package demo.helper

use demo.core.measure

pub fn helper() -> Int {
    return measure(2)
"#,
        );
        let core_source_path = temp.write(
            "workspace/vendor/core/src/lib.ql",
            r#"
package demo.core

pub fn measure(value: Int) -> Int {
    return value
}

pub fn wrap(value: Int) -> Int {
    return measure(value)
}
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"

[dependencies]
core = { path = "../vendor/core" }
helper = { path = "../vendor/helper" }
"#,
        );
        temp.write(
            "workspace/vendor/helper/qlang.toml",
            r#"
[package]
name = "helper"

[dependencies]
core = { path = "../core" }
"#,
        );
        temp.write(
            "workspace/vendor/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/vendor/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn measure(value: Int) -> Int
"#,
        );

        let app_source = fs::read_to_string(&app_path).expect("app source should read");
        let helper_source = fs::read_to_string(&helper_path).expect("helper source should read");
        let core_source = fs::read_to_string(&core_source_path).expect("core source should read");
        assert!(analyze_source(&app_source).is_err());
        assert!(analyze_source(&helper_source).is_err());
        let package = package_analysis_for_path(&app_path)
            .expect("package analysis should survive parse errors");
        let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");

        let references = workspace_source_references_for_import_in_broken_source(
            &app_uri,
            &app_source,
            &package,
            offset_to_position(&app_source, nth_offset(&app_source, "measure", 2)),
            true,
        )
        .expect("broken-source local dependency import references should exist");

        let contains = |path: &Path, source: &str, needle: &str, occurrence: usize| {
            let path = path
                .canonicalize()
                .expect("expected path should canonicalize");
            references.iter().any(|location| {
                location
                    .uri
                    .to_file_path()
                    .ok()
                    .and_then(|location_path| location_path.canonicalize().ok())
                    == Some(path.clone())
                    && location.range.start
                        == offset_to_position(source, nth_offset(source, needle, occurrence))
            })
        };

        assert_eq!(references.len(), 5);
        assert!(contains(&core_source_path, &core_source, "measure", 1));
        assert!(contains(&app_path, &app_source, "measure", 2));
        assert!(contains(&core_source_path, &core_source, "measure", 2));
        assert!(contains(&helper_path, &helper_source, "measure", 1));
        assert!(contains(&helper_path, &helper_source, "measure", 2));
    }

    #[test]
    fn workspace_import_prepare_rename_survives_parse_errors() {
        let temp = TempDir::new("ql-lsp-workspace-import-prepare-rename-parse-errors");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.exported

pub fn main() -> Int {
    let first = exported(1)
    let second = exported(first)
    return second
"#,
        );
        temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn exported(value: Int) -> Int {
    return value
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn exported(value: Int) -> Int
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&source).is_err());
        let package = package_analysis_for_path(&app_path)
            .expect("package analysis should survive parse errors");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let use_offset = nth_offset(&source, "exported", 2);

        assert_eq!(
            prepare_rename_for_workspace_import_in_broken_source(
                &uri,
                &source,
                &package,
                offset_to_position(&source, use_offset),
            ),
            Some(PrepareRenameResponse::RangeWithPlaceholder {
                range: span_to_range(
                    &source,
                    Span::new(use_offset, use_offset + "exported".len()),
                ),
                placeholder: "exported".to_owned(),
            }),
        );
    }

    #[test]
    fn workspace_root_function_prepare_rename_from_aliased_import_use_survives_parse_errors() {
        let temp = TempDir::new("ql-lsp-workspace-root-function-prepare-rename-parse-errors-alias");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.measure as run

pub fn main() -> Int {
    return run(1)
"#,
        );
        temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn measure(value: Int) -> Int {
    return value
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn measure(value: Int) -> Int
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&source).is_err());
        let package = package_analysis_for_path(&app_path)
            .expect("package analysis should survive parse errors");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let use_offset = nth_offset(&source, "run", 2);

        assert_eq!(
            prepare_rename_for_workspace_source_root_symbol_from_import_in_broken_source(
                &uri,
                &source,
                &package,
                offset_to_position(&source, use_offset),
            ),
            Some(PrepareRenameResponse::RangeWithPlaceholder {
                range: span_to_range(&source, Span::new(use_offset, use_offset + "run".len())),
                placeholder: "run".to_owned(),
            }),
        );
    }

    #[test]
    fn workspace_root_function_prepare_rename_from_import_use_survives_parse_errors_with_open_workspace_source()
     {
        let temp =
            TempDir::new("ql-lsp-workspace-root-function-prepare-rename-parse-errors-open-docs");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.measure as run

pub fn main() -> Int {
    return run(1)
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn helper() -> Int {
    return 0
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn measure(value: Int) -> Int
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&source).is_err());
        let package = package_analysis_for_path(&app_path)
            .expect("package analysis should survive parse errors");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let core_uri =
            Url::from_file_path(&core_source_path).expect("core source path should convert to URI");
        let open_core_source = r#"
package demo.core

pub fn helper() -> Int {
    return 0
}

pub fn measure(value: Int) -> Int {
    return value
}
"#
        .to_owned();
        let use_offset = nth_offset(&source, "run", 2);

        assert_eq!(
            prepare_rename_for_workspace_source_root_symbol_from_import_in_broken_source(
                &uri,
                &source,
                &package,
                offset_to_position(&source, use_offset),
            ),
            None,
            "disk-only prepare rename should miss unsaved workspace source",
        );
        assert_eq!(
            prepare_rename_for_workspace_source_root_symbol_from_import_in_broken_source_with_open_docs(
                &uri,
                &source,
                &package,
                &file_open_documents(vec![
                    (uri.clone(), source.clone()),
                    (core_uri, open_core_source),
                ]),
                offset_to_position(&source, use_offset),
            ),
            Some(PrepareRenameResponse::RangeWithPlaceholder {
                range: span_to_range(&source, Span::new(use_offset, use_offset + "run".len())),
                placeholder: "run".to_owned(),
            }),
        );
    }

    #[test]
    fn workspace_import_rename_survives_parse_errors_and_inserts_alias() {
        let temp = TempDir::new("ql-lsp-workspace-import-rename-parse-errors");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.exported

pub fn main() -> Int {
    let first = exported(1)
    let second = exported(first)
    return second
"#,
        );
        temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn exported(value: Int) -> Int {
    return value
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn exported(value: Int) -> Int
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&source).is_err());
        let package = package_analysis_for_path(&app_path)
            .expect("package analysis should survive parse errors");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let use_offset = nth_offset(&source, "exported", 2);

        let edit = rename_for_workspace_import_in_broken_source(
            &uri,
            &source,
            &package,
            offset_to_position(&source, use_offset),
            "run",
        )
        .expect("rename should validate")
        .expect("broken-source workspace import rename should produce edits");

        assert_workspace_edit(
            edit,
            &uri,
            vec![
                TextEdit::new(
                    span_to_range(
                        &source,
                        Span::new(
                            nth_offset(&source, "exported", 1),
                            nth_offset(&source, "exported", 1) + "exported".len(),
                        ),
                    ),
                    "exported as run".to_owned(),
                ),
                TextEdit::new(
                    span_to_range(
                        &source,
                        Span::new(
                            nth_offset(&source, "exported", 2),
                            nth_offset(&source, "exported", 2) + "exported".len(),
                        ),
                    ),
                    "run".to_owned(),
                ),
                TextEdit::new(
                    span_to_range(
                        &source,
                        Span::new(
                            nth_offset(&source, "exported", 3),
                            nth_offset(&source, "exported", 3) + "exported".len(),
                        ),
                    ),
                    "run".to_owned(),
                ),
            ],
        );

        assert_eq!(
            rename_for_workspace_import_in_broken_source(
                &uri,
                &source,
                &package,
                offset_to_position(&source, use_offset),
                "match",
            ),
            Err(RenameError::Keyword("match".to_owned())),
        );
    }

    #[test]
    fn workspace_import_rename_in_broken_source_prefers_open_workspace_source() {
        let temp = TempDir::new("ql-lsp-workspace-import-rename-parse-errors-open-docs");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.exported

pub fn main() -> Int {
    let first = exported(1)
    return exported(first)
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn helper() -> Int {
    return 0
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn exported(value: Int) -> Int
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&source).is_err());
        let package = package_analysis_for_path(&app_path)
            .expect("package analysis should survive parse errors");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let core_uri =
            Url::from_file_path(&core_source_path).expect("core source path should convert to URI");
        let open_core_source = r#"
package demo.core

pub fn helper() -> Int {
    return 0
}

pub fn exported(value: Int) -> Int {
    return value
}
"#
        .to_owned();
        let use_offset = nth_offset(&source, "exported", 2);

        assert_eq!(
            rename_for_workspace_import_in_broken_source(
                &uri,
                &source,
                &package,
                offset_to_position(&source, use_offset),
                "run",
            )
            .expect("rename should validate"),
            None,
            "disk-only rename should miss unsaved workspace source",
        );

        let edit = rename_for_workspace_import_in_broken_source_with_open_docs(
            &uri,
            &source,
            &package,
            &file_open_documents(vec![
                (uri.clone(), source.clone()),
                (core_uri, open_core_source),
            ]),
            offset_to_position(&source, use_offset),
            "run",
        )
        .expect("rename should validate")
        .expect("broken-source workspace import rename should produce edits");

        assert_workspace_edit(
            edit,
            &uri,
            vec![
                TextEdit::new(
                    span_to_range(
                        &source,
                        Span::new(
                            nth_offset(&source, "exported", 1),
                            nth_offset(&source, "exported", 1) + "exported".len(),
                        ),
                    ),
                    "exported as run".to_owned(),
                ),
                TextEdit::new(
                    span_to_range(
                        &source,
                        Span::new(
                            nth_offset(&source, "exported", 2),
                            nth_offset(&source, "exported", 2) + "exported".len(),
                        ),
                    ),
                    "run".to_owned(),
                ),
                TextEdit::new(
                    span_to_range(
                        &source,
                        Span::new(
                            nth_offset(&source, "exported", 3),
                            nth_offset(&source, "exported", 3) + "exported".len(),
                        ),
                    ),
                    "run".to_owned(),
                ),
            ],
        );
    }

    #[test]
    fn workspace_root_function_rename_from_import_use_survives_parse_errors() {
        let temp = TempDir::new("ql-lsp-workspace-root-function-rename-parse-errors");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.measure

pub fn main() -> Int {
    let first = measure(1)
    let second = measure(first)
    return second
"#,
        );
        let task_path = temp.write(
            "workspace/packages/app/src/task.ql",
            r#"
package demo.app

use demo.core.measure

pub fn task() -> Int {
    return measure(2)
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn measure(value: Int) -> Int {
    return value
}

pub fn wrap(value: Int) -> Int {
    return measure(value)
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn measure(value: Int) -> Int
"#,
        );

        let app_source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&app_source).is_err());
        let package = package_analysis_for_path(&app_path)
            .expect("package analysis should survive parse errors");
        let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let task_source = fs::read_to_string(&task_path).expect("task source should read");
        let task_uri = Url::from_file_path(&task_path).expect("task path should convert to URI");
        let core_source = fs::read_to_string(&core_source_path).expect("core source should read");
        let core_uri =
            Url::from_file_path(&core_source_path).expect("core source path should convert to URI");

        let edit =
            rename_for_workspace_source_root_symbol_from_import_in_broken_source_with_open_docs(
                &app_uri,
                &app_source,
                &package,
                &file_open_documents(vec![
                    (app_uri.clone(), app_source.clone()),
                    (task_uri.clone(), task_source.clone()),
                    (core_uri.clone(), core_source.clone()),
                ]),
                offset_to_position(&app_source, nth_offset(&app_source, "measure", 2)),
                "score",
            )
            .expect("rename should succeed")
            .expect("rename should return workspace edits");

        assert_workspace_edit_changes(
            edit,
            vec![
                (
                    app_uri,
                    vec![
                        TextEdit::new(
                            span_to_range(
                                &app_source,
                                Span::new(
                                    nth_offset(&app_source, "measure", 1),
                                    nth_offset(&app_source, "measure", 1) + "measure".len(),
                                ),
                            ),
                            "score".to_owned(),
                        ),
                        TextEdit::new(
                            span_to_range(
                                &app_source,
                                Span::new(
                                    nth_offset(&app_source, "measure", 2),
                                    nth_offset(&app_source, "measure", 2) + "measure".len(),
                                ),
                            ),
                            "score".to_owned(),
                        ),
                        TextEdit::new(
                            span_to_range(
                                &app_source,
                                Span::new(
                                    nth_offset(&app_source, "measure", 3),
                                    nth_offset(&app_source, "measure", 3) + "measure".len(),
                                ),
                            ),
                            "score".to_owned(),
                        ),
                    ],
                ),
                (
                    task_uri,
                    vec![
                        TextEdit::new(
                            span_to_range(
                                &task_source,
                                Span::new(
                                    nth_offset(&task_source, "measure", 1),
                                    nth_offset(&task_source, "measure", 1) + "measure".len(),
                                ),
                            ),
                            "score".to_owned(),
                        ),
                        TextEdit::new(
                            span_to_range(
                                &task_source,
                                Span::new(
                                    nth_offset(&task_source, "measure", 2),
                                    nth_offset(&task_source, "measure", 2) + "measure".len(),
                                ),
                            ),
                            "score".to_owned(),
                        ),
                    ],
                ),
                (
                    core_uri,
                    vec![
                        TextEdit::new(
                            span_to_range(
                                &core_source,
                                Span::new(
                                    nth_offset(&core_source, "measure", 1),
                                    nth_offset(&core_source, "measure", 1) + "measure".len(),
                                ),
                            ),
                            "score".to_owned(),
                        ),
                        TextEdit::new(
                            span_to_range(
                                &core_source,
                                Span::new(
                                    nth_offset(&core_source, "measure", 2),
                                    nth_offset(&core_source, "measure", 2) + "measure".len(),
                                ),
                            ),
                            "score".to_owned(),
                        ),
                    ],
                ),
            ],
        );
    }

    #[test]
    fn workspace_root_function_rename_from_import_use_updates_other_broken_consumers_in_workspace()
    {
        let temp = TempDir::new("ql-lsp-workspace-root-function-rename-parse-errors-broken-peers");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.measure

pub fn main() -> Int {
    let first = measure(1)
    let second = measure(first)
    return second
"#,
        );
        let task_path = temp.write(
            "workspace/packages/app/src/task.ql",
            r#"
package demo.app

use demo.core.measure

pub fn task() -> Int {
    return measure(2)
"#,
        );
        let jobs_path = temp.write(
            "workspace/packages/jobs/src/job.ql",
            r#"
package demo.jobs

use demo.core.measure

pub fn job() -> Int {
    return measure(3)
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn measure(value: Int) -> Int {
    return value
}

pub fn wrap(value: Int) -> Int {
    return measure(value)
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/jobs", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/jobs/qlang.toml",
            r#"
[package]
name = "jobs"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn measure(value: Int) -> Int
"#,
        );

        let app_source = fs::read_to_string(&app_path).expect("app source should read");
        let task_source = fs::read_to_string(&task_path).expect("task source should read");
        let jobs_source = fs::read_to_string(&jobs_path).expect("jobs source should read");
        assert!(analyze_source(&app_source).is_err());
        assert!(analyze_source(&task_source).is_err());
        assert!(analyze_source(&jobs_source).is_err());
        let package = package_analysis_for_path(&app_path)
            .expect("package analysis should survive parse errors");
        let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let task_uri = Url::from_file_path(&task_path).expect("task path should convert to URI");
        let jobs_uri = Url::from_file_path(&jobs_path).expect("jobs path should convert to URI");
        let core_source = fs::read_to_string(&core_source_path).expect("core source should read");
        let core_uri =
            Url::from_file_path(&core_source_path).expect("core source path should convert to URI");

        let edit =
            rename_for_workspace_source_root_symbol_from_import_in_broken_source_with_open_docs(
                &app_uri,
                &app_source,
                &package,
                &file_open_documents(vec![
                    (app_uri.clone(), app_source.clone()),
                    (task_uri.clone(), task_source.clone()),
                    (jobs_uri.clone(), jobs_source.clone()),
                    (core_uri.clone(), core_source.clone()),
                ]),
                offset_to_position(&app_source, nth_offset(&app_source, "measure", 2)),
                "score",
            )
            .expect("rename should succeed")
            .expect("rename should return workspace edits");

        assert_workspace_edit_changes(
            edit,
            vec![
                (
                    app_uri,
                    vec![
                        TextEdit::new(
                            span_to_range(
                                &app_source,
                                Span::new(
                                    nth_offset(&app_source, "measure", 1),
                                    nth_offset(&app_source, "measure", 1) + "measure".len(),
                                ),
                            ),
                            "score".to_owned(),
                        ),
                        TextEdit::new(
                            span_to_range(
                                &app_source,
                                Span::new(
                                    nth_offset(&app_source, "measure", 2),
                                    nth_offset(&app_source, "measure", 2) + "measure".len(),
                                ),
                            ),
                            "score".to_owned(),
                        ),
                        TextEdit::new(
                            span_to_range(
                                &app_source,
                                Span::new(
                                    nth_offset(&app_source, "measure", 3),
                                    nth_offset(&app_source, "measure", 3) + "measure".len(),
                                ),
                            ),
                            "score".to_owned(),
                        ),
                    ],
                ),
                (
                    task_uri,
                    vec![
                        TextEdit::new(
                            span_to_range(
                                &task_source,
                                Span::new(
                                    nth_offset(&task_source, "measure", 1),
                                    nth_offset(&task_source, "measure", 1) + "measure".len(),
                                ),
                            ),
                            "score".to_owned(),
                        ),
                        TextEdit::new(
                            span_to_range(
                                &task_source,
                                Span::new(
                                    nth_offset(&task_source, "measure", 2),
                                    nth_offset(&task_source, "measure", 2) + "measure".len(),
                                ),
                            ),
                            "score".to_owned(),
                        ),
                    ],
                ),
                (
                    jobs_uri,
                    vec![
                        TextEdit::new(
                            span_to_range(
                                &jobs_source,
                                Span::new(
                                    nth_offset(&jobs_source, "measure", 1),
                                    nth_offset(&jobs_source, "measure", 1) + "measure".len(),
                                ),
                            ),
                            "score".to_owned(),
                        ),
                        TextEdit::new(
                            span_to_range(
                                &jobs_source,
                                Span::new(
                                    nth_offset(&jobs_source, "measure", 2),
                                    nth_offset(&jobs_source, "measure", 2) + "measure".len(),
                                ),
                            ),
                            "score".to_owned(),
                        ),
                    ],
                ),
                (
                    core_uri,
                    vec![
                        TextEdit::new(
                            span_to_range(
                                &core_source,
                                Span::new(
                                    nth_offset(&core_source, "measure", 1),
                                    nth_offset(&core_source, "measure", 1) + "measure".len(),
                                ),
                            ),
                            "score".to_owned(),
                        ),
                        TextEdit::new(
                            span_to_range(
                                &core_source,
                                Span::new(
                                    nth_offset(&core_source, "measure", 2),
                                    nth_offset(&core_source, "measure", 2) + "measure".len(),
                                ),
                            ),
                            "score".to_owned(),
                        ),
                    ],
                ),
            ],
        );
    }

    #[test]
    fn workspace_root_function_rename_from_import_use_updates_broken_local_dependency_consumers() {
        let temp =
            TempDir::new("ql-lsp-workspace-root-function-rename-parse-errors-broken-local-deps");
        let app_path = temp.write(
            "workspace/app/src/main.ql",
            r#"
package demo.app

use demo.core.measure

pub fn main() -> Int {
    return measure(1)
"#,
        );
        let helper_path = temp.write(
            "workspace/vendor/helper/src/lib.ql",
            r#"
package demo.helper

use demo.core.measure

pub fn helper() -> Int {
    return measure(2)
"#,
        );
        let core_source_path = temp.write(
            "workspace/vendor/core/src/lib.ql",
            r#"
package demo.core

pub fn measure(value: Int) -> Int {
    return value
}

pub fn wrap(value: Int) -> Int {
    return measure(value)
}
"#,
        );
        temp.write(
            "workspace/app/qlang.toml",
            r#"
[package]
name = "app"

[dependencies]
core = { path = "../vendor/core" }
helper = { path = "../vendor/helper" }
"#,
        );
        temp.write(
            "workspace/vendor/helper/qlang.toml",
            r#"
[package]
name = "helper"

[dependencies]
core = { path = "../core" }
"#,
        );
        temp.write(
            "workspace/vendor/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/vendor/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn measure(value: Int) -> Int
"#,
        );

        let app_source = fs::read_to_string(&app_path).expect("app source should read");
        let helper_source = fs::read_to_string(&helper_path).expect("helper source should read");
        assert!(analyze_source(&app_source).is_err());
        assert!(analyze_source(&helper_source).is_err());
        let package = package_analysis_for_path(&app_path)
            .expect("package analysis should survive parse errors");
        let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let helper_uri =
            Url::from_file_path(&helper_path).expect("helper path should convert to URI");
        let core_source = fs::read_to_string(&core_source_path).expect("core source should read");
        let core_uri =
            Url::from_file_path(&core_source_path).expect("core source path should convert to URI");

        let edit =
            rename_for_workspace_source_root_symbol_from_import_in_broken_source_with_open_docs(
                &app_uri,
                &app_source,
                &package,
                &file_open_documents(vec![
                    (app_uri.clone(), app_source.clone()),
                    (helper_uri.clone(), helper_source.clone()),
                    (core_uri.clone(), core_source.clone()),
                ]),
                offset_to_position(&app_source, nth_offset(&app_source, "measure", 2)),
                "score",
            )
            .expect("rename should succeed")
            .expect("rename should return workspace edits");

        assert_workspace_edit_changes(
            edit,
            vec![
                (
                    app_uri,
                    vec![
                        TextEdit::new(
                            span_to_range(
                                &app_source,
                                Span::new(
                                    nth_offset(&app_source, "measure", 1),
                                    nth_offset(&app_source, "measure", 1) + "measure".len(),
                                ),
                            ),
                            "score".to_owned(),
                        ),
                        TextEdit::new(
                            span_to_range(
                                &app_source,
                                Span::new(
                                    nth_offset(&app_source, "measure", 2),
                                    nth_offset(&app_source, "measure", 2) + "measure".len(),
                                ),
                            ),
                            "score".to_owned(),
                        ),
                    ],
                ),
                (
                    helper_uri,
                    vec![
                        TextEdit::new(
                            span_to_range(
                                &helper_source,
                                Span::new(
                                    nth_offset(&helper_source, "measure", 1),
                                    nth_offset(&helper_source, "measure", 1) + "measure".len(),
                                ),
                            ),
                            "score".to_owned(),
                        ),
                        TextEdit::new(
                            span_to_range(
                                &helper_source,
                                Span::new(
                                    nth_offset(&helper_source, "measure", 2),
                                    nth_offset(&helper_source, "measure", 2) + "measure".len(),
                                ),
                            ),
                            "score".to_owned(),
                        ),
                    ],
                ),
                (
                    core_uri,
                    vec![
                        TextEdit::new(
                            span_to_range(
                                &core_source,
                                Span::new(
                                    nth_offset(&core_source, "measure", 1),
                                    nth_offset(&core_source, "measure", 1) + "measure".len(),
                                ),
                            ),
                            "score".to_owned(),
                        ),
                        TextEdit::new(
                            span_to_range(
                                &core_source,
                                Span::new(
                                    nth_offset(&core_source, "measure", 2),
                                    nth_offset(&core_source, "measure", 2) + "measure".len(),
                                ),
                            ),
                            "score".to_owned(),
                        ),
                    ],
                ),
            ],
        );
    }

    #[test]
    fn workspace_root_struct_rename_from_aliased_import_use_survives_parse_errors() {
        let temp = TempDir::new("ql-lsp-workspace-root-struct-rename-parse-errors-alias");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.Config as Cfg

pub fn main(config: Cfg) -> Int {
    return config.value
"#,
        );
        let task_path = temp.write(
            "workspace/packages/app/src/task.ql",
            r#"
package demo.app

use demo.core.Config

pub fn task(config: Config) -> Config {
    return config
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub struct Config {
    value: Int,
}

pub fn copy(config: Config) -> Config {
    return config
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub struct Config {
    value: Int,
}
"#,
        );

        let app_source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&app_source).is_err());
        let package = package_analysis_for_path(&app_path)
            .expect("package analysis should survive parse errors");
        let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let task_source = fs::read_to_string(&task_path).expect("task source should read");
        let task_uri = Url::from_file_path(&task_path).expect("task path should convert to URI");
        let core_source = fs::read_to_string(&core_source_path).expect("core source should read");
        let core_uri =
            Url::from_file_path(&core_source_path).expect("core source path should convert to URI");

        let edit =
            rename_for_workspace_source_root_symbol_from_import_in_broken_source_with_open_docs(
                &app_uri,
                &app_source,
                &package,
                &file_open_documents(vec![
                    (app_uri.clone(), app_source.clone()),
                    (task_uri.clone(), task_source.clone()),
                    (core_uri.clone(), core_source.clone()),
                ]),
                offset_to_position(&app_source, nth_offset(&app_source, "Cfg", 2)),
                "Settings",
            )
            .expect("rename should succeed")
            .expect("rename should return workspace edits");

        assert_workspace_edit_changes(
            edit,
            vec![
                (
                    app_uri,
                    vec![TextEdit::new(
                        span_to_range(
                            &app_source,
                            Span::new(
                                nth_offset(&app_source, "Config", 1),
                                nth_offset(&app_source, "Config", 1) + "Config".len(),
                            ),
                        ),
                        "Settings".to_owned(),
                    )],
                ),
                (
                    task_uri,
                    vec![
                        TextEdit::new(
                            span_to_range(
                                &task_source,
                                Span::new(
                                    nth_offset(&task_source, "Config", 1),
                                    nth_offset(&task_source, "Config", 1) + "Config".len(),
                                ),
                            ),
                            "Settings".to_owned(),
                        ),
                        TextEdit::new(
                            span_to_range(
                                &task_source,
                                Span::new(
                                    nth_offset(&task_source, "Config", 2),
                                    nth_offset(&task_source, "Config", 2) + "Config".len(),
                                ),
                            ),
                            "Settings".to_owned(),
                        ),
                        TextEdit::new(
                            span_to_range(
                                &task_source,
                                Span::new(
                                    nth_offset(&task_source, "Config", 3),
                                    nth_offset(&task_source, "Config", 3) + "Config".len(),
                                ),
                            ),
                            "Settings".to_owned(),
                        ),
                    ],
                ),
                (
                    core_uri,
                    vec![
                        TextEdit::new(
                            span_to_range(
                                &core_source,
                                Span::new(
                                    nth_offset(&core_source, "Config", 1),
                                    nth_offset(&core_source, "Config", 1) + "Config".len(),
                                ),
                            ),
                            "Settings".to_owned(),
                        ),
                        TextEdit::new(
                            span_to_range(
                                &core_source,
                                Span::new(
                                    nth_offset(&core_source, "Config", 2),
                                    nth_offset(&core_source, "Config", 2) + "Config".len(),
                                ),
                            ),
                            "Settings".to_owned(),
                        ),
                        TextEdit::new(
                            span_to_range(
                                &core_source,
                                Span::new(
                                    nth_offset(&core_source, "Config", 3),
                                    nth_offset(&core_source, "Config", 3) + "Config".len(),
                                ),
                            ),
                            "Settings".to_owned(),
                        ),
                    ],
                ),
            ],
        );
    }

    #[test]
    fn workspace_dependency_definitions_prefer_workspace_member_source_over_interface_artifact() {
        let temp = TempDir::new("ql-lsp-workspace-dependency-source-definitions");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.Command as Cmd
use demo.core.Config as Cfg
use demo.core.exported as run

pub fn main(config: Cfg) -> Int {
    let built = Cfg { value: 1, limit: 2 }
    let command = Cmd.Retry(1)
    let result = config.ping()
    return run(result) + built.value + command.unwrap_or(0)
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub enum Command {
    Retry(Int),
}

impl Command {
    pub fn unwrap_or(self, fallback: Int) -> Int {
        match self {
            Command.Retry(value) => value,
        }
    }
}

pub struct Config {
    value: Int,
    limit: Int,
}

impl Config {
    pub fn ping(self) -> Int {
        return self.value + self.limit
    }

    pub fn use_ping(self) -> Int {
        return self.ping()
    }
}

pub fn exported(value: Int) -> Int {
    return value
}

pub fn wrapper(value: Int) -> Int {
    return exported(value)
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub enum Command {
    Retry(Int),
}

impl Command {
    pub fn unwrap_or(self, fallback: Int) -> Int
}

pub struct Config {
    value: Int,
    limit: Int,
}

impl Config {
    pub fn ping(self) -> Int
}

pub fn exported(value: Int) -> Int
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        let analysis = analyze_source(&source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let core_source =
            fs::read_to_string(&core_source_path).expect("core source should read for assertions");

        for (needle, occurrence, expected_symbol, expected_occurrence) in [
            ("run", 2usize, "exported", 1usize),
            ("Retry", 1usize, "Retry", 1usize),
            ("ping", 1usize, "ping", 1usize),
            ("value", 2usize, "value", 3usize),
        ] {
            let definition = workspace_source_definition_for_dependency(
                &uri,
                &source,
                Some(&analysis),
                &package,
                offset_to_position(&source, nth_offset(&source, needle, occurrence)),
            )
            .unwrap_or_else(|| panic!("workspace dependency definition should exist for {needle}"));

            let GotoDefinitionResponse::Scalar(location) = definition else {
                panic!("workspace dependency definition should resolve to one location")
            };
            assert_eq!(
                location
                    .uri
                    .to_file_path()
                    .expect("definition URI should convert to a file path")
                    .canonicalize()
                    .expect("definition path should canonicalize"),
                core_source_path
                    .canonicalize()
                    .expect("core source path should canonicalize"),
            );
            assert_eq!(
                location.range.start,
                offset_to_position(
                    &core_source,
                    nth_offset(&core_source, expected_symbol, expected_occurrence)
                ),
            );
        }
    }

    #[test]
    fn same_named_local_dependency_semantic_tokens_survive_parse_errors_for_members() {
        let temp = TempDir::new("ql-lsp-same-named-local-dependency-semantic-tokens-parse-errors");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.shared.alpha.build as build
use demo.shared.beta.build as other

pub fn main() -> Int {
    let current = build()
    return current.ping() + current.value + other().tick(
"#,
        );
        temp.write(
            "workspace/vendor/alpha/src/lib.ql",
            r#"
package demo.shared.alpha

pub struct Config {
    value: Int,
}

pub fn build() -> Config {
    return Config { value: 1 }
}

impl Config {
    pub fn ping(self) -> Int {
        return self.value
    }
}
"#,
        );
        temp.write(
            "workspace/vendor/beta/src/lib.ql",
            r#"
package demo.shared.beta

pub struct Config {
    amount: Int,
}

pub fn build() -> Config {
    return Config { amount: 2 }
}

impl Config {
    pub fn tick(self) -> Int {
        return self.amount
    }
}
"#,
        );
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

pub struct Config {
    value: Int,
}

pub fn build() -> Config

impl Config {
    pub fn ping(self) -> Int
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

pub struct Config {
    amount: Int,
}

pub fn build() -> Config

impl Config {
    pub fn tick(self) -> Int
}
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&source).is_err());
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");

        let SemanticTokensResult::Tokens(tokens) =
            semantic_tokens_for_workspace_dependency_fallback(&uri, &source, &package)
        else {
            panic!("expected full semantic tokens")
        };
        let decoded = decode_semantic_tokens(&tokens.data);
        let legend = semantic_tokens_legend();
        let function_type = legend
            .token_types
            .iter()
            .position(|token_type| *token_type == SemanticTokenType::FUNCTION)
            .expect("function legend entry should exist") as u32;
        let variable_type = legend
            .token_types
            .iter()
            .position(|token_type| *token_type == SemanticTokenType::VARIABLE)
            .expect("variable legend entry should exist") as u32;
        let property_type = legend
            .token_types
            .iter()
            .position(|token_type| *token_type == SemanticTokenType::PROPERTY)
            .expect("property legend entry should exist") as u32;
        let method_type = legend
            .token_types
            .iter()
            .position(|token_type| *token_type == SemanticTokenType::METHOD)
            .expect("method legend entry should exist") as u32;

        for (needle, occurrence, token_type) in [
            ("build", 1usize, function_type),
            ("other", 2usize, function_type),
            ("current", 1usize, variable_type),
            ("current", 2usize, variable_type),
            ("ping", 1usize, method_type),
            ("value", 1usize, property_type),
            ("tick", 1usize, method_type),
        ] {
            let span = Span::new(
                nth_offset(&source, needle, occurrence),
                nth_offset(&source, needle, occurrence) + needle.len(),
            );
            let range = span_to_range(&source, span);
            assert!(
                decoded.contains(&(
                    range.start.line,
                    range.start.character,
                    range.end.character - range.start.character,
                    token_type,
                )),
                "expected semantic token for {needle} occurrence {occurrence}",
            );
        }
    }

    #[test]
    fn workspace_dependency_value_queries_survive_parse_errors_and_prefer_workspace_member_source()
    {
        let temp = TempDir::new("ql-lsp-workspace-dependency-value-source-queries-parse-errors");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.Config as Cfg

pub fn main(config: Cfg) -> Int {
    let current = config
    return current.value
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub struct Config {
    value: Int,
    extra: Int,
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub struct Config {
    value: Int,
}
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&source).is_err());
        let package = package_analysis_for_path(&app_path)
            .expect("package analysis should survive parse errors");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let current_position = offset_to_position(&source, nth_offset(&source, "current", 2));

        let hover =
            workspace_source_hover_for_dependency(&uri, &source, None, &package, current_position)
                .expect("broken-source workspace dependency hover should exist");
        let HoverContents::Markup(markup) = hover.contents else {
            panic!("hover should use markdown")
        };
        assert!(markup.value.contains("struct Config"));

        let definition = workspace_source_definition_for_dependency(
            &uri,
            &source,
            None,
            &package,
            current_position,
        )
        .expect("broken-source workspace dependency definition should exist");
        let GotoDefinitionResponse::Scalar(definition_location) = definition else {
            panic!("workspace dependency definition should resolve to one location")
        };
        assert_eq!(
            definition_location
                .uri
                .to_file_path()
                .expect("definition URI should convert to a file path")
                .canonicalize()
                .expect("definition path should canonicalize"),
            core_source_path
                .canonicalize()
                .expect("core source path should canonicalize"),
        );

        let type_definition = workspace_source_type_definition_for_dependency(
            &uri,
            &source,
            None,
            &package,
            current_position,
        )
        .expect("broken-source workspace dependency type definition should exist");
        let GotoTypeDefinitionResponse::Scalar(type_location) = type_definition else {
            panic!("workspace dependency type definition should resolve to one location")
        };
        assert_eq!(
            type_location
                .uri
                .to_file_path()
                .expect("type definition URI should convert to a file path")
                .canonicalize()
                .expect("type definition path should canonicalize"),
            core_source_path
                .canonicalize()
                .expect("core source path should canonicalize"),
        );
    }

    #[test]
    fn workspace_dependency_type_definitions_prefer_workspace_member_source_over_interface_artifact()
     {
        let temp = TempDir::new("ql-lsp-workspace-dependency-source-type-definitions");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.Command as Cmd
use demo.core.Config as Cfg
use demo.core.Holder as Hold

pub fn main(config: Cfg) -> Int {
    let built = Cfg { value: 1, limit: 2 }
    let holder = Hold { child: config.clone_self() }
    let command = Cmd.Retry(1)
    return holder.child.value + built.value + command.unwrap_or(0)
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub enum Command {
    Retry(Int),
}

pub struct Config {
    value: Int,
    limit: Int,
}

pub struct Holder {
    child: Config,
}

impl Command {
    pub fn unwrap_or(self, fallback: Int) -> Int {
        match self {
            Command.Retry(value) => value,
        }
    }
}

impl Config {
    pub fn clone_self(self) -> Config {
        return self
    }
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub enum Command {
    Retry(Int),
}

pub struct Config {
    value: Int,
    limit: Int,
}

pub struct Holder {
    child: Config,
}

impl Command {
    pub fn unwrap_or(self, fallback: Int) -> Int
}

impl Config {
    pub fn clone_self(self) -> Config
}
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        let analysis = analyze_source(&source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let core_source =
            fs::read_to_string(&core_source_path).expect("core source should read for assertions");

        for (needle, occurrence, expected_symbol, expected_occurrence) in [
            ("Cfg", 2usize, "Config", 1usize),
            ("built", 2usize, "Config", 1usize),
            ("clone_self", 1usize, "Config", 1usize),
            ("Retry", 1usize, "Command", 1usize),
            ("child", 2usize, "Config", 1usize),
        ] {
            let definition = workspace_source_type_definition_for_dependency(
                &uri,
                &source,
                Some(&analysis),
                &package,
                offset_to_position(&source, nth_offset(&source, needle, occurrence)),
            )
            .unwrap_or_else(|| {
                panic!("workspace dependency type definition should exist for {needle}")
            });

            let GotoTypeDefinitionResponse::Scalar(location) = definition else {
                panic!("workspace dependency type definition should resolve to one location")
            };
            assert_eq!(
                location
                    .uri
                    .to_file_path()
                    .expect("definition URI should convert to a file path")
                    .canonicalize()
                    .expect("definition path should canonicalize"),
                core_source_path
                    .canonicalize()
                    .expect("core source path should canonicalize"),
            );
            assert_eq!(
                location.range.start,
                offset_to_position(
                    &core_source,
                    nth_offset(&core_source, expected_symbol, expected_occurrence)
                ),
            );
        }
    }

    #[test]
    fn workspace_dependency_references_prefer_workspace_member_source_over_interface_artifact() {
        let temp = TempDir::new("ql-lsp-workspace-dependency-source-references");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.Command as Cmd
use demo.core.Config as Cfg
use demo.core.exported as run

pub fn main(config: Cfg) -> Int {
    let built = Cfg { value: 1, limit: 2 }
    let command = Cmd.Retry(1)
    let result = config.ping()
    return run(result) + built.value + command.unwrap_or(0)
}
"#,
        );
        let task_path = temp.write(
            "workspace/packages/app/src/task.ql",
            r#"
package demo.app

use demo.core.Command as OtherCmd
use demo.core.Config as OtherCfg
use demo.core.exported as call

pub fn task(config: OtherCfg) -> Int {
    let command = OtherCmd.Retry(2)
    let result = config.ping()
    return call(result) + config.value + command.unwrap_or(0)
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub enum Command {
    Retry(Int),
}

impl Command {
    pub fn unwrap_or(self, fallback: Int) -> Int {
        match self {
            Command.Retry(value) => value,
        }
    }
}

pub struct Config {
    value: Int,
    limit: Int,
}

impl Config {
    pub fn ping(self) -> Int {
        return self.value + self.limit
    }

    pub fn use_ping(self) -> Int {
        return self.ping()
    }
}

pub fn exported(value: Int) -> Int {
    return value
}

pub fn wrapper(value: Int) -> Int {
    return exported(value)
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub enum Command {
    Retry(Int),
}

impl Command {
    pub fn unwrap_or(self, fallback: Int) -> Int
}

pub struct Config {
    value: Int,
    limit: Int,
}

impl Config {
    pub fn ping(self) -> Int
}

pub fn exported(value: Int) -> Int
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        let analysis = analyze_source(&source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let core_source =
            fs::read_to_string(&core_source_path).expect("core source should read for assertions");
        let task_source = fs::read_to_string(&task_path).expect("task source should read");
        let task_uri = Url::from_file_path(&task_path).expect("task path should convert to URI");

        for (
            needle,
            occurrence,
            expected_symbol,
            expected_occurrence,
            expected_count,
            local_occurrences,
            source_occurrence,
            task_needle,
            task_occurrences,
        ) in [
            (
                "Retry",
                1usize,
                "Retry",
                1usize,
                4usize,
                vec![1usize],
                Some(2usize),
                "Retry",
                vec![1usize],
            ),
            (
                "ping",
                1usize,
                "ping",
                1usize,
                4usize,
                vec![1usize],
                Some(3usize),
                "ping",
                vec![1usize],
            ),
            (
                "value",
                2usize,
                "value",
                3usize,
                5usize,
                vec![1usize, 2usize],
                Some(4usize),
                "value",
                vec![1usize],
            ),
            (
                "run",
                2usize,
                "exported",
                1usize,
                6usize,
                vec![1usize, 2usize],
                Some(2usize),
                "call",
                vec![1usize, 2usize],
            ),
        ] {
            let references = workspace_source_references_for_dependency(
                &uri,
                &source,
                Some(&analysis),
                &package,
                offset_to_position(&source, nth_offset(&source, needle, occurrence)),
                true,
            )
            .unwrap_or_else(|| panic!("workspace dependency references should exist for {needle}"));

            assert_eq!(references.len(), expected_count, "{needle}");
            assert_eq!(
                references[0]
                    .uri
                    .to_file_path()
                    .expect("definition URI should convert to a file path")
                    .canonicalize()
                    .expect("definition path should canonicalize"),
                core_source_path
                    .canonicalize()
                    .expect("core source path should canonicalize"),
            );
            assert_eq!(
                references[0].range.start,
                offset_to_position(
                    &core_source,
                    nth_offset(&core_source, expected_symbol, expected_occurrence)
                ),
            );

            for (reference, local_occurrence) in
                references[1..].iter().zip(local_occurrences.into_iter())
            {
                assert_eq!(reference.uri, uri);
                assert_eq!(
                    reference.range.start,
                    offset_to_position(&source, nth_offset(&source, needle, local_occurrence)),
                );
            }

            if let Some(source_occurrence) = source_occurrence {
                assert!(
                    references.iter().any(|reference| {
                        reference
                            .uri
                            .to_file_path()
                            .ok()
                            .and_then(|path| path.canonicalize().ok())
                            == core_source_path.canonicalize().ok()
                            && reference.range.start
                                == offset_to_position(
                                    &core_source,
                                    nth_offset(&core_source, expected_symbol, source_occurrence),
                                )
                    }),
                    "{needle} should include workspace source occurrence",
                );
            }

            for task_occurrence in task_occurrences {
                assert!(
                    references.iter().any(|reference| {
                        reference.uri == task_uri
                            && reference.range.start
                                == offset_to_position(
                                    &task_source,
                                    nth_offset(&task_source, task_needle, task_occurrence),
                                )
                    }),
                    "{needle} should include task file occurrence",
                );
            }
        }
    }

    #[test]
    fn workspace_dependency_method_rename_updates_workspace_source_and_other_files() {
        let temp = TempDir::new("ql-lsp-workspace-dependency-method-rename");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.Config as Cfg

pub fn main(config: Cfg) -> Int {
    return config.ping()
}
"#,
        );
        let task_path = temp.write(
            "workspace/packages/app/src/task.ql",
            r#"
package demo.app

use demo.core.Config as OtherCfg

pub fn task(config: OtherCfg) -> Int {
    return config.ping()
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub struct Config {
    value: Int,
}

impl Config {
    pub fn ping(self) -> Int {
        return self.value
    }

    pub fn repeat(self) -> Int {
        return self.ping()
    }
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub struct Config {
    value: Int,
}

impl Config {
    pub fn ping(self) -> Int
}
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        let analysis = analyze_source(&source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let task_source = fs::read_to_string(&task_path).expect("task source should read");
        let task_uri = Url::from_file_path(&task_path).expect("task path should convert to URI");
        let core_source = fs::read_to_string(&core_source_path).expect("core source should read");
        let core_uri =
            Url::from_file_path(&core_source_path).expect("core source path should convert to URI");
        let open_docs = file_open_documents(vec![
            (uri.clone(), source.clone()),
            (task_uri.clone(), task_source.clone()),
            (core_uri.clone(), core_source.clone()),
        ]);

        let edit = rename_for_workspace_source_dependency_with_open_docs(
            &uri,
            &source,
            Some(&analysis),
            &package,
            &open_docs,
            offset_to_position(&source, nth_offset(&source, "ping", 1)),
            "probe",
        )
        .expect("rename should succeed")
        .expect("rename should return workspace edits");

        assert_workspace_edit_changes(
            edit,
            vec![
                (
                    uri.clone(),
                    vec![TextEdit::new(
                        span_to_range(
                            &source,
                            Span::new(
                                nth_offset(&source, "ping", 1),
                                nth_offset(&source, "ping", 1) + "ping".len(),
                            ),
                        ),
                        "probe".to_owned(),
                    )],
                ),
                (
                    task_uri.clone(),
                    vec![TextEdit::new(
                        span_to_range(
                            &task_source,
                            Span::new(
                                nth_offset(&task_source, "ping", 1),
                                nth_offset(&task_source, "ping", 1) + "ping".len(),
                            ),
                        ),
                        "probe".to_owned(),
                    )],
                ),
                (
                    core_uri,
                    vec![
                        TextEdit::new(
                            span_to_range(
                                &core_source,
                                Span::new(
                                    nth_offset(&core_source, "ping", 1),
                                    nth_offset(&core_source, "ping", 1) + "ping".len(),
                                ),
                            ),
                            "probe".to_owned(),
                        ),
                        TextEdit::new(
                            span_to_range(
                                &core_source,
                                Span::new(
                                    nth_offset(&core_source, "ping", 2),
                                    nth_offset(&core_source, "ping", 2) + "ping".len(),
                                ),
                            ),
                            "probe".to_owned(),
                        ),
                    ],
                ),
            ],
        );
    }

    #[test]
    fn workspace_dependency_field_rename_survives_parse_errors_and_updates_workspace_edits() {
        let temp = TempDir::new("ql-lsp-workspace-dependency-field-rename-parse-errors");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.Config as Cfg

pub fn main(config: Cfg) -> Int {
    let result = config.value
    return result + config.
"#,
        );
        let task_path = temp.write(
            "workspace/packages/app/src/task.ql",
            r#"
package demo.app

use demo.core.Config as OtherCfg

pub fn task(config: OtherCfg) -> Int {
    return config.value
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub struct Config {
    value: Int,
    limit: Int,
}

impl Config {
    pub fn total(self) -> Int {
        return self.value + self.limit
    }
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub struct Config {
    value: Int,
    limit: Int,
}

impl Config {
    pub fn total(self) -> Int
}
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&source).is_err());
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should survive errors");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let task_source = fs::read_to_string(&task_path).expect("task source should read");
        let task_uri = Url::from_file_path(&task_path).expect("task path should convert to URI");
        let core_source = fs::read_to_string(&core_source_path).expect("core source should read");
        let core_uri =
            Url::from_file_path(&core_source_path).expect("core source path should convert to URI");
        let open_docs = file_open_documents(vec![
            (uri.clone(), source.clone()),
            (task_uri.clone(), task_source.clone()),
            (core_uri.clone(), core_source.clone()),
        ]);

        let edit = rename_for_workspace_source_dependency_with_open_docs(
            &uri,
            &source,
            None,
            &package,
            &open_docs,
            offset_to_position(&source, nth_offset(&source, "value", 1)),
            "count",
        )
        .expect("rename should succeed")
        .expect("rename should return workspace edits");

        assert_workspace_edit_changes(
            edit,
            vec![
                (
                    uri.clone(),
                    vec![TextEdit::new(
                        span_to_range(
                            &source,
                            Span::new(
                                nth_offset(&source, "value", 1),
                                nth_offset(&source, "value", 1) + "value".len(),
                            ),
                        ),
                        "count".to_owned(),
                    )],
                ),
                (
                    task_uri.clone(),
                    vec![TextEdit::new(
                        span_to_range(
                            &task_source,
                            Span::new(
                                nth_offset(&task_source, "value", 1),
                                nth_offset(&task_source, "value", 1) + "value".len(),
                            ),
                        ),
                        "count".to_owned(),
                    )],
                ),
                (
                    core_uri,
                    vec![
                        TextEdit::new(
                            span_to_range(
                                &core_source,
                                Span::new(
                                    nth_offset(&core_source, "value", 1),
                                    nth_offset(&core_source, "value", 1) + "value".len(),
                                ),
                            ),
                            "count".to_owned(),
                        ),
                        TextEdit::new(
                            span_to_range(
                                &core_source,
                                Span::new(
                                    nth_offset(&core_source, "value", 2),
                                    nth_offset(&core_source, "value", 2) + "value".len(),
                                ),
                            ),
                            "count".to_owned(),
                        ),
                    ],
                ),
            ],
        );
    }

    #[test]
    fn workspace_dependency_member_prepare_rename_prefers_open_local_dependency_source() {
        let temp = TempDir::new("ql-lsp-workspace-dependency-open-doc-member-prepare-rename");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.shared.alpha.build as build

pub fn main() -> Int {
    let current = build()
    return current.extra.id + current.pulse().id
}
"#,
        );
        let alpha_source_path = temp.write(
            "workspace/vendor/alpha/src/lib.ql",
            r#"
package demo.shared.alpha

pub struct Counter {
    value: Int,
}

impl Counter {
    pub fn ping(self) -> Int {
        return self.value
    }
}

pub fn build() -> Counter {
    return Counter { value: 1 }
}
"#,
        );
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
            "workspace/vendor/alpha/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.shared.alpha

pub struct Counter {
    value: Int,
}

impl Counter {
    pub fn ping(self) -> Int
}

pub fn build() -> Counter
"#,
        );

        let open_alpha_source = r#"
package demo.shared.alpha

pub struct Extra {
    id: Int,
}

pub struct Counter {
    value: Int,
    extra: Extra,
}

impl Counter {
    pub fn pulse(self) -> Extra {
        return self.extra
    }
}

pub fn build() -> Counter {
    return Counter { value: 1, extra: Extra { id: 2 } }
}
"#;
        let source = fs::read_to_string(&app_path).expect("app source should read");
        let analysis = analyze_source(&source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let alpha_uri =
            Url::from_file_path(&alpha_source_path).expect("alpha path should convert to URI");
        let open_docs = file_open_documents(vec![(alpha_uri, open_alpha_source.to_owned())]);

        for (needle, occurrence, kind) in [
            ("extra", 1usize, AnalysisSymbolKind::Field),
            ("pulse", 1usize, AnalysisSymbolKind::Method),
        ] {
            let offset = nth_offset(&source, needle, occurrence);
            assert!(
                package
                    .dependency_prepare_rename_in_source_at(&source, offset + 1)
                    .is_none(),
                "disk-only prepare rename should miss unsaved dependency member {needle}",
            );

            let rename_target = workspace_source_dependency_prepare_rename_with_open_docs(
                &source,
                Some(&analysis),
                &package,
                &open_docs,
                offset_to_position(&source, offset + 1),
            )
            .expect("open-doc prepare rename should resolve unsaved dependency member");
            assert_eq!(rename_target.kind, kind);
            assert_eq!(rename_target.name, needle);
            assert_eq!(rename_target.span, Span::new(offset, offset + needle.len()));
        }
    }

    #[test]
    fn workspace_dependency_member_rename_prefers_open_local_dependency_source() {
        let temp = TempDir::new("ql-lsp-workspace-dependency-open-doc-member-rename");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.shared.alpha.build as build

pub fn main() -> Int {
    return build().pulse()
}
"#,
        );
        let task_path = temp.write(
            "workspace/packages/app/src/task.ql",
            r#"
package demo.app

use demo.shared.alpha.build as build
use demo.shared.alpha.forward as forward

pub fn task() -> Int {
    return build().pulse() + forward(build())
}
"#,
        );
        let alpha_source_path = temp.write(
            "workspace/vendor/alpha/src/lib.ql",
            r#"
package demo.shared.alpha

pub struct Counter {
    value: Int,
}

impl Counter {
    pub fn ping(self) -> Int {
        return self.value
    }
}

pub fn build() -> Counter {
    return Counter { value: 1 }
}
"#,
        );
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
            "workspace/vendor/alpha/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.shared.alpha

pub struct Counter {
    value: Int,
}

impl Counter {
    pub fn ping(self) -> Int
}

pub fn build() -> Counter
"#,
        );

        let open_alpha_source = r#"
package demo.shared.alpha

pub struct Counter {
    value: Int,
}

impl Counter {
    pub fn pulse(self) -> Int {
        return self.value
    }
}

pub fn forward(counter: Counter) -> Int {
    return counter.pulse()
}

pub fn build() -> Counter {
    return Counter { value: 1 }
}
"#;
        let source = fs::read_to_string(&app_path).expect("app source should read");
        let analysis = analyze_source(&source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let task_source = fs::read_to_string(&task_path).expect("task source should read");
        let task_uri = Url::from_file_path(&task_path).expect("task path should convert to URI");
        let alpha_uri =
            Url::from_file_path(&alpha_source_path).expect("alpha path should convert to URI");
        let pulse_position = offset_to_position(&source, nth_offset(&source, "pulse", 1) + 1);

        let empty_docs = file_open_documents(vec![]);
        assert!(
            rename_for_workspace_source_dependency_with_open_docs(
                &uri,
                &source,
                Some(&analysis),
                &package,
                &empty_docs,
                pulse_position,
                "probe",
            )
            .expect("disk-only rename should evaluate")
            .is_none(),
            "disk-only rename should miss unsaved dependency member",
        );

        let open_docs = file_open_documents(vec![
            (uri.clone(), source.clone()),
            (task_uri.clone(), task_source.clone()),
            (alpha_uri.clone(), open_alpha_source.to_owned()),
        ]);
        let edit = rename_for_workspace_source_dependency_with_open_docs(
            &uri,
            &source,
            Some(&analysis),
            &package,
            &open_docs,
            pulse_position,
            "probe",
        )
        .expect("rename should succeed")
        .expect("rename should return workspace edits");

        assert_workspace_edit_changes(
            edit,
            vec![
                (
                    uri,
                    vec![TextEdit::new(
                        span_to_range(
                            &source,
                            Span::new(
                                nth_offset(&source, "pulse", 1),
                                nth_offset(&source, "pulse", 1) + "pulse".len(),
                            ),
                        ),
                        "probe".to_owned(),
                    )],
                ),
                (
                    task_uri,
                    vec![TextEdit::new(
                        span_to_range(
                            &task_source,
                            Span::new(
                                nth_offset(&task_source, "pulse", 1),
                                nth_offset(&task_source, "pulse", 1) + "pulse".len(),
                            ),
                        ),
                        "probe".to_owned(),
                    )],
                ),
                (
                    alpha_uri,
                    vec![
                        TextEdit::new(
                            span_to_range(
                                open_alpha_source,
                                Span::new(
                                    nth_offset(open_alpha_source, "pulse", 1),
                                    nth_offset(open_alpha_source, "pulse", 1) + "pulse".len(),
                                ),
                            ),
                            "probe".to_owned(),
                        ),
                        TextEdit::new(
                            span_to_range(
                                open_alpha_source,
                                Span::new(
                                    nth_offset(open_alpha_source, "pulse", 2),
                                    nth_offset(open_alpha_source, "pulse", 2) + "pulse".len(),
                                ),
                            ),
                            "probe".to_owned(),
                        ),
                    ],
                ),
            ],
        );
    }

    #[test]
    fn local_dependency_method_rename_updates_workspace_consumers_from_source_definition() {
        let temp = TempDir::new("ql-lsp-local-dependency-method-rename-source-definition");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.Config as Cfg

pub fn main(config: Cfg) -> Int {
    return config.ping()
}
"#,
        );
        let task_path = temp.write(
            "workspace/packages/app/src/task.ql",
            r#"
package demo.app

use demo.core.Config as OtherCfg

pub fn task(config: OtherCfg) -> Int {
    return config.ping()
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub struct Config {
    value: Int,
}

impl Config {
    pub fn ping(self) -> Int {
        return self.value
    }

    pub fn repeat(self) -> Int {
        return self.ping()
    }
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub struct Config {
    value: Int,
}

impl Config {
    pub fn ping(self) -> Int
}
"#,
        );

        let app_source = fs::read_to_string(&app_path).expect("app source should read");
        let app_analysis = analyze_source(&app_source).expect("app source should analyze");
        let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let task_source = fs::read_to_string(&task_path).expect("task source should read");
        let task_uri = Url::from_file_path(&task_path).expect("task path should convert to URI");
        let core_source = fs::read_to_string(&core_source_path).expect("core source should read");
        let core_analysis = analyze_source(&core_source).expect("core source should analyze");
        let core_uri =
            Url::from_file_path(&core_source_path).expect("core source path should convert to URI");
        let package =
            package_analysis_for_path(&core_source_path).expect("package analysis should succeed");
        let app_package =
            package_analysis_for_path(&app_path).expect("app package analysis should succeed");
        let open_docs = file_open_documents(vec![
            (app_uri.clone(), app_source.clone()),
            (task_uri.clone(), task_source.clone()),
            (core_uri.clone(), core_source.clone()),
        ]);
        let local_target = local_source_dependency_target_with_analysis(
            &core_uri,
            &core_source,
            &core_analysis,
            &package,
            &open_docs,
            offset_to_position(&core_source, nth_offset(&core_source, "ping", 1)),
        )
        .expect("local dependency target should exist");
        let app_target = dependency_definition_target_at(
            &app_source,
            Some(&app_analysis),
            &app_package,
            offset_to_position(&app_source, nth_offset(&app_source, "ping", 1)),
        )
        .expect("app dependency target should exist");
        assert!(
            same_dependency_definition_target(&local_target, &app_target),
            "local source target should match app dependency target: left={local_target:?} right={app_target:?}",
        );
        let external_locations = workspace_dependency_reference_locations_with_open_docs(
            &package,
            Some(core_source_path.as_path()),
            &open_docs,
            &local_target,
            false,
        );
        assert!(
            !external_locations.is_empty(),
            "workspace dependency references should exist for local source target: {local_target:?}",
        );

        let edit = rename_for_local_source_dependency_with_open_docs(
            &core_uri,
            &core_source,
            &core_analysis,
            &package,
            &open_docs,
            offset_to_position(&core_source, nth_offset(&core_source, "ping", 1)),
            "probe",
        )
        .expect("rename should succeed")
        .expect("rename should produce workspace edits");

        assert_workspace_edit_changes(
            edit,
            vec![
                (
                    core_uri,
                    vec![
                        TextEdit::new(
                            span_to_range(
                                &core_source,
                                Span::new(
                                    nth_offset(&core_source, "ping", 1),
                                    nth_offset(&core_source, "ping", 1) + "ping".len(),
                                ),
                            ),
                            "probe".to_owned(),
                        ),
                        TextEdit::new(
                            span_to_range(
                                &core_source,
                                Span::new(
                                    nth_offset(&core_source, "ping", 2),
                                    nth_offset(&core_source, "ping", 2) + "ping".len(),
                                ),
                            ),
                            "probe".to_owned(),
                        ),
                    ],
                ),
                (
                    app_uri,
                    vec![TextEdit::new(
                        span_to_range(
                            &app_source,
                            Span::new(
                                nth_offset(&app_source, "ping", 1),
                                nth_offset(&app_source, "ping", 1) + "ping".len(),
                            ),
                        ),
                        "probe".to_owned(),
                    )],
                ),
                (
                    task_uri,
                    vec![TextEdit::new(
                        span_to_range(
                            &task_source,
                            Span::new(
                                nth_offset(&task_source, "ping", 1),
                                nth_offset(&task_source, "ping", 1) + "ping".len(),
                            ),
                        ),
                        "probe".to_owned(),
                    )],
                ),
            ],
        );
    }

    #[test]
    fn local_dependency_field_rename_updates_workspace_consumers_from_source_definition() {
        let temp = TempDir::new("ql-lsp-local-dependency-field-rename-source-definition");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.Config as Cfg

pub fn main(config: Cfg) -> Int {
    return config.value
}
"#,
        );
        let task_path = temp.write(
            "workspace/packages/app/src/task.ql",
            r#"
package demo.app

use demo.core.Config as OtherCfg

pub fn task(config: OtherCfg) -> Int {
    return config.value
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub struct Config {
    value: Int,
    limit: Int,
}

impl Config {
    pub fn total(self) -> Int {
        return self.value + self.limit
    }
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub struct Config {
    value: Int,
    limit: Int,
}

impl Config {
    pub fn total(self) -> Int
}
"#,
        );

        let app_source = fs::read_to_string(&app_path).expect("app source should read");
        let app_analysis = analyze_source(&app_source).expect("app source should analyze");
        let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let task_source = fs::read_to_string(&task_path).expect("task source should read");
        let task_uri = Url::from_file_path(&task_path).expect("task path should convert to URI");
        let core_source = fs::read_to_string(&core_source_path).expect("core source should read");
        let core_analysis = analyze_source(&core_source).expect("core source should analyze");
        let core_uri =
            Url::from_file_path(&core_source_path).expect("core source path should convert to URI");
        let package =
            package_analysis_for_path(&core_source_path).expect("package analysis should succeed");
        let app_package =
            package_analysis_for_path(&app_path).expect("app package analysis should succeed");
        let open_docs = file_open_documents(vec![
            (app_uri.clone(), app_source.clone()),
            (task_uri.clone(), task_source.clone()),
            (core_uri.clone(), core_source.clone()),
        ]);
        let local_target = local_source_dependency_target_with_analysis(
            &core_uri,
            &core_source,
            &core_analysis,
            &package,
            &open_docs,
            offset_to_position(&core_source, nth_offset(&core_source, "value", 1)),
        )
        .expect("local dependency target should exist");
        let app_target = dependency_definition_target_at(
            &app_source,
            Some(&app_analysis),
            &app_package,
            offset_to_position(&app_source, nth_offset(&app_source, "value", 1)),
        )
        .expect("app dependency target should exist");
        assert!(
            same_dependency_definition_target(&local_target, &app_target),
            "local source target should match app dependency target: left={local_target:?} right={app_target:?}",
        );
        let external_locations = workspace_dependency_reference_locations_with_open_docs(
            &package,
            Some(core_source_path.as_path()),
            &open_docs,
            &local_target,
            false,
        );
        assert!(
            !external_locations.is_empty(),
            "workspace dependency references should exist for local source target: {local_target:?}",
        );

        let edit = rename_for_local_source_dependency_with_open_docs(
            &core_uri,
            &core_source,
            &core_analysis,
            &package,
            &open_docs,
            offset_to_position(&core_source, nth_offset(&core_source, "value", 1)),
            "count",
        )
        .expect("rename should succeed")
        .expect("rename should produce workspace edits");

        assert_workspace_edit_changes(
            edit,
            vec![
                (
                    core_uri,
                    vec![
                        TextEdit::new(
                            span_to_range(
                                &core_source,
                                Span::new(
                                    nth_offset(&core_source, "value", 1),
                                    nth_offset(&core_source, "value", 1) + "value".len(),
                                ),
                            ),
                            "count".to_owned(),
                        ),
                        TextEdit::new(
                            span_to_range(
                                &core_source,
                                Span::new(
                                    nth_offset(&core_source, "value", 2),
                                    nth_offset(&core_source, "value", 2) + "value".len(),
                                ),
                            ),
                            "count".to_owned(),
                        ),
                    ],
                ),
                (
                    app_uri,
                    vec![TextEdit::new(
                        span_to_range(
                            &app_source,
                            Span::new(
                                nth_offset(&app_source, "value", 1),
                                nth_offset(&app_source, "value", 1) + "value".len(),
                            ),
                        ),
                        "count".to_owned(),
                    )],
                ),
                (
                    task_uri,
                    vec![TextEdit::new(
                        span_to_range(
                            &task_source,
                            Span::new(
                                nth_offset(&task_source, "value", 1),
                                nth_offset(&task_source, "value", 1) + "value".len(),
                            ),
                        ),
                        "count".to_owned(),
                    )],
                ),
            ],
        );
    }

    #[test]
    fn local_dependency_queries_prefer_dependency_source_over_interface_artifact() {
        let temp = TempDir::new("ql-lsp-local-dependency-source-queries");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.Config as Cfg
use demo.core.exported as run

pub fn main(config: Cfg) -> Int {
    let built = Cfg { value: 1, limit: 2 }
    return run(config.ping()) + built.value
}
"#,
        );
        let task_path = temp.write(
            "workspace/packages/app/src/task.ql",
            r#"
package demo.app

use demo.core.Config as OtherCfg
use demo.core.exported as call

pub fn task(config: OtherCfg) -> Int {
    return call(config.ping()) + config.value
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/vendor/core/src/lib.ql",
            r#"
package demo.core

pub struct Config {
    value: Int,
    limit: Int,
}

impl Config {
    pub fn ping(self) -> Int {
        return self.value + self.limit
    }
}

pub fn exported(value: Int) -> Int {
    return value
}

pub fn wrapper(value: Int) -> Int {
    return exported(value)
}
"#,
        );
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
core = { path = "../../vendor/core" }
"#,
        );
        temp.write(
            "workspace/vendor/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/vendor/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub struct Config {
    value: Int,
    limit: Int,
}

impl Config {
    pub fn ping(self) -> Int
}

pub fn exported(value: Int) -> Int
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        let analysis = analyze_source(&source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let core_source =
            fs::read_to_string(&core_source_path).expect("core source should read for assertions");
        let task_source = fs::read_to_string(&task_path).expect("task source should read");
        let task_uri = Url::from_file_path(&task_path).expect("task path should convert to URI");

        let definition = workspace_source_definition_for_dependency(
            &uri,
            &source,
            Some(&analysis),
            &package,
            offset_to_position(&source, nth_offset(&source, "run", 2)),
        )
        .expect("local dependency definition should exist");
        let GotoDefinitionResponse::Scalar(definition_location) = definition else {
            panic!("local dependency definition should resolve to one location")
        };
        assert_eq!(
            definition_location
                .uri
                .to_file_path()
                .expect("definition URI should convert to a file path")
                .canonicalize()
                .expect("definition path should canonicalize"),
            core_source_path
                .canonicalize()
                .expect("core source path should canonicalize"),
        );
        assert_eq!(
            definition_location.range.start,
            offset_to_position(&core_source, nth_offset(&core_source, "exported", 1)),
        );

        let type_definition = workspace_source_type_definition_for_dependency(
            &uri,
            &source,
            Some(&analysis),
            &package,
            offset_to_position(&source, nth_offset(&source, "Cfg", 2)),
        )
        .expect("local dependency type definition should exist");
        let GotoTypeDefinitionResponse::Scalar(type_location) = type_definition else {
            panic!("local dependency type definition should resolve to one location")
        };
        assert_eq!(
            type_location
                .uri
                .to_file_path()
                .expect("type definition URI should convert to a file path")
                .canonicalize()
                .expect("type definition path should canonicalize"),
            core_source_path
                .canonicalize()
                .expect("core source path should canonicalize"),
        );
        assert_eq!(
            type_location.range.start,
            offset_to_position(&core_source, nth_offset(&core_source, "Config", 1)),
        );

        let references = workspace_source_references_for_dependency(
            &uri,
            &source,
            Some(&analysis),
            &package,
            offset_to_position(&source, nth_offset(&source, "run", 2)),
            true,
        )
        .expect("local dependency references should exist");
        assert_eq!(references.len(), 6);
        assert_eq!(
            references[0]
                .uri
                .to_file_path()
                .expect("definition URI should convert to a file path")
                .canonicalize()
                .expect("definition path should canonicalize"),
            core_source_path
                .canonicalize()
                .expect("core source path should canonicalize"),
        );
        assert_eq!(
            references[0].range.start,
            offset_to_position(&core_source, nth_offset(&core_source, "exported", 1)),
        );

        for (reference, local_occurrence) in references[1..3].iter().zip([1usize, 2usize]) {
            assert_eq!(reference.uri, uri);
            assert_eq!(
                reference.range.start,
                offset_to_position(&source, nth_offset(&source, "run", local_occurrence)),
            );
        }
        assert_eq!(
            references[3]
                .uri
                .to_file_path()
                .expect("reference URI should convert to a file path")
                .canonicalize()
                .expect("reference path should canonicalize"),
            core_source_path
                .canonicalize()
                .expect("core source path should canonicalize"),
        );
        assert_eq!(
            references[3].range.start,
            offset_to_position(&core_source, nth_offset(&core_source, "exported", 2)),
        );
        for (reference, local_occurrence) in references[4..].iter().zip([1usize, 2usize]) {
            assert_eq!(reference.uri, task_uri);
            assert_eq!(
                reference.range.start,
                offset_to_position(
                    &task_source,
                    nth_offset(&task_source, "call", local_occurrence)
                ),
            );
        }
    }

    #[test]
    fn same_named_local_dependency_queries_prefer_matching_dependency_source() {
        let temp = TempDir::new("ql-lsp-same-named-local-dependency-source-queries");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.shared.alpha.Config as Cfg
use demo.shared.alpha.exported as run

pub fn main(config: Cfg) -> Int {
    return run(config.value)
}
"#,
        );
        let task_path = temp.write(
            "workspace/packages/app/src/task.ql",
            r#"
package demo.app

use demo.shared.beta.Config as OtherCfg
use demo.shared.beta.exported as call

pub fn task(config: OtherCfg) -> Int {
    return call(config.value)
}
"#,
        );
        let alpha_source_path = temp.write(
            "workspace/vendor/alpha/src/lib.ql",
            r#"
package demo.shared.alpha

pub struct Config {
    value: Int,
}

pub fn exported(value: Int) -> Int {
    return value
}
"#,
        );
        let beta_source_path = temp.write(
            "workspace/vendor/beta/src/lib.ql",
            r#"
package demo.shared.beta

pub struct Config {
    value: Int,
}

pub fn exported(value: Int) -> Int {
    return value
}
"#,
        );
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

pub struct Config {
    value: Int,
}

pub fn exported(value: Int) -> Int
"#,
        );
        temp.write(
            "workspace/vendor/beta/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.shared.beta

pub struct Config {
    value: Int,
}

pub fn exported(value: Int) -> Int
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        let analysis = analyze_source(&source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let alpha_source =
            fs::read_to_string(&alpha_source_path).expect("alpha source should read");
        let task_uri = Url::from_file_path(&task_path).expect("task path should convert to URI");

        let definition = workspace_source_definition_for_dependency(
            &uri,
            &source,
            Some(&analysis),
            &package,
            offset_to_position(&source, nth_offset(&source, "run", 2)),
        )
        .expect("same-named local dependency definition should exist");
        let GotoDefinitionResponse::Scalar(definition_location) = definition else {
            panic!("same-named local dependency definition should resolve to one location")
        };
        assert_eq!(
            definition_location
                .uri
                .to_file_path()
                .expect("definition URI should convert to a file path")
                .canonicalize()
                .expect("definition path should canonicalize"),
            alpha_source_path
                .canonicalize()
                .expect("alpha source path should canonicalize"),
        );
        assert_eq!(
            definition_location.range.start,
            offset_to_position(&alpha_source, nth_offset(&alpha_source, "exported", 1)),
        );

        let type_definition = workspace_source_type_definition_for_dependency(
            &uri,
            &source,
            Some(&analysis),
            &package,
            offset_to_position(&source, nth_offset(&source, "Cfg", 2)),
        )
        .expect("same-named local dependency type definition should exist");
        let GotoTypeDefinitionResponse::Scalar(type_location) = type_definition else {
            panic!("same-named local dependency type definition should resolve to one location")
        };
        assert_eq!(
            type_location
                .uri
                .to_file_path()
                .expect("type definition URI should convert to a file path")
                .canonicalize()
                .expect("type definition path should canonicalize"),
            alpha_source_path
                .canonicalize()
                .expect("alpha source path should canonicalize"),
        );
        assert_eq!(
            type_location.range.start,
            offset_to_position(&alpha_source, nth_offset(&alpha_source, "Config", 1)),
        );

        let references = workspace_source_references_for_dependency(
            &uri,
            &source,
            Some(&analysis),
            &package,
            offset_to_position(&source, nth_offset(&source, "run", 2)),
            true,
        )
        .expect("same-named local dependency references should exist");

        assert_eq!(references.len(), 3);
        assert_eq!(
            references[0]
                .uri
                .to_file_path()
                .expect("reference URI should convert to a file path")
                .canonicalize()
                .expect("reference path should canonicalize"),
            alpha_source_path
                .canonicalize()
                .expect("alpha source path should canonicalize"),
        );
        assert_eq!(
            references[0].range.start,
            offset_to_position(&alpha_source, nth_offset(&alpha_source, "exported", 1)),
        );
        assert_eq!(references[1].uri, uri);
        assert_eq!(
            references[1].range.start,
            offset_to_position(&source, nth_offset(&source, "run", 1)),
        );
        assert_eq!(references[2].uri, uri);
        assert_eq!(
            references[2].range.start,
            offset_to_position(&source, nth_offset(&source, "run", 2)),
        );
        assert!(
            references.iter().all(|reference| reference.uri != task_uri),
            "references should not include same-named sibling dependency uses",
        );
        assert!(
            references.iter().all(|reference| {
                reference
                    .uri
                    .to_file_path()
                    .ok()
                    .and_then(|path| path.canonicalize().ok())
                    != beta_source_path.canonicalize().ok()
            }),
            "references should not include beta dependency source",
        );
    }

    #[test]
    fn same_named_local_dependency_broken_source_member_queries_prefer_matching_dependency_source()
    {
        let temp = TempDir::new("ql-lsp-same-named-local-dependency-broken-source-member-queries");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.shared.alpha.build as build

pub fn main() -> Int {
    return build().ping() + build().value
"#,
        );
        let task_path = temp.write(
            "workspace/packages/app/src/task.ql",
            r#"
package demo.app

use demo.shared.beta.build as other

pub fn task() -> Bool {
    return other().ping() && other().value
}
"#,
        );
        let alpha_source_path = temp.write(
            "workspace/vendor/alpha/src/lib.ql",
            r#"
package demo.shared.alpha

pub struct Config {
    value: Int,
}

pub fn build() -> Config {
    return Config { value: 1 }
}

impl Config {
    pub fn ping(self) -> Int {
        return self.value
    }
}
"#,
        );
        let beta_source_path = temp.write(
            "workspace/vendor/beta/src/lib.ql",
            r#"
package demo.shared.beta

pub struct Config {
    value: Bool,
}

pub fn build() -> Config {
    return Config { value: true }
}

impl Config {
    pub fn ping(self) -> Bool {
        return self.value
    }
}
"#,
        );
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

pub struct Config {
    value: Int,
}

pub fn build() -> Config

impl Config {
    pub fn ping(self) -> Int
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

pub struct Config {
    value: Bool,
}

pub fn build() -> Config

impl Config {
    pub fn ping(self) -> Bool
}
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&source).is_err());
        let package = package_analysis_for_path(&app_path)
            .expect("package analysis should survive parse errors");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let alpha_source =
            fs::read_to_string(&alpha_source_path).expect("alpha source should read");
        let task_uri = Url::from_file_path(&task_path).expect("task path should convert to URI");

        let method_hover = workspace_source_hover_for_dependency(
            &uri,
            &source,
            None,
            &package,
            offset_to_position(&source, nth_offset(&source, "ping", 1)),
        )
        .expect("broken-source same-named dependency method hover should exist");
        let HoverContents::Markup(method_hover_markup) = method_hover.contents else {
            panic!("method hover should render as markdown")
        };
        assert!(method_hover_markup.value.contains("fn ping(self) -> Int"));
        assert!(!method_hover_markup.value.contains("fn ping(self) -> Bool"));

        let field_hover = workspace_source_hover_for_dependency(
            &uri,
            &source,
            None,
            &package,
            offset_to_position(&source, nth_offset(&source, "value", 1)),
        )
        .expect("broken-source same-named dependency field hover should exist");
        let HoverContents::Markup(field_hover_markup) = field_hover.contents else {
            panic!("field hover should render as markdown")
        };
        assert!(field_hover_markup.value.contains("field value: Int"));
        assert!(!field_hover_markup.value.contains("field value: Bool"));

        let method_definition = workspace_source_definition_for_dependency(
            &uri,
            &source,
            None,
            &package,
            offset_to_position(&source, nth_offset(&source, "ping", 1)),
        )
        .expect("broken-source same-named dependency method definition should exist");
        let GotoDefinitionResponse::Scalar(method_location) = method_definition else {
            panic!("broken-source method definition should resolve to one location")
        };
        assert_eq!(
            method_location
                .uri
                .to_file_path()
                .expect("method definition URI should convert to a file path")
                .canonicalize()
                .expect("method definition path should canonicalize"),
            alpha_source_path
                .canonicalize()
                .expect("alpha source path should canonicalize"),
        );
        assert_eq!(
            method_location.range.start,
            offset_to_position(&alpha_source, nth_offset(&alpha_source, "ping", 1)),
        );

        let field_definition = workspace_source_definition_for_dependency(
            &uri,
            &source,
            None,
            &package,
            offset_to_position(&source, nth_offset(&source, "value", 1)),
        )
        .expect("broken-source same-named dependency field definition should exist");
        let GotoDefinitionResponse::Scalar(field_location) = field_definition else {
            panic!("broken-source field definition should resolve to one location")
        };
        assert_eq!(
            field_location
                .uri
                .to_file_path()
                .expect("field definition URI should convert to a file path")
                .canonicalize()
                .expect("field definition path should canonicalize"),
            alpha_source_path
                .canonicalize()
                .expect("alpha source path should canonicalize"),
        );
        assert_eq!(
            field_location.range.start,
            offset_to_position(&alpha_source, nth_offset(&alpha_source, "value", 1)),
        );

        let references = workspace_source_references_for_dependency_in_broken_source(
            &uri,
            &source,
            &package,
            offset_to_position(&source, nth_offset(&source, "ping", 1)),
            true,
        )
        .expect("broken-source same-named dependency method references should exist");

        assert_eq!(references.len(), 2);
        assert!(
            references.iter().any(|reference| {
                reference
                    .uri
                    .to_file_path()
                    .ok()
                    .and_then(|path| path.canonicalize().ok())
                    == alpha_source_path.canonicalize().ok()
                    && reference.range.start
                        == offset_to_position(&alpha_source, nth_offset(&alpha_source, "ping", 1))
            }),
            "references should include alpha dependency source method definition",
        );
        assert!(
            references.iter().any(|reference| {
                reference.uri == uri
                    && reference.range.start
                        == offset_to_position(&source, nth_offset(&source, "ping", 1))
            }),
            "references should include broken-source local method occurrence",
        );
        assert!(
            references.iter().all(|reference| reference.uri != task_uri),
            "references should not include same-named sibling dependency uses",
        );
        assert!(
            references.iter().all(|reference| {
                reference
                    .uri
                    .to_file_path()
                    .ok()
                    .and_then(|path| path.canonicalize().ok())
                    != beta_source_path.canonicalize().ok()
            }),
            "references should not include beta dependency source",
        );

        let completion_source = r#"
package demo.app

use demo.shared.alpha.build as build

pub fn main() -> Int {
    return build().pi( + build().va
"#;
        assert!(analyze_source(completion_source).is_err());

        let method_completion = completion_for_dependency_methods(
            completion_source,
            &package,
            offset_to_position(
                completion_source,
                nth_offset(completion_source, "pi", 1) + 2,
            ),
        )
        .expect("broken-source same-named dependency method completion should exist");
        let CompletionResponse::Array(method_items) = method_completion else {
            panic!("method completion should resolve to a plain item array")
        };
        assert_eq!(method_items.len(), 1);
        assert_eq!(method_items[0].label, "ping");
        assert_eq!(method_items[0].kind, Some(CompletionItemKind::METHOD));
        assert_eq!(
            method_items[0].detail.as_deref(),
            Some("fn ping(self) -> Int")
        );

        let field_completion = completion_for_dependency_member_fields(
            completion_source,
            &package,
            offset_to_position(
                completion_source,
                nth_offset(completion_source, "va", 1) + 2,
            ),
        )
        .expect("broken-source same-named dependency field completion should exist");
        let CompletionResponse::Array(field_items) = field_completion else {
            panic!("field completion should resolve to a plain item array")
        };
        assert_eq!(field_items.len(), 1);
        assert_eq!(field_items[0].label, "value");
        assert_eq!(field_items[0].kind, Some(CompletionItemKind::FIELD));
        assert_eq!(field_items[0].detail.as_deref(), Some("field value: Int"));
    }

    #[test]
    fn same_named_local_dependency_broken_open_type_implementation_prefers_matching_dependency_source()
     {
        let temp =
            TempDir::new("ql-lsp-same-named-local-dependency-broken-open-type-implementation");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.shared.alpha.Config as Cfg

pub fn main(config: Cfg) -> Int {
    return 0
}
"#,
        );
        let task_path = temp.write(
            "workspace/packages/app/src/task.ql",
            r#"
package demo.app

pub fn task() -> Int {
    return 0
}
"#,
        );
        let alpha_source_path = temp.write(
            "workspace/vendor/alpha/src/lib.ql",
            r#"
package demo.shared.alpha

pub struct Config {
    value: Int,
}
"#,
        );
        temp.write(
            "workspace/vendor/beta/src/lib.ql",
            r#"
package demo.shared.beta

pub struct Config {
    value: Bool,
}
"#,
        );
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

pub struct Config {
    value: Int,
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

pub struct Config {
    value: Bool,
}
"#,
        );

        let open_task_source = r#"
package demo.app

use demo.shared.alpha.Config as Cfg
use demo.shared.beta.Config as OtherCfg

extend Cfg {
    fn alpha(self) -> Int {
        return 1
    }
}

extend OtherCfg {
    fn beta(self) -> Bool {
        return true
    }
}

pub fn broken() -> Int {
    return Cfg {
"#
        .to_owned();

        let source = fs::read_to_string(&app_path).expect("app source should read");
        let analysis = analyze_source(&source).expect("app source should analyze");
        let package = package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let task_uri = Url::from_file_path(&task_path).expect("task path should convert to URI");

        assert!(analyze_source(&open_task_source).is_err());

        let implementation = workspace_source_implementation_for_dependency_with_open_docs(
            &source,
            Some(&analysis),
            &package,
            &file_open_documents(vec![(task_uri.clone(), open_task_source.clone())]),
            offset_to_position(&source, nth_offset(&source, "Cfg", 2)),
        )
        .expect("broken open same-named dependency type implementation should exist");

        let GotoDefinitionResponse::Scalar(location) = implementation else {
            panic!("matching broken open dependency implementation should stay scalar")
        };
        assert_eq!(location.uri, task_uri);
        assert_eq!(
            location.range.start,
            offset_to_position(&open_task_source, nth_offset(&open_task_source, "extend Cfg", 1)),
        );
        assert_ne!(
            location
                .uri
                .to_file_path()
                .expect("implementation URI should convert to file path")
                .canonicalize()
                .expect("implementation path should canonicalize"),
            alpha_source_path
                .canonicalize()
                .expect("alpha source path should canonicalize"),
            "implementation should come from the broken open consumer, not dependency source",
        );
    }

    #[test]
    fn same_named_local_dependency_broken_open_trait_method_implementation_prefers_matching_dependency_source(
    ) {
        let temp = TempDir::new(
            "qlsp-same-named-local-dependency-broken-open-trait-method-implementation",
        );
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.shared.alpha.Runner

pub fn main(runner: Runner) -> Int {
    return runner.run()
}
"#,
        );
        let task_path = temp.write(
            "workspace/packages/app/src/task.ql",
            r#"
package demo.app

pub fn task() -> Int {
    return 0
}
"#,
        );
        temp.write(
            "workspace/vendor/alpha/src/lib.ql",
            r#"
package demo.shared.alpha

pub trait Runner {
    fn run(self) -> Int
}
"#,
        );
        temp.write(
            "workspace/vendor/beta/src/lib.ql",
            r#"
package demo.shared.beta

pub trait Runner {
    fn run(self) -> Bool
}
"#,
        );
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

pub trait Runner {
    fn run(self) -> Int
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

pub trait Runner {
    fn run(self) -> Bool
}
"#,
        );

        let open_task_source = r#"
package demo.app

use demo.shared.alpha.Runner
use demo.shared.beta.Runner as OtherRunner

struct AlphaWorker {}
struct BetaWorker {}

impl Runner for AlphaWorker {
    fn run(self) -> Int {
        return 1
    }
}

impl OtherRunner for BetaWorker {
    fn run(self) -> Bool {
        return true
    }
}

pub fn broken() -> Int {
    return AlphaWorker {
"#
        .to_owned();

        let source = fs::read_to_string(&app_path).expect("app source should read");
        let analysis = analyze_source(&source).expect("app source should analyze");
        let package = package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let task_uri = Url::from_file_path(&task_path).expect("task path should convert to URI");

        assert!(analyze_source(&open_task_source).is_err());

        let implementation = workspace_source_method_implementation_for_dependency_with_open_docs(
            &uri,
            &source,
            Some(&analysis),
            &package,
            &file_open_documents(vec![(task_uri.clone(), open_task_source.clone())]),
            offset_to_position(&source, nth_offset_in_context(&source, "run", "runner.run()", 1)),
        )
        .expect("broken open same-named trait method implementation should exist");

        let GotoDefinitionResponse::Scalar(location) = implementation else {
            panic!("matching broken open trait method implementation should stay scalar")
        };
        assert_eq!(location.uri, task_uri);
        assert_eq!(
            location.range.start,
            offset_to_position(
                &open_task_source,
                nth_offset_in_context(&open_task_source, "run", "fn run(self) -> Int", 1),
            ),
        );
    }

    #[test]
    fn same_named_local_dependency_broken_source_variant_queries_prefer_matching_dependency_source()
    {
        let temp = TempDir::new("ql-lsp-same-named-local-dependency-broken-source-variant-queries");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.shared.alpha.Command as Cmd
use demo.shared.beta.Command as OtherCmd

pub fn main() -> Int {
    let first = Cmd.Retry(1)
    let second = Cmd.Retry(2)
    let third = OtherCmd.Retry(
"#,
        );
        let alpha_source_path = temp.write(
            "workspace/vendor/alpha/src/lib.ql",
            r#"
package demo.shared.alpha

pub enum Command {
    Retry(Int),
}
"#,
        );
        let beta_source_path = temp.write(
            "workspace/vendor/beta/src/lib.ql",
            r#"
package demo.shared.beta

pub enum Command {
    Retry(Int),
}
"#,
        );
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

        let source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&source).is_err());
        let package = package_analysis_for_path(&app_path)
            .expect("package analysis should survive parse errors");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let alpha_source =
            fs::read_to_string(&alpha_source_path).expect("alpha source should read");

        let definition = workspace_source_definition_for_dependency(
            &uri,
            &source,
            None,
            &package,
            offset_to_position(&source, nth_offset(&source, "Retry", 2)),
        )
        .expect("broken-source same-named dependency variant definition should exist");
        let GotoDefinitionResponse::Scalar(definition_location) = definition else {
            panic!("broken-source variant definition should resolve to one location")
        };
        assert_eq!(
            definition_location
                .uri
                .to_file_path()
                .expect("definition URI should convert to a file path")
                .canonicalize()
                .expect("definition path should canonicalize"),
            alpha_source_path
                .canonicalize()
                .expect("alpha source path should canonicalize"),
        );
        assert_eq!(
            definition_location.range.start,
            offset_to_position(&alpha_source, nth_offset(&alpha_source, "Retry", 1)),
        );

        let type_definition = workspace_source_type_definition_for_dependency(
            &uri,
            &source,
            None,
            &package,
            offset_to_position(&source, nth_offset(&source, "Retry", 2)),
        )
        .expect("broken-source same-named dependency variant type definition should exist");
        let GotoTypeDefinitionResponse::Scalar(type_location) = type_definition else {
            panic!("broken-source variant type definition should resolve to one location")
        };
        assert_eq!(
            type_location
                .uri
                .to_file_path()
                .expect("type definition URI should convert to a file path")
                .canonicalize()
                .expect("type definition path should canonicalize"),
            alpha_source_path
                .canonicalize()
                .expect("alpha source path should canonicalize"),
        );
        assert_eq!(
            type_location.range.start,
            offset_to_position(&alpha_source, nth_offset(&alpha_source, "Command", 1)),
        );

        let references = workspace_source_references_for_dependency_in_broken_source(
            &uri,
            &source,
            &package,
            offset_to_position(&source, nth_offset(&source, "Retry", 2)),
            true,
        )
        .expect("broken-source same-named dependency variant references should exist");

        assert_eq!(references.len(), 3);
        assert!(
            references.iter().any(|reference| {
                reference
                    .uri
                    .to_file_path()
                    .ok()
                    .and_then(|path| path.canonicalize().ok())
                    == alpha_source_path.canonicalize().ok()
                    && reference.range.start
                        == offset_to_position(&alpha_source, nth_offset(&alpha_source, "Retry", 1))
            }),
            "references should include alpha dependency source variant definition",
        );
        assert!(
            references
                .iter()
                .filter(|reference| reference.uri == uri)
                .count()
                == 2,
            "references should keep only local alpha variant uses",
        );
        assert!(
            references.iter().all(|reference| {
                reference
                    .uri
                    .to_file_path()
                    .ok()
                    .and_then(|path| path.canonicalize().ok())
                    != beta_source_path.canonicalize().ok()
            }),
            "references should not include beta dependency source",
        );

        let highlights = fallback_document_highlights_for_package_at(
            &uri,
            &source,
            &package,
            offset_to_position(&source, nth_offset(&source, "Retry", 2)),
        )
        .expect("broken-source same-named dependency variant document highlight should exist");
        let actual = highlights
            .into_iter()
            .map(|highlight| highlight.range.start)
            .collect::<Vec<_>>();
        let expected = vec![
            offset_to_position(&source, nth_offset(&source, "Retry", 1)),
            offset_to_position(&source, nth_offset(&source, "Retry", 2)),
        ];
        assert_eq!(actual, expected);
    }

    #[test]
    fn same_named_local_dependency_broken_source_variant_prepare_rename_and_rename_prefer_matching_dependency_source()
     {
        let temp = TempDir::new("ql-lsp-same-named-local-dependency-broken-source-variant-rename");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.shared.alpha.Command as Cmd
use demo.shared.beta.Command as OtherCmd

pub fn main() -> Int {
    let first = Cmd.Retry(1)
    let second = Cmd.Retry(2)
    let third = OtherCmd.Retry(
"#,
        );
        temp.write(
            "workspace/vendor/alpha/src/lib.ql",
            r#"
package demo.shared.alpha

pub enum Command {
    Retry(Int),
}
"#,
        );
        temp.write(
            "workspace/vendor/beta/src/lib.ql",
            r#"
package demo.shared.beta

pub enum Command {
    Retry(Int),
}
"#,
        );
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

        let source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&source).is_err());
        let package = package_analysis_for_path(&app_path)
            .expect("package analysis should survive parse errors");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let use_offset = nth_offset(&source, "Retry", 2);

        assert_eq!(
            prepare_rename_for_dependency_imports(
                &source,
                &package,
                offset_to_position(&source, use_offset),
            ),
            Some(PrepareRenameResponse::RangeWithPlaceholder {
                range: span_to_range(&source, Span::new(use_offset, use_offset + "Retry".len())),
                placeholder: "Retry".to_owned(),
            }),
        );

        let edit = rename_for_dependency_imports(
            &uri,
            &source,
            &package,
            offset_to_position(&source, use_offset),
            "Repeat",
        )
        .expect("rename should succeed")
        .expect("rename should return workspace edits");
        assert_workspace_edit(
            edit,
            &uri,
            vec![
                TextEdit::new(
                    span_to_range(
                        &source,
                        Span::new(
                            nth_offset(&source, "Retry", 1),
                            nth_offset(&source, "Retry", 1) + "Retry".len(),
                        ),
                    ),
                    "Repeat".to_owned(),
                ),
                TextEdit::new(
                    span_to_range(
                        &source,
                        Span::new(
                            nth_offset(&source, "Retry", 2),
                            nth_offset(&source, "Retry", 2) + "Retry".len(),
                        ),
                    ),
                    "Repeat".to_owned(),
                ),
            ],
        );
    }

    #[test]
    fn same_named_local_dependency_broken_source_member_prepare_rename_and_rename_prefer_matching_dependency_source()
     {
        let temp = TempDir::new("ql-lsp-same-named-local-dependency-broken-source-member-rename");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.shared.alpha.build as build
use demo.shared.beta.build as other

pub fn main() -> Int {
    let first = build().ping()
    let second = build().ping()
    let third = build().value
    let fourth = build().value
    let fifth = other().ping() + other().value
    let broken = other(
"#,
        );
        temp.write(
            "workspace/vendor/alpha/src/lib.ql",
            r#"
package demo.shared.alpha

pub struct Config {
    value: Int,
}

pub fn build() -> Config {
    return Config { value: 1 }
}

impl Config {
    pub fn ping(self) -> Int {
        return self.value
    }
}
"#,
        );
        temp.write(
            "workspace/vendor/beta/src/lib.ql",
            r#"
package demo.shared.beta

pub struct Config {
    value: Bool,
}

pub fn build() -> Config {
    return Config { value: true }
}

impl Config {
    pub fn ping(self) -> Bool {
        return self.value
    }
}
"#,
        );
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

pub struct Config {
    value: Int,
}

pub fn build() -> Config

impl Config {
    pub fn ping(self) -> Int
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

pub struct Config {
    value: Bool,
}

pub fn build() -> Config

impl Config {
    pub fn ping(self) -> Bool
}
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&source).is_err());
        let package = package_analysis_for_path(&app_path)
            .expect("package analysis should survive parse errors");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");

        let method_use = nth_offset(&source, "ping", 2);
        assert_eq!(
            prepare_rename_for_dependency_imports(
                &source,
                &package,
                offset_to_position(&source, method_use),
            ),
            Some(PrepareRenameResponse::RangeWithPlaceholder {
                range: span_to_range(&source, Span::new(method_use, method_use + "ping".len())),
                placeholder: "ping".to_owned(),
            }),
        );

        let method_edit = rename_for_dependency_imports(
            &uri,
            &source,
            &package,
            offset_to_position(&source, method_use),
            "probe",
        )
        .expect("rename should succeed")
        .expect("rename should return workspace edits");
        assert_workspace_edit(
            method_edit,
            &uri,
            vec![
                TextEdit::new(
                    span_to_range(
                        &source,
                        Span::new(
                            nth_offset(&source, "ping", 1),
                            nth_offset(&source, "ping", 1) + "ping".len(),
                        ),
                    ),
                    "probe".to_owned(),
                ),
                TextEdit::new(
                    span_to_range(
                        &source,
                        Span::new(
                            nth_offset(&source, "ping", 2),
                            nth_offset(&source, "ping", 2) + "ping".len(),
                        ),
                    ),
                    "probe".to_owned(),
                ),
            ],
        );

        let field_use = nth_offset(&source, "value", 2);
        assert_eq!(
            prepare_rename_for_dependency_imports(
                &source,
                &package,
                offset_to_position(&source, field_use),
            ),
            Some(PrepareRenameResponse::RangeWithPlaceholder {
                range: span_to_range(&source, Span::new(field_use, field_use + "value".len())),
                placeholder: "value".to_owned(),
            }),
        );

        let field_edit = rename_for_dependency_imports(
            &uri,
            &source,
            &package,
            offset_to_position(&source, field_use),
            "count",
        )
        .expect("rename should succeed")
        .expect("rename should return workspace edits");
        assert_workspace_edit(
            field_edit,
            &uri,
            vec![
                TextEdit::new(
                    span_to_range(
                        &source,
                        Span::new(
                            nth_offset(&source, "value", 1),
                            nth_offset(&source, "value", 1) + "value".len(),
                        ),
                    ),
                    "count".to_owned(),
                ),
                TextEdit::new(
                    span_to_range(
                        &source,
                        Span::new(
                            nth_offset(&source, "value", 2),
                            nth_offset(&source, "value", 2) + "value".len(),
                        ),
                    ),
                    "count".to_owned(),
                ),
            ],
        );
    }

    #[test]
    fn same_named_local_dependency_broken_source_variant_completion_prefers_matching_dependency_source()
     {
        let temp =
            TempDir::new("ql-lsp-same-named-local-dependency-broken-source-variant-completion");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.shared.alpha.Command as Cmd
use demo.shared.beta.Command as OtherCmd

pub fn main() -> Int {
    let first = Cmd.B
"#,
        );
        temp.write(
            "workspace/vendor/alpha/src/lib.ql",
            r#"
package demo.shared.alpha

pub enum Command {
    Retry(Int),
    Backoff(Int),
}
"#,
        );
        temp.write(
            "workspace/vendor/beta/src/lib.ql",
            r#"
package demo.shared.beta

pub enum Command {
    Retry(Int),
    Block(Int),
}
"#,
        );
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
    Backoff(Int),
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
    Block(Int),
}
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&source).is_err());
        let package = package_analysis_for_path(&app_path)
            .expect("package analysis should survive parse errors");
        let completion = completion_for_dependency_variants(
            &source,
            &package,
            offset_to_position(&source, nth_offset(&source, "B", 1) + 1),
        )
        .expect("broken-source same-named dependency variant completion should exist");

        let CompletionResponse::Array(items) = completion else {
            panic!("variant completion should resolve to a plain item array")
        };
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].label, "Backoff");
        assert_eq!(items[0].kind, Some(CompletionItemKind::ENUM_MEMBER));
        assert_eq!(
            items[0].detail.as_deref(),
            Some("variant Command.Backoff(Int)")
        );
    }

    #[test]
    fn same_named_local_dependency_broken_source_struct_field_completion_prefers_matching_dependency_source()
     {
        let temp = TempDir::new(
            "ql-lsp-same-named-local-dependency-broken-source-struct-field-completion",
        );
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.shared.alpha.Settings as Settings
use demo.shared.beta.Settings as OtherSettings

pub fn main() -> Int {
    let value = Settings { po
"#,
        );
        temp.write(
            "workspace/vendor/alpha/src/lib.ql",
            r#"
package demo.shared.alpha

pub struct Settings {
    host: String,
    port: Int,
}
"#,
        );
        temp.write(
            "workspace/vendor/beta/src/lib.ql",
            r#"
package demo.shared.beta

pub struct Settings {
    host: String,
    block: Bool,
}
"#,
        );
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

pub struct Settings {
    host: String,
    port: Int,
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

pub struct Settings {
    host: String,
    block: Bool,
}
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&source).is_err());
        let package = package_analysis_for_path(&app_path)
            .expect("package analysis should survive parse errors");
        let completion = completion_for_dependency_struct_fields(
            &source,
            &package,
            offset_to_position(&source, nth_offset(&source, "po", 1) + 2),
        )
        .expect("broken-source same-named dependency struct field completion should exist");

        let CompletionResponse::Array(items) = completion else {
            panic!("struct field completion should resolve to a plain item array")
        };
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].label, "port");
        assert_eq!(items[0].kind, Some(CompletionItemKind::FIELD));
        assert_eq!(items[0].detail.as_deref(), Some("field port: Int"));
    }

    #[test]
    fn same_named_local_dependency_workspace_member_completion_prefers_matching_dependency_source_over_stale_interface()
     {
        let temp = TempDir::new(
            "ql-lsp-same-named-local-dependency-workspace-member-completion-source-preferred",
        );
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.shared.alpha.build as build
use demo.shared.beta.build as other

pub fn main() -> Int {
    return build().pi() + build().to + other().pong() + other().block
}
"#,
        );
        temp.write(
            "workspace/vendor/alpha/src/lib.ql",
            r#"
package demo.shared.alpha

pub struct Counter {
    total: Int,
    value: Int,
}

impl Counter {
    pub fn ping(self) -> Int {
        return self.total
    }
}

pub fn build() -> Counter {
    return Counter { total: 1, value: 2 }
}
"#,
        );
        temp.write(
            "workspace/vendor/beta/src/lib.ql",
            r#"
package demo.shared.beta

pub struct Counter {
    block: Int,
    value: Int,
}

impl Counter {
    pub fn pong(self) -> Int {
        return self.block
    }
}

pub fn build() -> Counter {
    return Counter { block: 3, value: 4 }
}
"#,
        );
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

pub struct Counter {
    count: Int,
    value: Int,
}

impl Counter {
    pub fn paint(self) -> Int
}

pub fn build() -> Counter
"#,
        );
        temp.write(
            "workspace/vendor/beta/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.shared.beta

pub struct Counter {
    bonus: Int,
    value: Int,
}

impl Counter {
    pub fn pop(self) -> Int
}

pub fn build() -> Counter
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&source).is_ok());
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");

        let method_completion = workspace_source_method_completions(
            &source,
            &package,
            offset_to_position(&source, nth_offset(&source, "pi", 1) + 2),
        )
        .expect("workspace same-named dependency method completion should exist");
        let CompletionResponse::Array(method_items) = method_completion else {
            panic!("method completion should resolve to a plain item array")
        };
        assert_eq!(method_items.len(), 1);
        assert_eq!(method_items[0].label, "ping");
        assert_eq!(method_items[0].kind, Some(CompletionItemKind::METHOD));
        assert_eq!(
            method_items[0].detail.as_deref(),
            Some("fn ping(self) -> Int")
        );

        let field_completion = workspace_source_member_field_completions(
            &source,
            &package,
            offset_to_position(&source, nth_offset(&source, "to", 1) + 2),
        )
        .expect("workspace same-named dependency field completion should exist");
        let CompletionResponse::Array(field_items) = field_completion else {
            panic!("field completion should resolve to a plain item array")
        };
        assert_eq!(field_items.len(), 1);
        assert_eq!(field_items[0].label, "total");
        assert_eq!(field_items[0].kind, Some(CompletionItemKind::FIELD));
        assert_eq!(field_items[0].detail.as_deref(), Some("field total: Int"));
    }

    #[test]
    fn same_named_local_dependency_workspace_variant_and_struct_field_completion_prefer_matching_dependency_source_over_stale_interface()
     {
        let temp = TempDir::new(
            "ql-lsp-same-named-local-dependency-workspace-variant-struct-field-completion-source-preferred",
        );
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.shared.alpha.Command as Cmd
use demo.shared.beta.Command as OtherCmd
use demo.shared.alpha.Settings as Settings
use demo.shared.beta.Settings as OtherSettings

pub fn main() -> Int {
    let first = Cmd.B(1)
    let settings = Settings { host: "localhost", po: 1 }
    let other = OtherCmd.Block(1)
    let second = OtherSettings { host: "localhost", block: true }
    return first + settings.port + other + second.block
}
"#,
        );
        temp.write(
            "workspace/vendor/alpha/src/lib.ql",
            r#"
package demo.shared.alpha

pub enum Command {
    Retry(Int),
    Backoff(Int),
}

pub struct Settings {
    host: String,
    port: Int,
}
"#,
        );
        temp.write(
            "workspace/vendor/beta/src/lib.ql",
            r#"
package demo.shared.beta

pub enum Command {
    Retry(Int),
    Block(Int),
}

pub struct Settings {
    host: String,
    block: Bool,
}
"#,
        );
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
    Bounce(Int),
}

pub struct Settings {
    host: String,
    priority: Int,
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
    Barrier(Int),
}

pub struct Settings {
    host: String,
    branch: Bool,
}
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&source).is_ok());
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");

        let variant_completion = workspace_source_variant_completions(
            &source,
            &package,
            offset_to_position(&source, nth_offset(&source, "B", 1) + 1),
        )
        .expect("workspace same-named dependency variant completion should exist");
        let CompletionResponse::Array(variant_items) = variant_completion else {
            panic!("variant completion should resolve to a plain item array")
        };
        assert_eq!(variant_items.len(), 1);
        assert_eq!(variant_items[0].label, "Backoff");
        assert_eq!(variant_items[0].kind, Some(CompletionItemKind::ENUM_MEMBER));
        assert_eq!(
            variant_items[0].detail.as_deref(),
            Some("variant Command.Backoff(Int)")
        );

        let field_completion = workspace_source_struct_field_completions(
            &source,
            &package,
            offset_to_position(&source, nth_offset(&source, "po", 1) + 2),
        )
        .expect("workspace same-named dependency struct field completion should exist");
        let CompletionResponse::Array(field_items) = field_completion else {
            panic!("struct field completion should resolve to a plain item array")
        };
        assert_eq!(field_items.len(), 1);
        assert_eq!(field_items[0].label, "port");
        assert_eq!(field_items[0].kind, Some(CompletionItemKind::FIELD));
        assert_eq!(field_items[0].detail.as_deref(), Some("field port: Int"));
    }

    #[test]
    fn workspace_dependency_queries_use_unsaved_open_local_dependency_source() {
        let temp = TempDir::new("ql-lsp-workspace-dependency-open-doc-queries");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.shared.alpha.build as build

pub fn main() -> Int {
    return build().ping()
}
"#,
        );
        let alpha_source_path = temp.write(
            "workspace/vendor/alpha/src/lib.ql",
            r#"
package demo.shared.alpha

pub struct Counter {
    value: Int,
}

impl Counter {
    pub fn ping(self) -> Int {
        return self.value
    }
}

pub fn build() -> Counter {
    return Counter { value: 1 }
}
"#,
        );
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
            "workspace/vendor/alpha/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.shared.alpha

pub struct Counter {
    value: Int,
}

impl Counter {
    pub fn ping(self) -> Int
}

pub fn build() -> Counter
"#,
        );

        let open_alpha_source = r#"
package demo.shared.alpha

pub struct Counter {
    value: Int,
}


impl Counter {
    pub fn ping(self) -> Int {
        return self.value
    }
}

pub fn forward(counter: Counter) -> Int {
    return counter.ping()
}

pub fn build() -> Counter {
    return Counter { value: 1 }
}
"#;
        let source = fs::read_to_string(&app_path).expect("app source should read");
        let analysis = analyze_source(&source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let alpha_uri =
            Url::from_file_path(&alpha_source_path).expect("alpha path should convert to URI");
        let open_docs =
            file_open_documents(vec![(alpha_uri.clone(), open_alpha_source.to_owned())]);

        let definition = workspace_source_definition_for_dependency_with_open_docs(
            &uri,
            &source,
            Some(&analysis),
            &package,
            &open_docs,
            offset_to_position(&source, nth_offset(&source, "ping", 1) + 1),
        )
        .expect("dependency definition should use open dependency source");
        let GotoDefinitionResponse::Scalar(location) = definition else {
            panic!("dependency definition should resolve to a scalar source location")
        };
        assert_eq!(location.uri, alpha_uri);
        assert_eq!(
            location.range.start,
            offset_to_position(open_alpha_source, nth_offset(open_alpha_source, "ping", 1)),
        );

        let references = workspace_source_references_for_dependency_with_open_docs(
            &uri,
            &source,
            Some(&analysis),
            &package,
            &open_docs,
            offset_to_position(&source, nth_offset(&source, "ping", 1) + 1),
            true,
        )
        .expect("dependency references should use open dependency source");

        assert!(
            references.iter().any(|reference| {
                reference.uri == alpha_uri
                    && reference.range.start
                        == offset_to_position(
                            open_alpha_source,
                            nth_offset(open_alpha_source, "ping", 1),
                        )
            }),
            "references should include open dependency source definition",
        );
        assert!(
            references.iter().any(|reference| {
                reference.uri == alpha_uri
                    && reference.range.start
                        == offset_to_position(
                            open_alpha_source,
                            nth_offset(open_alpha_source, "ping", 2),
                        )
            }),
            "references should include open dependency source method use",
        );
    }

    #[test]
    fn workspace_dependency_method_completion_uses_unsaved_open_local_dependency_source() {
        let temp = TempDir::new("ql-lsp-workspace-dependency-open-doc-method-completion");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.shared.alpha.build as build

pub fn main() -> Int {
    return build().pu()
}
"#,
        );
        let alpha_source_path = temp.write(
            "workspace/vendor/alpha/src/lib.ql",
            r#"
package demo.shared.alpha

pub struct Counter {
    value: Int,
}

impl Counter {
    pub fn ping(self) -> Int {
        return self.value
    }
}

pub fn build() -> Counter {
    return Counter { value: 1 }
}
"#,
        );
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
            "workspace/vendor/alpha/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.shared.alpha

pub struct Counter {
    value: Int,
}

impl Counter {
    pub fn ping(self) -> Int
}

pub fn build() -> Counter
"#,
        );

        let open_alpha_source = r#"
package demo.shared.alpha

pub struct Counter {
    value: Int,
}

impl Counter {
    pub fn pulse(self) -> Int {
        return self.value
    }
}

pub fn build() -> Counter {
    return Counter { value: 1 }
}
"#;
        let source = fs::read_to_string(&app_path).expect("app source should read");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let alpha_uri =
            Url::from_file_path(&alpha_source_path).expect("alpha path should convert to URI");
        let open_docs = file_open_documents(vec![(alpha_uri, open_alpha_source.to_owned())]);
        let offset = nth_offset(&source, "build().pu", 1) + "build().pu".len();

        let completion = workspace_source_method_completions_with_open_docs(
            &source,
            &package,
            &open_docs,
            offset_to_position(&source, offset),
        )
        .expect("method completion should use open dependency source");

        let CompletionResponse::Array(items) = completion else {
            panic!("method completion should resolve to a plain item array")
        };
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].label, "pulse");
        assert_eq!(items[0].kind, Some(CompletionItemKind::METHOD));
        assert_eq!(items[0].detail.as_deref(), Some("fn pulse(self) -> Int"));
    }

    #[test]
    fn workspace_dependency_definition_and_hover_prefer_open_local_dependency_members() {
        let temp = TempDir::new("ql-lsp-workspace-dependency-open-doc-member-navigation");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.shared.alpha.build as build

pub fn main() -> Int {
    return build().pulse()
}
"#,
        );
        let alpha_source_path = temp.write(
            "workspace/vendor/alpha/src/lib.ql",
            r#"
package demo.shared.alpha

pub struct Counter {
    value: Int,
}

impl Counter {
    pub fn ping(self) -> Int {
        return self.value
    }
}

pub fn build() -> Counter {
    return Counter { value: 1 }
}
"#,
        );
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
            "workspace/vendor/alpha/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.shared.alpha

pub struct Counter {
    value: Int,
}

impl Counter {
    pub fn ping(self) -> Int
}

pub fn build() -> Counter
"#,
        );

        let open_alpha_source = r#"
package demo.shared.alpha

pub struct Counter {
    value: Int,
}

impl Counter {
    pub fn pulse(self) -> Int {
        return self.value
    }
}

pub fn build() -> Counter {
    return Counter { value: 1 }
}
"#;
        let source = fs::read_to_string(&app_path).expect("app source should read");
        let analysis = analyze_source(&source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let alpha_uri =
            Url::from_file_path(&alpha_source_path).expect("alpha path should convert to URI");
        let open_docs =
            file_open_documents(vec![(alpha_uri.clone(), open_alpha_source.to_owned())]);
        let pulse_position = offset_to_position(&source, nth_offset(&source, "pulse", 1) + 1);

        assert_eq!(
            workspace_source_definition_for_dependency(
                &uri,
                &source,
                Some(&analysis),
                &package,
                pulse_position,
            ),
            None,
            "disk-only definition should miss unsaved dependency members",
        );

        let definition = workspace_source_definition_for_dependency_with_open_docs(
            &uri,
            &source,
            Some(&analysis),
            &package,
            &open_docs,
            pulse_position,
        )
        .expect("dependency definition should use open dependency member source");
        let GotoDefinitionResponse::Scalar(location) = definition else {
            panic!("dependency definition should resolve to a scalar source location")
        };
        assert_eq!(location.uri, alpha_uri);
        assert_eq!(
            location.range.start,
            offset_to_position(open_alpha_source, nth_offset(open_alpha_source, "pulse", 1)),
        );

        assert_eq!(
            workspace_source_hover_for_dependency(
                &uri,
                &source,
                Some(&analysis),
                &package,
                pulse_position,
            ),
            None,
            "disk-only hover should miss unsaved dependency members",
        );

        let hover = workspace_source_hover_for_dependency_with_open_docs(
            &uri,
            &source,
            Some(&analysis),
            &package,
            &open_docs,
            pulse_position,
        )
        .expect("dependency hover should use open dependency member source");
        let HoverContents::Markup(markup) = hover.contents else {
            panic!("hover should use markdown")
        };
        assert!(markup.value.contains("fn pulse(self) -> Int"));
        assert!(!markup.value.contains("fn ping(self) -> Int"));

        assert_eq!(
            workspace_source_method_implementation_for_dependency_with_open_docs(
                &uri,
                &source,
                Some(&analysis),
                &package,
                &open_docs,
                pulse_position,
            ),
            Some(GotoImplementationResponse::Scalar(Location::new(
                alpha_uri,
                span_to_range(open_alpha_source, nth_span(open_alpha_source, "pulse", 1)),
            ))),
        );
    }

    struct WorkspaceDependencyTraitMethodImplementationFixture {
        _temp: TempDir,
        app_source: String,
        app_uri: Url,
        package: ql_analysis::PackageAnalysis,
        tools_source: String,
        tools_uri: Url,
        bots_source: Option<String>,
        bots_uri: Option<Url>,
    }

    fn setup_workspace_dependency_trait_method_implementation_fixture(
        prefix: &str,
        app_source: &str,
        tools_source: &str,
        bots_source: Option<&str>,
    ) -> WorkspaceDependencyTraitMethodImplementationFixture {
        let temp = TempDir::new(prefix);
        let app_path = temp.write("workspace/packages/app/src/main.ql", app_source);
        temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub trait Runner {
    fn run(self) -> Int
}
"#,
        );
        let tools_source_path = temp.write("workspace/packages/tools/src/lib.ql", tools_source);
        let bots_source_path = bots_source
            .map(|source| temp.write("workspace/packages/bots/src/lib.ql", source));

        temp.write(
            "workspace/qlang.toml",
            if bots_source_path.is_some() {
                r#"
[workspace]
members = ["packages/app", "packages/bots", "packages/core", "packages/tools"]
"#
            } else {
                r#"
[workspace]
members = ["packages/app", "packages/core", "packages/tools"]
"#
            },
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/tools/qlang.toml",
            r#"
[package]
name = "tools"

[references]
packages = ["../core"]
"#,
        );
        if bots_source_path.is_some() {
            temp.write(
                "workspace/packages/bots/qlang.toml",
                r#"
[package]
name = "bots"

[references]
packages = ["../core"]
"#,
            );
        }
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub trait Runner {
    fn run(self) -> Int
}
"#,
        );

        let app_source = fs::read_to_string(&app_path).expect("app source should read");
        let app_uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let package = package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let tools_source = fs::read_to_string(&tools_source_path).expect("tools source should read");
        let tools_uri =
            Url::from_file_path(&tools_source_path).expect("tools path should convert to URI");
        let bots_source = bots_source_path
            .as_ref()
            .map(|path| fs::read_to_string(path).expect("bots source should read"));
        let bots_uri = bots_source_path
            .map(|path| Url::from_file_path(path).expect("bots path should convert to URI"));

        WorkspaceDependencyTraitMethodImplementationFixture {
            _temp: temp,
            app_source,
            app_uri,
            package,
            tools_source,
            tools_uri,
            bots_source,
            bots_uri,
        }
    }

    #[test]
    fn workspace_dependency_trait_method_call_implementation_aggregates_workspace_impl_methods() {
        let fixture = setup_workspace_dependency_trait_method_implementation_fixture(
            "ql-lsp-workspace-dependency-trait-method-call-implementation",
            r#"
package demo.app

use demo.core.Runner

pub fn main(runner: Runner) -> Int {
    return runner.run()
}
"#,
            r#"
package demo.tools

use demo.core.Runner

struct ToolWorker {}

impl Runner for ToolWorker {
    fn run(self) -> Int {
        return 2
    }
}
"#,
            Some(
                r#"
package demo.bots

use demo.core.Runner

struct BotWorker {}

impl Runner for BotWorker {
    fn run(self) -> Int {
        return 3
    }
}
"#,
            ),
        );
        let analysis =
            analyze_source(&fixture.app_source).expect("app source should analyze");

        let implementation = workspace_source_method_implementation_for_dependency_with_open_docs(
            &fixture.app_uri,
            &fixture.app_source,
            Some(&analysis),
            &fixture.package,
            &file_open_documents(vec![]),
            offset_to_position(
                &fixture.app_source,
                nth_offset_in_context(&fixture.app_source, "run", "runner.run()", 1),
            ),
        )
        .expect("dependency trait method call implementation should exist");

        let GotoDefinitionResponse::Array(locations) = implementation else {
            panic!("visible dependency trait impls should resolve to many locations")
        };
        assert_eq!(locations.len(), 2);

        assert!(locations.contains(&Location::new(
            fixture.tools_uri.clone(),
            span_to_range(
                &fixture.tools_source,
                nth_span_in_context(&fixture.tools_source, "run", "fn run(self)", 1),
            ),
        )));

        let bots_source = fixture
            .bots_source
            .as_ref()
            .expect("bots source should exist");
        let bots_uri = fixture.bots_uri.clone().expect("bots URI should exist");
        assert!(locations.contains(&Location::new(
            bots_uri,
            span_to_range(
                bots_source,
                nth_span_in_context(bots_source, "run", "fn run(self)", 1),
            ),
        )));
    }

    #[test]
    fn workspace_dependency_trait_method_call_implementation_prefers_open_workspace_impl_methods()
    {
        let fixture = setup_workspace_dependency_trait_method_implementation_fixture(
            "ql-lsp-workspace-dependency-trait-method-call-open-docs",
            r#"
package demo.app

use demo.core.Runner

pub fn main(runner: Runner) -> Int {
    return runner.run()
}
"#,
            r#"
package demo.tools

use demo.core.Runner

struct ToolWorker {}

impl Runner for ToolWorker {
    fn walk(self) -> Int {
        return 2
    }
}
"#,
            None,
        );
        let open_tools_source = r#"
package demo.tools

use demo.core.Runner

struct ToolWorker {}

impl Runner for ToolWorker {
    fn run(self) -> Int {
        return 2
    }
}
"#
        .to_owned();
        let analysis =
            analyze_source(&fixture.app_source).expect("app source should analyze");

        let implementation = workspace_source_method_implementation_for_dependency_with_open_docs(
            &fixture.app_uri,
            &fixture.app_source,
            Some(&analysis),
            &fixture.package,
            &file_open_documents(vec![(
                fixture.tools_uri.clone(),
                open_tools_source.clone(),
            )]),
            offset_to_position(
                &fixture.app_source,
                nth_offset_in_context(&fixture.app_source, "run", "runner.run()", 1),
            ),
        )
        .expect("dependency trait method call should use open workspace impl methods");

        let GotoDefinitionResponse::Scalar(location) = implementation else {
            panic!("single open-doc dependency trait impl should resolve to one location")
        };
        assert_eq!(location.uri, fixture.tools_uri);
        assert_eq!(
            location.range.start,
            offset_to_position(
                &open_tools_source,
                nth_offset_in_context(&open_tools_source, "run", "fn run(self)", 1),
            ),
        );
    }

    #[test]
    fn workspace_dependency_member_type_definitions_prefer_open_local_dependency_members() {
        let temp = TempDir::new("ql-lsp-workspace-dependency-open-doc-member-type-definitions");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.shared.alpha.build as build

pub fn main() -> Int {
    let current = build()
    return current.extra.id + current.pulse().id
}
"#,
        );
        let alpha_source_path = temp.write(
            "workspace/vendor/alpha/src/lib.ql",
            r#"
package demo.shared.alpha

pub struct Counter {
    value: Int,
}

impl Counter {
    pub fn ping(self) -> Int {
        return self.value
    }
}

pub fn build() -> Counter {
    return Counter { value: 1 }
}
"#,
        );
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
            "workspace/vendor/alpha/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.shared.alpha

pub struct Counter {
    value: Int,
}

impl Counter {
    pub fn ping(self) -> Int
}

pub fn build() -> Counter
"#,
        );

        let open_alpha_source = r#"
package demo.shared.alpha

pub struct Extra {
    id: Int,
}

pub struct Counter {
    value: Int,
    extra: Extra,
}

impl Counter {
    pub fn pulse(self) -> Extra {
        return self.extra
    }
}

pub fn build() -> Counter {
    return Counter { value: 1, extra: Extra { id: 2 } }
}
"#;
        let source = fs::read_to_string(&app_path).expect("app source should read");
        let analysis = analyze_source(&source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let alpha_uri =
            Url::from_file_path(&alpha_source_path).expect("alpha path should convert to URI");
        let open_docs =
            file_open_documents(vec![(alpha_uri.clone(), open_alpha_source.to_owned())]);

        for (needle, occurrence) in [("extra", 1usize), ("pulse", 1usize)] {
            let position = offset_to_position(&source, nth_offset(&source, needle, occurrence) + 1);
            assert_eq!(
                workspace_source_type_definition_for_dependency(
                    &uri,
                    &source,
                    Some(&analysis),
                    &package,
                    position,
                ),
                None,
                "disk-only type definition should miss unsaved dependency member {needle}",
            );

            let type_definition = workspace_source_type_definition_for_dependency_with_open_docs(
                &uri,
                &source,
                Some(&analysis),
                &package,
                &open_docs,
                position,
            )
            .expect("dependency member type definition should use open dependency source");
            let GotoTypeDefinitionResponse::Scalar(location) = type_definition else {
                panic!(
                    "dependency member type definition should resolve to a scalar source location"
                )
            };
            assert_eq!(location.uri, alpha_uri);
            assert_eq!(
                location.range.start,
                offset_to_position(open_alpha_source, nth_offset(open_alpha_source, "Extra", 1)),
            );
        }
    }

    #[test]
    fn workspace_dependency_member_type_implementation_prefers_open_local_dependency_members() {
        let temp = TempDir::new("ql-lsp-workspace-dependency-open-doc-member-implementation");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.shared.alpha.build as build

pub fn main() -> Int {
    let current = build()
    return current.extra.id + current.pulse().id
}
"#,
        );
        let alpha_source_path = temp.write(
            "workspace/vendor/alpha/src/lib.ql",
            r#"
package demo.shared.alpha

pub struct Counter {
    value: Int,
}

impl Counter {
    pub fn ping(self) -> Int {
        return self.value
    }
}

pub fn build() -> Counter {
    return Counter { value: 1 }
}
"#,
        );
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
            "workspace/vendor/alpha/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.shared.alpha

pub struct Counter {
    value: Int,
}

impl Counter {
    pub fn ping(self) -> Int
}

pub fn build() -> Counter
"#,
        );

        let open_alpha_source = r#"
package demo.shared.alpha

pub struct Extra {
    id: Int,
}

impl Extra {
    pub fn read(self) -> Int {
        return self.id
    }
}

extend Extra {
    pub fn bonus(self) -> Int {
        return self.id + 1
    }
}

pub struct Counter {
    value: Int,
    extra: Extra,
}

impl Counter {
    pub fn pulse(self) -> Extra {
        return self.extra
    }
}

pub fn build() -> Counter {
    return Counter { value: 1, extra: Extra { id: 2 } }
}
"#;
        let source = fs::read_to_string(&app_path).expect("app source should read");
        let analysis = analyze_source(&source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let alpha_uri =
            Url::from_file_path(&alpha_source_path).expect("alpha path should convert to URI");
        let open_docs =
            file_open_documents(vec![(alpha_uri.clone(), open_alpha_source.to_owned())]);

        for (needle, occurrence) in [("extra", 1usize), ("pulse", 1usize)] {
            let position = offset_to_position(&source, nth_offset(&source, needle, occurrence) + 1);
            assert_eq!(
                workspace_source_implementation_for_dependency_with_open_docs(
                    &source,
                    Some(&analysis),
                    &package,
                    &file_open_documents(vec![]),
                    position,
                ),
                None,
                "disk-only implementation should miss unsaved dependency member type {needle}",
            );

            let implementation = workspace_source_implementation_for_dependency_with_open_docs(
                &source,
                Some(&analysis),
                &package,
                &open_docs,
                position,
            )
            .expect("dependency member type implementation should use open dependency source");
            let GotoImplementationResponse::Array(locations) = implementation else {
                panic!(
                    "dependency member type implementation should resolve to dependency impl blocks"
                )
            };
            assert_eq!(locations.len(), 2);
            assert!(
                locations.iter().all(|location| location.uri == alpha_uri),
                "implementation should stay in the open dependency source",
            );
            for marker in ["impl Extra", "extend Extra"] {
                assert!(
                    locations.iter().any(|location| {
                        location.range.start
                            == offset_to_position(
                                open_alpha_source,
                                nth_offset(open_alpha_source, marker, 1),
                            )
                    }),
                    "dependency member type implementation should include {marker} for {needle}",
                );
            }
        }
    }

    #[test]
    fn workspace_dependency_member_semantic_tokens_prefer_open_local_dependency_members() {
        let temp = TempDir::new("ql-lsp-workspace-dependency-open-doc-member-semantic-tokens");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.shared.alpha.build as build

pub fn main() -> Int {
    let current = build()
    return current.extra.id + current.pulse().id
}
"#,
        );
        let alpha_source_path = temp.write(
            "workspace/vendor/alpha/src/lib.ql",
            r#"
package demo.shared.alpha

pub struct Counter {
    value: Int,
}

impl Counter {
    pub fn ping(self) -> Int {
        return self.value
    }
}

pub fn build() -> Counter {
    return Counter { value: 1 }
}
"#,
        );
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
            "workspace/vendor/alpha/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.shared.alpha

pub struct Counter {
    value: Int,
}

impl Counter {
    pub fn ping(self) -> Int
}

pub fn build() -> Counter
"#,
        );

        let open_alpha_source = r#"
package demo.shared.alpha

pub struct Extra {
    id: Int,
}

pub struct Counter {
    value: Int,
    extra: Extra,
}

impl Counter {
    pub fn pulse(self) -> Extra {
        return self.extra
    }
}

pub fn build() -> Counter {
    return Counter { value: 1, extra: Extra { id: 2 } }
}
"#;
        let source = fs::read_to_string(&app_path).expect("app source should read");
        let analysis = analyze_source(&source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let alpha_uri =
            Url::from_file_path(&alpha_source_path).expect("alpha path should convert to URI");

        let SemanticTokensResult::Tokens(disk_tokens) =
            semantic_tokens_for_workspace_package_analysis(&uri, &source, &analysis, &package)
        else {
            panic!("expected full semantic tokens")
        };
        let disk_decoded = decode_semantic_tokens(&disk_tokens.data);

        let SemanticTokensResult::Tokens(tokens) =
            semantic_tokens_for_workspace_package_analysis_with_open_docs(
                &uri,
                &source,
                &analysis,
                &package,
                &file_open_documents(vec![
                    (uri.clone(), source.clone()),
                    (alpha_uri, open_alpha_source.to_owned()),
                ]),
            )
        else {
            panic!("expected full semantic tokens")
        };
        let decoded = decode_semantic_tokens(&tokens.data);
        let legend = semantic_tokens_legend();
        let property_type = legend
            .token_types
            .iter()
            .position(|token_type| *token_type == SemanticTokenType::PROPERTY)
            .expect("property legend entry should exist") as u32;
        let method_type = legend
            .token_types
            .iter()
            .position(|token_type| *token_type == SemanticTokenType::METHOD)
            .expect("method legend entry should exist") as u32;

        for (needle, occurrence, token_type) in [
            ("extra", 1usize, property_type),
            ("pulse", 1usize, method_type),
        ] {
            let span = Span::new(
                nth_offset(&source, needle, occurrence),
                nth_offset(&source, needle, occurrence) + needle.len(),
            );
            let range = span_to_range(&source, span);
            let token = (
                range.start.line,
                range.start.character,
                range.end.character - range.start.character,
                token_type,
            );
            assert!(
                !disk_decoded.contains(&token),
                "disk-only semantic tokens should miss unsaved dependency member {needle}",
            );
            assert!(
                decoded.contains(&token),
                "open-doc semantic tokens should include dependency member {needle}",
            );
        }
    }

    #[test]
    fn workspace_dependency_references_and_highlights_prefer_open_local_dependency_members() {
        let temp = TempDir::new("ql-lsp-workspace-dependency-open-doc-member-references");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.shared.alpha.build as build

pub fn main() -> Int {
    return build().pulse()
}
"#,
        );
        let alpha_source_path = temp.write(
            "workspace/vendor/alpha/src/lib.ql",
            r#"
package demo.shared.alpha

pub struct Counter {
    value: Int,
}

impl Counter {
    pub fn ping(self) -> Int {
        return self.value
    }
}

pub fn build() -> Counter {
    return Counter { value: 1 }
}
"#,
        );
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
            "workspace/vendor/alpha/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.shared.alpha

pub struct Counter {
    value: Int,
}

impl Counter {
    pub fn ping(self) -> Int
}

pub fn build() -> Counter
"#,
        );

        let open_alpha_source = r#"
package demo.shared.alpha

pub struct Counter {
    value: Int,
}

impl Counter {
    pub fn pulse(self) -> Int {
        return self.value
    }
}

pub fn forward(counter: Counter) -> Int {
    return counter.pulse()
}

pub fn build() -> Counter {
    return Counter { value: 1 }
}
"#;
        let source = fs::read_to_string(&app_path).expect("app source should read");
        let analysis = analyze_source(&source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let alpha_uri =
            Url::from_file_path(&alpha_source_path).expect("alpha path should convert to URI");
        let open_docs =
            file_open_documents(vec![(alpha_uri.clone(), open_alpha_source.to_owned())]);
        let pulse_position = offset_to_position(&source, nth_offset(&source, "pulse", 1) + 1);

        assert_eq!(
            workspace_source_references_for_dependency(
                &uri,
                &source,
                Some(&analysis),
                &package,
                pulse_position,
                true,
            ),
            None,
            "disk-only references should miss unsaved dependency members",
        );

        let references = workspace_source_references_for_dependency_with_open_docs(
            &uri,
            &source,
            Some(&analysis),
            &package,
            &open_docs,
            pulse_position,
            true,
        )
        .expect("dependency references should use open dependency member source");
        assert!(
            references.iter().any(|reference| {
                reference.uri == alpha_uri
                    && reference.range.start
                        == offset_to_position(
                            open_alpha_source,
                            nth_offset(open_alpha_source, "pulse", 1),
                        )
            }),
            "references should include open dependency source definition",
        );
        assert!(
            references.iter().any(|reference| {
                reference.uri == alpha_uri
                    && reference.range.start
                        == offset_to_position(
                            open_alpha_source,
                            nth_offset(open_alpha_source, "pulse", 2),
                        )
            }),
            "references should include open dependency source member use",
        );
        assert!(
            references.iter().any(|reference| {
                reference.uri == uri
                    && reference.range.start
                        == offset_to_position(&source, nth_offset(&source, "pulse", 1))
            }),
            "references should include current source member use",
        );

        let highlights = fallback_document_highlights_for_package_at_with_open_docs(
            &uri,
            &source,
            &package,
            pulse_position,
            &open_docs,
        )
        .expect("document highlights should use open dependency member source");
        assert_eq!(highlights.len(), 1);
        assert_eq!(
            highlights[0].range.start,
            offset_to_position(&source, nth_offset(&source, "pulse", 1)),
        );
    }

    #[test]
    fn workspace_dependency_broken_source_queries_use_unsaved_open_local_dependency_source() {
        let temp = TempDir::new("ql-lsp-workspace-dependency-open-doc-broken-queries");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.shared.alpha.build as build

pub fn main() -> Int {
    return build().ping()
"#,
        );
        let alpha_source_path = temp.write(
            "workspace/vendor/alpha/src/lib.ql",
            r#"
package demo.shared.alpha

pub struct Counter {
    value: Int,
}

impl Counter {
    pub fn ping(self) -> Int {
        return self.value
    }
}

pub fn build() -> Counter {
    return Counter { value: 1 }
}
"#,
        );
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
            "workspace/vendor/alpha/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.shared.alpha

pub struct Counter {
    value: Int,
}

impl Counter {
    pub fn ping(self) -> Int
}

pub fn build() -> Counter
"#,
        );

        let open_alpha_source = r#"
package demo.shared.alpha

pub struct Counter {
    value: Int,
}

impl Counter {
    pub fn ping(self) -> Int {
        return self.value
    }
}

pub fn forward(counter: Counter) -> Int {
    return counter.ping()
}

pub fn build() -> Counter {
    return Counter { value: 1 }
}
"#;
        let source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&source).is_err());
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let alpha_uri =
            Url::from_file_path(&alpha_source_path).expect("alpha path should convert to URI");
        let open_docs =
            file_open_documents(vec![(alpha_uri.clone(), open_alpha_source.to_owned())]);
        let ping_position = offset_to_position(&source, nth_offset(&source, "ping", 1) + 1);

        let references =
            workspace_source_references_for_dependency_in_broken_source_with_open_docs(
                &uri,
                &source,
                &package,
                &open_docs,
                ping_position,
                true,
            )
            .expect("broken-source dependency references should use open dependency source");

        assert!(
            references.iter().any(|reference| {
                reference.uri == alpha_uri
                    && reference.range.start
                        == offset_to_position(
                            open_alpha_source,
                            nth_offset(open_alpha_source, "ping", 1),
                        )
            }),
            "references should include open dependency source definition",
        );
        assert!(
            references.iter().any(|reference| {
                reference.uri == alpha_uri
                    && reference.range.start
                        == offset_to_position(
                            open_alpha_source,
                            nth_offset(open_alpha_source, "ping", 2),
                        )
            }),
            "references should include open dependency source method use",
        );
        assert!(
            references.iter().any(|reference| {
                reference.uri == uri
                    && reference.range.start
                        == offset_to_position(&source, nth_offset(&source, "ping", 1))
            }),
            "references should include broken-source local method occurrence",
        );

        assert_eq!(
            workspace_source_method_implementation_for_dependency_with_open_docs(
                &uri,
                &source,
                None,
                &package,
                &open_docs,
                ping_position,
            ),
            Some(GotoImplementationResponse::Scalar(Location::new(
                alpha_uri.clone(),
                span_to_range(open_alpha_source, nth_span(open_alpha_source, "ping", 1)),
            ))),
        );

        let highlights = fallback_document_highlights_for_package_at_with_open_docs(
            &uri,
            &source,
            &package,
            ping_position,
            &open_docs,
        )
        .expect("broken-source document highlights should use open dependency source");
        assert_eq!(highlights.len(), 1);
        assert_eq!(
            highlights[0].range.start,
            offset_to_position(&source, nth_offset(&source, "ping", 1)),
        );
    }

    #[test]
    fn workspace_dependency_member_type_implementation_in_broken_source_prefers_open_local_dependency_members(
    ) {
        let temp =
            TempDir::new("ql-lsp-workspace-dependency-open-doc-member-implementation-broken");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.shared.alpha.build as build

pub fn main() -> Int {
    let current = build()
    return current.extra.id + current.pulse().id
"#,
        );
        let alpha_source_path = temp.write(
            "workspace/vendor/alpha/src/lib.ql",
            r#"
package demo.shared.alpha

pub struct Counter {
    value: Int,
}

impl Counter {
    pub fn ping(self) -> Int {
        return self.value
    }
}

pub fn build() -> Counter {
    return Counter { value: 1 }
}
"#,
        );
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
            "workspace/vendor/alpha/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.shared.alpha

pub struct Counter {
    value: Int,
}

impl Counter {
    pub fn ping(self) -> Int
}

pub fn build() -> Counter
"#,
        );

        let open_alpha_source = r#"
package demo.shared.alpha

pub struct Extra {
    id: Int,
}

impl Extra {
    pub fn read(self) -> Int {
        return self.id
    }
}

extend Extra {
    pub fn bonus(self) -> Int {
        return self.id + 1
    }
}

pub struct Counter {
    value: Int,
    extra: Extra,
}

impl Counter {
    pub fn pulse(self) -> Extra {
        return self.extra
    }
}

pub fn build() -> Counter {
    return Counter { value: 1, extra: Extra { id: 2 } }
}
"#;
        let source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&source).is_err());
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let alpha_uri =
            Url::from_file_path(&alpha_source_path).expect("alpha path should convert to URI");
        let open_docs =
            file_open_documents(vec![(alpha_uri.clone(), open_alpha_source.to_owned())]);

        for (needle, occurrence) in [("extra", 1usize), ("pulse", 1usize)] {
            let position = offset_to_position(&source, nth_offset(&source, needle, occurrence) + 1);
            assert_eq!(
                workspace_source_implementation_for_dependency_with_open_docs(
                    &source,
                    None,
                    &package,
                    &file_open_documents(vec![]),
                    position,
                ),
                None,
                "disk-only broken-source implementation should miss unsaved dependency member type {needle}",
            );

            let implementation = workspace_source_implementation_for_dependency_with_open_docs(
                &source,
                None,
                &package,
                &open_docs,
                position,
            )
            .expect("broken-source dependency member type implementation should use open source");
            let GotoImplementationResponse::Array(locations) = implementation else {
                panic!(
                    "broken-source dependency member type implementation should resolve to dependency impl blocks"
                )
            };
            assert_eq!(locations.len(), 2);
            assert!(
                locations.iter().all(|location| location.uri == alpha_uri),
                "implementation should stay in the open dependency source",
            );
            for marker in ["impl Extra", "extend Extra"] {
                assert!(
                    locations.iter().any(|location| {
                        location.range.start
                            == offset_to_position(
                                open_alpha_source,
                                nth_offset(open_alpha_source, marker, 1),
                            )
                    }),
                    "broken-source dependency member type implementation should include {marker} for {needle}",
                );
            }
        }
    }

    #[test]
    fn workspace_dependency_trait_method_call_implementation_in_broken_source_prefers_open_workspace_impl_methods(
    ) {
        let fixture = setup_workspace_dependency_trait_method_implementation_fixture(
            "ql-lsp-workspace-dependency-trait-method-implementation-broken-open",
            r#"
package demo.app

use demo.core.Runner

pub fn main(runner: Runner) -> Int {
    return runner.run(
}
"#,
            r#"
package demo.tools

use demo.core.Runner

struct ToolWorker {}

impl Runner for ToolWorker {
    fn walk(self) -> Int {
        return 2
    }
}
"#,
            None,
        );
        let open_tools_source = r#"
package demo.tools

use demo.core.Runner

struct ToolWorker {}

impl Runner for ToolWorker {
    fn run(self) -> Int {
        return 2
    }
}
"#
        .to_owned();
        assert!(analyze_source(&fixture.app_source).is_err());

        let implementation = workspace_source_method_implementation_for_broken_source_with_open_docs(
            &fixture.app_uri,
            &fixture.app_source,
            &fixture.package,
            &file_open_documents(vec![(
                fixture.tools_uri.clone(),
                open_tools_source.clone(),
            )]),
            offset_to_position(
                &fixture.app_source,
                nth_offset_in_context(&fixture.app_source, "run", "runner.run(", 1),
            ),
        )
        .expect("broken dependency trait call should use open workspace impl methods");

        let GotoImplementationResponse::Scalar(location) = implementation else {
            panic!("single broken dependency trait impl should resolve to one location")
        };
        assert_eq!(location.uri, fixture.tools_uri);
        assert_eq!(
            location.range.start,
            offset_to_position(
                &open_tools_source,
                nth_offset_in_context(&open_tools_source, "run", "fn run(self)", 1),
            ),
        );
    }

    #[test]
    fn workspace_dependency_broken_source_method_completion_uses_unsaved_open_local_dependency_source()
     {
        let temp = TempDir::new("ql-lsp-workspace-dependency-open-doc-broken-method-completion");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.shared.alpha.build as build

pub fn main() -> Int {
    return build().pu(
"#,
        );
        let alpha_source_path = temp.write(
            "workspace/vendor/alpha/src/lib.ql",
            r#"
package demo.shared.alpha

pub struct Counter {
    value: Int,
}

impl Counter {
    pub fn ping(self) -> Int {
        return self.value
    }
}

pub fn build() -> Counter {
    return Counter { value: 1 }
}
"#,
        );
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
            "workspace/vendor/alpha/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.shared.alpha

pub struct Counter {
    value: Int,
}

impl Counter {
    pub fn ping(self) -> Int
}

pub fn build() -> Counter
"#,
        );

        let open_alpha_source = r#"
package demo.shared.alpha

pub struct Counter {
    value: Int,
}

impl Counter {
    pub fn pulse(self) -> Int {
        return self.value
    }
}

pub fn build() -> Counter {
    return Counter { value: 1 }
}
"#;
        let source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&source).is_err());
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let alpha_uri =
            Url::from_file_path(&alpha_source_path).expect("alpha path should convert to URI");
        let open_docs = file_open_documents(vec![(alpha_uri, open_alpha_source.to_owned())]);
        let offset = nth_offset(&source, "build().pu", 1) + "build().pu".len();

        let completion = workspace_source_method_completions_with_open_docs(
            &source,
            &package,
            &open_docs,
            offset_to_position(&source, offset),
        )
        .expect("broken-source method completion should use open dependency source");

        let CompletionResponse::Array(items) = completion else {
            panic!("broken-source method completion should resolve to a plain item array")
        };
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].label, "pulse");
        assert_eq!(items[0].kind, Some(CompletionItemKind::METHOD));
        assert_eq!(items[0].detail.as_deref(), Some("fn pulse(self) -> Int"));
    }

    #[test]
    fn same_named_local_dependency_member_document_highlights_prefer_matching_dependency_source() {
        let temp = TempDir::new("ql-lsp-same-named-local-dependency-member-document-highlights");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.shared.alpha.build as build
use demo.shared.beta.build as other

pub fn main() -> Int {
    return build().ping() + build().value + build().ping() + build().value + other().ping() + other().value
}
"#,
        );
        let alpha_source_path = temp.write(
            "workspace/vendor/alpha/src/lib.ql",
            r#"
package demo.shared.alpha

pub struct Config {
    value: Int,
}

pub fn build() -> Config {
    return Config { value: 1 }
}

impl Config {
    pub fn ping(self) -> Int {
        return self.value
    }
}
"#,
        );
        temp.write(
            "workspace/vendor/beta/src/lib.ql",
            r#"
package demo.shared.beta

pub struct Config {
    value: Bool,
}

pub fn build() -> Config {
    return Config { value: true }
}

impl Config {
    pub fn ping(self) -> Bool {
        return self.value
    }
}
"#,
        );
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

pub struct Config {
    value: Int,
}

pub fn build() -> Config

impl Config {
    pub fn ping(self) -> Int
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

pub struct Config {
    value: Bool,
}

pub fn build() -> Config

impl Config {
    pub fn ping(self) -> Bool
}
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        let analysis = analyze_source(&source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let alpha_source =
            fs::read_to_string(&alpha_source_path).expect("alpha source should read");

        let method_highlights = workspace_dependency_document_highlights(
            &uri,
            &source,
            &analysis,
            &package,
            offset_to_position(&source, nth_offset(&source, "ping", 1)),
        )
        .expect("same-named dependency method document highlight should exist");
        let method_actual = method_highlights
            .into_iter()
            .map(|highlight| highlight.range.start)
            .collect::<Vec<_>>();
        let method_expected = vec![
            offset_to_position(&source, nth_offset(&source, "ping", 1)),
            offset_to_position(&source, nth_offset(&source, "ping", 2)),
        ];
        assert_eq!(method_actual, method_expected);

        let field_highlights = workspace_dependency_document_highlights(
            &uri,
            &source,
            &analysis,
            &package,
            offset_to_position(&source, nth_offset(&source, "value", 1)),
        )
        .expect("same-named dependency field document highlight should exist");
        let field_actual = field_highlights
            .into_iter()
            .map(|highlight| highlight.range.start)
            .collect::<Vec<_>>();
        let field_expected = vec![
            offset_to_position(&source, nth_offset(&source, "value", 1)),
            offset_to_position(&source, nth_offset(&source, "value", 2)),
        ];
        assert_eq!(field_actual, field_expected);

        assert!(
            !method_expected.contains(&offset_to_position(&source, nth_offset(&source, "ping", 3))),
            "alpha highlights should not include beta member occurrence",
        );
        assert!(
            !field_expected.contains(&offset_to_position(
                &source,
                nth_offset(&source, "value", 3)
            )),
            "alpha highlights should not include beta field occurrence",
        );
        assert!(
            alpha_source.contains("pub fn ping(self) -> Int"),
            "fixture should keep alpha source distinct for disambiguation",
        );
    }

    #[test]
    fn same_named_local_dependency_broken_source_member_document_highlights_prefer_matching_dependency_source()
     {
        let temp = TempDir::new(
            "ql-lsp-same-named-local-dependency-broken-source-member-document-highlights",
        );
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.shared.alpha.build as build
use demo.shared.beta.build as other

pub fn main() -> Int {
    return build().ping() + build().value + build().ping() + build().value + other().ping() + other().value
"#,
        );
        let alpha_source_path = temp.write(
            "workspace/vendor/alpha/src/lib.ql",
            r#"
package demo.shared.alpha

pub struct Config {
    value: Int,
}

pub fn build() -> Config {
    return Config { value: 1 }
}

impl Config {
    pub fn ping(self) -> Int {
        return self.value
    }
}
"#,
        );
        temp.write(
            "workspace/vendor/beta/src/lib.ql",
            r#"
package demo.shared.beta

pub struct Config {
    value: Bool,
}

pub fn build() -> Config {
    return Config { value: true }
}

impl Config {
    pub fn ping(self) -> Bool {
        return self.value
    }
}
"#,
        );
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

pub struct Config {
    value: Int,
}

pub fn build() -> Config

impl Config {
    pub fn ping(self) -> Int
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

pub struct Config {
    value: Bool,
}

pub fn build() -> Config

impl Config {
    pub fn ping(self) -> Bool
}
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&source).is_err());
        let package = package_analysis_for_path(&app_path)
            .expect("package analysis should survive parse errors");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let alpha_source =
            fs::read_to_string(&alpha_source_path).expect("alpha source should read");

        let method_highlights = fallback_document_highlights_for_package_at(
            &uri,
            &source,
            &package,
            offset_to_position(&source, nth_offset(&source, "ping", 1)),
        )
        .expect("broken-source same-named dependency method document highlight should exist");
        let method_actual = method_highlights
            .into_iter()
            .map(|highlight| highlight.range.start)
            .collect::<Vec<_>>();
        let method_expected = vec![
            offset_to_position(&source, nth_offset(&source, "ping", 1)),
            offset_to_position(&source, nth_offset(&source, "ping", 2)),
        ];
        assert_eq!(method_actual, method_expected);

        let field_highlights = fallback_document_highlights_for_package_at(
            &uri,
            &source,
            &package,
            offset_to_position(&source, nth_offset(&source, "value", 1)),
        )
        .expect("broken-source same-named dependency field document highlight should exist");
        let field_actual = field_highlights
            .into_iter()
            .map(|highlight| highlight.range.start)
            .collect::<Vec<_>>();
        let field_expected = vec![
            offset_to_position(&source, nth_offset(&source, "value", 1)),
            offset_to_position(&source, nth_offset(&source, "value", 2)),
        ];
        assert_eq!(field_actual, field_expected);

        assert!(
            !method_expected.contains(&offset_to_position(&source, nth_offset(&source, "ping", 3))),
            "alpha highlights should not include beta member occurrence",
        );
        assert!(
            !field_expected.contains(&offset_to_position(
                &source,
                nth_offset(&source, "value", 3)
            )),
            "alpha highlights should not include beta field occurrence",
        );
        assert!(
            alpha_source.contains("pub fn ping(self) -> Int"),
            "fixture should keep alpha source distinct for disambiguation",
        );
    }

    #[test]
    fn workspace_dependency_references_without_declaration_include_other_workspace_uses() {
        let temp = TempDir::new("ql-lsp-workspace-dependency-source-references-no-decl");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.Config as Cfg

pub fn main(config: Cfg) -> Int {
    return config.ping()
}
"#,
        );
        let task_path = temp.write(
            "workspace/packages/app/src/task.ql",
            r#"
package demo.app

use demo.core.Config as OtherCfg

pub fn task(config: OtherCfg) -> Int {
    return config.ping()
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub struct Config {
    value: Int,
}

impl Config {
    pub fn ping(self) -> Int {
        return self.value
    }

    pub fn use_ping(self) -> Int {
        return self.ping()
    }
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub struct Config {
    value: Int,
}

impl Config {
    pub fn ping(self) -> Int
}
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        let analysis = analyze_source(&source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let core_source =
            fs::read_to_string(&core_source_path).expect("core source should read for assertions");
        let task_source = fs::read_to_string(&task_path).expect("task source should read");
        let task_uri = Url::from_file_path(&task_path).expect("task path should convert to URI");

        let references = workspace_source_references_for_dependency(
            &uri,
            &source,
            Some(&analysis),
            &package,
            offset_to_position(&source, nth_offset(&source, "ping", 1)),
            false,
        )
        .expect("workspace dependency references without declaration should exist");

        assert_eq!(references.len(), 3);
        assert_eq!(references[0].uri, uri);
        assert_eq!(
            references[0].range.start,
            offset_to_position(&source, nth_offset(&source, "ping", 1)),
        );
        assert!(
            references.iter().any(|reference| {
                reference
                    .uri
                    .to_file_path()
                    .ok()
                    .and_then(|path| path.canonicalize().ok())
                    == core_source_path.canonicalize().ok()
                    && reference.range.start
                        == offset_to_position(&core_source, nth_offset(&core_source, "ping", 3))
            }),
            "references should include workspace source method use",
        );
        assert!(
            references.iter().any(|reference| {
                reference.uri == task_uri
                    && reference.range.start
                        == offset_to_position(&task_source, nth_offset(&task_source, "ping", 1))
            }),
            "references should include other workspace file method use",
        );
    }

    #[test]
    fn workspace_dependency_value_references_survive_parse_errors_and_prefer_workspace_member_source()
     {
        let temp = TempDir::new("ql-lsp-workspace-dependency-source-references-parse-errors");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.exported as run

pub fn main(value: Int) -> Int {
    let result = run(value)
    return result
"#,
        );
        let task_path = temp.write(
            "workspace/packages/app/src/task.ql",
            r#"
package demo.app

use demo.core.exported as call

pub fn task(value: Int) -> Int {
    return call(value)
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn exported(value: Int) -> Int {
    return value
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn exported(value: Int) -> Int
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&source).is_err());
        let package = package_analysis_for_path(&app_path)
            .expect("package analysis should survive parse errors");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let core_source =
            fs::read_to_string(&core_source_path).expect("core source should read for assertions");
        let task_source = fs::read_to_string(&task_path).expect("task source should read");
        let task_uri = Url::from_file_path(&task_path).expect("task path should convert to URI");
        let references = workspace_source_references_for_dependency_in_broken_source(
            &uri,
            &source,
            &package,
            offset_to_position(&source, nth_offset(&source, "run", 2)),
            true,
        )
        .expect("broken-source workspace dependency value references should exist");

        assert_eq!(references.len(), 5);
        assert_eq!(
            references[0]
                .uri
                .to_file_path()
                .expect("definition URI should convert to a file path")
                .canonicalize()
                .expect("definition path should canonicalize"),
            core_source_path
                .canonicalize()
                .expect("core source path should canonicalize"),
        );
        assert_eq!(
            references[0].range.start,
            offset_to_position(&core_source, nth_offset(&core_source, "exported", 1)),
        );
        assert_eq!(references[1].uri, uri);
        assert_eq!(
            references[1].range.start,
            offset_to_position(&source, nth_offset(&source, "run", 1)),
        );
        assert_eq!(references[2].uri, uri);
        assert_eq!(
            references[2].range.start,
            offset_to_position(&source, nth_offset(&source, "run", 2)),
        );
        assert!(
            references.iter().any(|reference| {
                reference.uri == task_uri
                    && reference.range.start
                        == offset_to_position(&task_source, nth_offset(&task_source, "call", 1))
            }),
            "run should include task alias definition",
        );
        assert!(
            references.iter().any(|reference| {
                reference.uri == task_uri
                    && reference.range.start
                        == offset_to_position(&task_source, nth_offset(&task_source, "call", 2))
            }),
            "run should include task call occurrence",
        );
    }

    #[test]
    fn workspace_dependency_value_references_without_declaration_survive_parse_errors() {
        let temp =
            TempDir::new("ql-lsp-workspace-dependency-source-references-parse-errors-no-decl");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.exported as run

pub fn main(value: Int) -> Int {
    return run(value
"#,
        );
        let task_path = temp.write(
            "workspace/packages/app/src/task.ql",
            r#"
package demo.app

use demo.core.exported as call

pub fn task(value: Int) -> Int {
    return call(value)
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn exported(value: Int) -> Int {
    return value
}

pub fn wrapper(value: Int) -> Int {
    return exported(value)
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn exported(value: Int) -> Int
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&source).is_err());
        let package = package_analysis_for_path(&app_path)
            .expect("package analysis should survive parse errors");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let core_source =
            fs::read_to_string(&core_source_path).expect("core source should read for assertions");
        let task_source = fs::read_to_string(&task_path).expect("task source should read");
        let task_uri = Url::from_file_path(&task_path).expect("task path should convert to URI");

        let references = workspace_source_references_for_dependency_in_broken_source(
            &uri,
            &source,
            &package,
            offset_to_position(&source, nth_offset(&source, "run", 2)),
            false,
        )
        .expect(
            "broken-source workspace dependency value references without declaration should exist",
        );

        assert_eq!(references.len(), 3);
        assert_eq!(references[0].uri, uri);
        assert_eq!(
            references[0].range.start,
            offset_to_position(&source, nth_offset(&source, "run", 2)),
        );
        assert!(
            references.iter().any(|reference| {
                reference
                    .uri
                    .to_file_path()
                    .ok()
                    .and_then(|path| path.canonicalize().ok())
                    == core_source_path.canonicalize().ok()
                    && reference.range.start
                        == offset_to_position(&core_source, nth_offset(&core_source, "exported", 2))
            }),
            "references should include workspace source occurrence",
        );
        assert!(
            references.iter().any(|reference| {
                reference.uri == task_uri
                    && reference.range.start
                        == offset_to_position(&task_source, nth_offset(&task_source, "call", 2))
            }),
            "references should include other workspace file occurrence",
        );
    }

    #[test]
    fn document_highlight_keeps_same_file_definition_and_usages() {
        let temp = TempDir::new("ql-lsp-document-highlight-same-file");
        let source_path = temp.write(
            "pkg/src/main.ql",
            r#"
pub fn helper() -> Int {
    return 1
}

pub fn main() -> Int {
    let first = helper()
    return helper() + first
}
"#,
        );
        let source = fs::read_to_string(&source_path).expect("source should read");
        let analysis = analyze_source(&source).expect("source should analyze");
        let uri = Url::from_file_path(&source_path).expect("source path should convert to URI");

        let highlights = document_highlights_for_analysis_at(
            &uri,
            &source,
            &analysis,
            offset_to_position(&source, nth_offset(&source, "helper", 2)),
        )
        .expect("same-file document highlight should exist");

        let actual = highlights
            .into_iter()
            .map(|highlight| highlight.range.start)
            .collect::<Vec<_>>();
        let expected = vec![
            offset_to_position(&source, nth_offset(&source, "helper", 1)),
            offset_to_position(&source, nth_offset(&source, "helper", 2)),
            offset_to_position(&source, nth_offset(&source, "helper", 3)),
        ];

        assert_eq!(actual, expected);
    }

    #[test]
    fn document_highlight_keeps_package_import_occurrences_in_current_file() {
        let temp = TempDir::new("ql-lsp-document-highlight-package-import");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.exported as run

pub fn main() -> Int {
    return run(1)
}
"#,
        );
        temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn exported(value: Int) -> Int {
    return value
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn exported(value: Int) -> Int
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        let analysis = analyze_source(&source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");

        let highlights = workspace_import_document_highlights(
            &uri,
            &source,
            &analysis,
            &package,
            offset_to_position(&source, nth_offset(&source, "run", 2)),
        )
        .expect("package-aware document highlight should exist");

        let actual = highlights
            .into_iter()
            .map(|highlight| highlight.range.start)
            .collect::<Vec<_>>();
        let expected = vec![
            offset_to_position(&source, nth_offset(&source, "run", 1)),
            offset_to_position(&source, nth_offset(&source, "run", 2)),
        ];

        assert_eq!(actual, expected);
    }

    #[test]
    fn workspace_import_document_highlights_prefer_open_workspace_source() {
        let temp = TempDir::new("ql-lsp-document-highlight-package-import-open-docs");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.measure as run

pub fn main() -> Int {
    let first = run(1)
    let second = run(first)
    return second
}
"#,
        );
        let core_source_path = temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn helper() -> Int {
    return 0
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn measure(value: Int) -> Int
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        let analysis = analyze_source(&source).expect("app source should analyze");
        let package =
            package_analysis_for_path(&app_path).expect("package analysis should succeed");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let core_uri =
            Url::from_file_path(&core_source_path).expect("core path should convert to URI");
        let open_core_source = r#"
package demo.core

pub fn helper() -> Int {
    return 0
}

pub fn measure(value: Int) -> Int {
    return value
}
"#
        .to_owned();

        assert_eq!(
            workspace_import_document_highlights(
                &uri,
                &source,
                &analysis,
                &package,
                offset_to_position(&source, nth_offset(&source, "run", 2)),
            ),
            None,
            "disk-only document highlight should miss unsaved workspace source",
        );

        let highlights = workspace_import_document_highlights_with_open_docs(
            &uri,
            &source,
            &analysis,
            &package,
            &file_open_documents(vec![
                (uri.clone(), source.clone()),
                (core_uri, open_core_source),
            ]),
            offset_to_position(&source, nth_offset(&source, "run", 2)),
        )
        .expect("package-aware document highlight should use open workspace source");

        let actual = highlights
            .into_iter()
            .map(|highlight| highlight.range.start)
            .collect::<Vec<_>>();
        let expected = vec![
            offset_to_position(&source, nth_offset(&source, "run", 1)),
            offset_to_position(&source, nth_offset(&source, "run", 2)),
            offset_to_position(&source, nth_offset(&source, "run", 3)),
        ];

        assert_eq!(actual, expected);
    }

    #[test]
    fn document_highlight_keeps_workspace_import_occurrences_in_broken_source() {
        let temp = TempDir::new("ql-lsp-document-highlight-package-import-parse-errors");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.exported as run

pub fn main() -> Int {
    let first = run(1)
    let second = run(first)
    return second
"#,
        );
        temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn exported(value: Int) -> Int {
    return value
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn exported(value: Int) -> Int
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&source).is_err());
        let package = package_analysis_for_path(&app_path)
            .expect("package analysis should survive parse errors");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");

        let highlights = fallback_document_highlights_for_package_at(
            &uri,
            &source,
            &package,
            offset_to_position(&source, nth_offset(&source, "run", 2)),
        )
        .expect("broken-source workspace import document highlight should exist");

        let actual = highlights
            .into_iter()
            .map(|highlight| highlight.range.start)
            .collect::<Vec<_>>();
        let expected = vec![
            offset_to_position(&source, nth_offset(&source, "run", 1)),
            offset_to_position(&source, nth_offset(&source, "run", 2)),
            offset_to_position(&source, nth_offset(&source, "run", 3)),
        ];

        assert_eq!(actual, expected);
    }

    #[test]
    fn document_highlight_keeps_dependency_value_occurrences_in_broken_source() {
        let temp = TempDir::new("ql-lsp-document-highlight-dependency-value-parse-errors");
        let app_path = temp.write(
            "workspace/packages/app/src/main.ql",
            r#"
package demo.app

use demo.core.exported as run

pub fn main() -> Int {
    let first = run(1)
    return run(first)
"#,
        );
        temp.write(
            "workspace/packages/app/src/task.ql",
            r#"
package demo.app

use demo.core.exported as call

pub fn task(value: Int) -> Int {
    return call(value)
}
"#,
        );
        temp.write(
            "workspace/packages/core/src/lib.ql",
            r#"
package demo.core

pub fn exported(value: Int) -> Int {
    return value
}
"#,
        );
        temp.write(
            "workspace/qlang.toml",
            r#"
[workspace]
members = ["packages/app", "packages/core"]
"#,
        );
        temp.write(
            "workspace/packages/app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "workspace/packages/core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "workspace/packages/core/core.qi",
            r#"
// qlang interface v1
// package: core

// source: src/lib.ql
package demo.core

pub fn exported(value: Int) -> Int
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&source).is_err());
        let package = package_analysis_for_path(&app_path)
            .expect("package analysis should survive parse errors");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");

        let highlights = fallback_document_highlights_for_package_at(
            &uri,
            &source,
            &package,
            offset_to_position(&source, nth_offset(&source, "run", 3)),
        )
        .expect("broken-source dependency value document highlight should exist");

        let actual = highlights
            .into_iter()
            .map(|highlight| highlight.range.start)
            .collect::<Vec<_>>();
        let expected = vec![
            offset_to_position(&source, nth_offset(&source, "run", 1)),
            offset_to_position(&source, nth_offset(&source, "run", 2)),
            offset_to_position(&source, nth_offset(&source, "run", 3)),
        ];

        assert_eq!(actual, expected);
    }

    #[test]
    fn document_highlight_keeps_dependency_structured_root_indexed_value_occurrences_in_broken_source()
     {
        let temp = TempDir::new(
            "ql-lsp-document-highlight-dependency-structured-root-indexed-value-parse-errors",
        );
        let app_path = temp.write(
            "workspace/app/src/lib.ql",
            r#"
package demo.app

use demo.dep.maybe_children

pub fn read(flag: Bool) -> Int {
    let first = (if flag { maybe_children()? } else { maybe_children()? })[0]
    let second = (match flag { true => maybe_children()?, false => maybe_children()? })[1]
    return first.value + second.value + first.value
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

pub fn maybe_children() -> Option[[Child; 2]]
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&source).is_err());
        let package = package_analysis_for_path(&app_path)
            .expect("package analysis should survive parse errors");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");

        let highlights = fallback_document_highlights_for_package_at(
            &uri,
            &source,
            &package,
            offset_to_position(&source, nth_offset(&source, "first", 3)),
        )
        .expect(
            "broken-source dependency structured root-indexed value document highlight should exist",
        );

        let actual = highlights
            .into_iter()
            .map(|highlight| highlight.range.start)
            .collect::<Vec<_>>();
        let expected = vec![
            offset_to_position(&source, nth_offset(&source, "first", 1)),
            offset_to_position(&source, nth_offset(&source, "first", 2)),
            offset_to_position(&source, nth_offset(&source, "first", 3)),
        ];

        assert_eq!(actual, expected);
    }

    #[test]
    fn dependency_value_prepare_rename_and_rename_survive_structured_root_indexed_parse_errors() {
        let temp =
            TempDir::new("ql-lsp-dependency-value-rename-structured-root-indexed-parse-errors");
        let app_path = temp.write(
            "workspace/app/src/lib.ql",
            r#"
package demo.app

use demo.dep.maybe_children

pub fn read(flag: Bool) -> Int {
    let first = (if flag { maybe_children()? } else { maybe_children()? })[0]
    let second = (match flag { true => maybe_children()?, false => maybe_children()? })[1]
    return first.value + second.value + first.value
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

pub fn maybe_children() -> Option[[Child; 2]]
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&source).is_err());
        let package = package_analysis_for_path(&app_path)
            .expect("package analysis should survive parse errors");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");
        let use_offset = nth_offset(&source, "first", 2);

        assert_eq!(
            prepare_rename_for_dependency_imports(
                &source,
                &package,
                offset_to_position(&source, use_offset),
            ),
            Some(PrepareRenameResponse::RangeWithPlaceholder {
                range: span_to_range(&source, Span::new(use_offset, use_offset + "first".len())),
                placeholder: "first".to_owned(),
            }),
        );

        let edit = rename_for_dependency_imports(
            &uri,
            &source,
            &package,
            offset_to_position(&source, use_offset),
            "current_child",
        )
        .expect("rename should succeed")
        .expect("rename should return workspace edits");
        let changes = edit
            .changes
            .expect("rename should use simple workspace changes");
        let edits = changes
            .get(&uri)
            .expect("rename should edit current document");
        assert_eq!(
            edits,
            &vec![
                TextEdit::new(
                    span_to_range(
                        &source,
                        Span::new(
                            nth_offset(&source, "first", 1),
                            nth_offset(&source, "first", 1) + "first".len(),
                        ),
                    ),
                    "current_child".to_owned(),
                ),
                TextEdit::new(
                    span_to_range(
                        &source,
                        Span::new(
                            nth_offset(&source, "first", 2),
                            nth_offset(&source, "first", 2) + "first".len(),
                        ),
                    ),
                    "current_child".to_owned(),
                ),
                TextEdit::new(
                    span_to_range(
                        &source,
                        Span::new(
                            nth_offset(&source, "first", 3),
                            nth_offset(&source, "first", 3) + "first".len(),
                        ),
                    ),
                    "current_child".to_owned(),
                ),
            ],
        );
    }

    #[test]
    fn workspace_import_references_skip_non_workspace_dependency_in_broken_source() {
        let temp = TempDir::new("ql-lsp-workspace-import-references-skip-dependency");
        let app_path = temp.write(
            "app/src/main.ql",
            r#"
package demo.app

use demo.core.exported as run

pub fn main() -> Int {
    return run(
"#,
        );
        temp.write(
            "app/qlang.toml",
            r#"
[package]
name = "app"

[references]
packages = ["../core"]
"#,
        );
        temp.write(
            "core/qlang.toml",
            r#"
[package]
name = "core"
"#,
        );
        temp.write(
            "core/core.qi",
            r#"
// qlang interface v1
// package: core
package demo.core

pub fn exported(value: Int) -> Int
"#,
        );

        let source = fs::read_to_string(&app_path).expect("app source should read");
        assert!(analyze_source(&source).is_err());
        let package = package_analysis_for_path(&app_path)
            .expect("package analysis should survive parse errors");
        let uri = Url::from_file_path(&app_path).expect("app path should convert to URI");

        let references = workspace_source_references_for_import_in_broken_source(
            &uri,
            &source,
            &package,
            offset_to_position(&source, nth_offset(&source, "run", 2)),
            true,
        )
        .is_none();

        assert!(references);
    }
