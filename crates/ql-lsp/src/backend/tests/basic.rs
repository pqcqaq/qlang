use super::*;

#[test]
fn package_source_snapshot_prefers_parseable_open_document() {
    let temp = TempDir::new("ql-lsp-package-source-snapshot-open");
    let source_path = temp.write(
        "workspace/src/lib.ql",
        r#"
package demo.core

pub struct Disk {}
"#,
    );
    temp.write(
        "workspace/qlang.toml",
        r#"
[package]
name = "core"
"#,
    );
    let package = package_analysis_for_path(&source_path).expect("package analysis should succeed");
    let uri = Url::from_file_path(&source_path).expect("source path should convert to URI");
    let open_source = r#"
package demo.core

pub struct Open {}
"#
    .to_owned();
    let open_docs = file_open_documents(vec![(uri.clone(), open_source.clone())]);

    let snapshot = package_source_snapshot_with_open_docs(&package, &open_docs, &source_path)
        .expect("open source snapshot should exist");
    let PackageSourceSnapshot::Analyzed {
        uri: snapshot_uri,
        source,
        analysis,
    } = snapshot
    else {
        panic!("parseable open document should produce an analyzed snapshot")
    };

    assert_eq!(snapshot_uri, uri);
    assert_eq!(source, open_source);
    let definition = analysis
        .definition_at(nth_offset(&source, "Open", 1))
        .expect("open document definition should be analyzed");
    assert_eq!(definition.kind, AnalysisSymbolKind::Struct);
    assert_eq!(definition.name, "Open");
}

#[test]
fn package_source_snapshot_keeps_broken_open_document() {
    let temp = TempDir::new("ql-lsp-package-source-snapshot-broken-open");
    let source_path = temp.write(
        "workspace/src/lib.ql",
        r#"
package demo.core

pub struct Disk {}
"#,
    );
    temp.write(
        "workspace/qlang.toml",
        r#"
[package]
name = "core"
"#,
    );
    let package = package_analysis_for_path(&source_path).expect("package analysis should succeed");
    let uri = Url::from_file_path(&source_path).expect("source path should convert to URI");
    let open_source = r#"
package demo.core

pub struct Open {
"#
    .to_owned();
    assert!(analyze_source(&open_source).is_err());
    let open_docs = file_open_documents(vec![(uri.clone(), open_source.clone())]);

    let snapshot = package_source_snapshot_with_open_docs(&package, &open_docs, &source_path)
        .expect("broken open source snapshot should exist");
    let PackageSourceSnapshot::BrokenOpen {
        uri: snapshot_uri,
        source,
    } = snapshot
    else {
        panic!("broken open document should stay available as a broken snapshot")
    };

    assert_eq!(snapshot_uri, uri);
    assert_eq!(source, open_source);
}

#[test]
fn source_order_location_normalization_sorts_and_deduplicates_overlaps() {
    let uri = Url::parse("file:///test.ql").expect("uri should parse");
    let first = Location::new(
        uri.clone(),
        Range::new(Position::new(1, 10), Position::new(1, 15)),
    );
    let overlapping = Location::new(
        uri.clone(),
        Range::new(Position::new(1, 12), Position::new(1, 20)),
    );
    let later = Location::new(uri, Range::new(Position::new(2, 0), Position::new(2, 5)));
    let mut locations = vec![later.clone(), overlapping, first.clone()];

    normalize_locations_in_source_order(&mut locations);

    assert_eq!(locations, vec![first, later]);
}

#[test]
fn implementation_response_normalizes_locations_before_returning_array() {
    let first_uri = Url::parse("file:///a.ql").expect("first URI should parse");
    let second_uri = Url::parse("file:///b.ql").expect("second URI should parse");
    let first = Location::new(
        first_uri.clone(),
        Range::new(Position::new(4, 0), Position::new(4, 5)),
    );
    let overlapping_first = Location::new(
        first_uri,
        Range::new(Position::new(4, 2), Position::new(4, 6)),
    );
    let second = Location::new(
        second_uri,
        Range::new(Position::new(1, 0), Position::new(1, 5)),
    );

    let response = implementation_response_from_locations(vec![
        second.clone(),
        overlapping_first,
        first.clone(),
    ])
    .expect("implementation response should exist");
    let GotoDefinitionResponse::Array(locations) = response else {
        panic!("deduplicated multi-location implementation should stay an array")
    };

    assert_eq!(locations, vec![first, second]);
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
