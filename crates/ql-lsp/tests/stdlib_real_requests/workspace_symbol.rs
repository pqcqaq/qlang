use std::fs;
use std::path::Path;

use crate::common::request::{
    TempDir, did_open_via_request, offset_to_position, workspace_symbol_via_request,
};
use crate::common::stdlib_real::real_stdlib_source_path;
use crate::support::open_real_stdlib_workspace;
use tower_lsp::lsp_types::{SymbolInformation, SymbolKind, Url};

#[tokio::test(flavor = "current_thread")]
async fn workspace_symbol_request_uses_current_real_stdlib_workspace() {
    let temp = TempDir::new("ql-lsp-real-stdlib-workspace-symbol-request");
    let app_source = r#"
package demo.app

use std.core.max_int as largest_int

pub fn main() -> Int {
    return largest_int(1, 2)
}
"#;
    let (mut service, _, stdlib_root) = open_real_stdlib_workspace(&temp, app_source).await;

    assert_symbol(
        &workspace_symbol_via_request(&mut service, "max_int").await,
        "max_int",
        SymbolKind::FUNCTION,
        &real_stdlib_source_path(&stdlib_root, "core"),
    );
    assert_symbol(
        &workspace_symbol_via_request(&mut service, "Option").await,
        "Option",
        SymbolKind::ENUM,
        &real_stdlib_source_path(&stdlib_root, "option"),
    );

    let core_source_path = real_stdlib_source_path(&stdlib_root, "core");
    let core_disk_source = fs::read_to_string(&core_source_path)
        .expect("temp std.core source should exist")
        .replace("\r\n", "\n");
    let open_core_source =
        format!("{core_disk_source}\n\npub fn fresh_helper() -> Int {{\n    return 2\n}}\n");
    let core_uri =
        Url::from_file_path(&core_source_path).expect("temp std.core source path should convert");
    did_open_via_request(&mut service, core_uri.clone(), open_core_source.clone()).await;

    let fresh_symbols = workspace_symbol_via_request(&mut service, "fresh_helper").await;

    assert_eq!(fresh_symbols.len(), 1);
    assert_eq!(fresh_symbols[0].name, "fresh_helper");
    assert_eq!(fresh_symbols[0].kind, SymbolKind::FUNCTION);
    assert_eq!(fresh_symbols[0].location.uri, core_uri);
    assert_eq!(
        fresh_symbols[0].location.range.start,
        offset_to_position(
            &open_core_source,
            open_core_source
                .find("fresh_helper")
                .expect("fresh helper should exist")
        ),
        "workspace/symbol should prefer the open real stdlib source",
    );
}

fn assert_symbol(
    symbols: &[SymbolInformation],
    name: &str,
    kind: SymbolKind,
    expected_path: &Path,
) {
    let expected_path = expected_path
        .canonicalize()
        .expect("expected stdlib symbol path should canonicalize");
    assert!(
        symbols.iter().any(|symbol| {
            symbol.name == name
                && symbol.kind == kind
                && symbol
                    .location
                    .uri
                    .to_file_path()
                    .ok()
                    .and_then(|path| path.canonicalize().ok())
                    .is_some_and(|path| path == expected_path)
        }),
        "workspace/symbol should include {kind:?} `{name}` from {}: {symbols:#?}",
        expected_path.display(),
    );
}
