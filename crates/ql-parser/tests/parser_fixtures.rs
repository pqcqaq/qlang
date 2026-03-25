use std::fs;
use std::path::PathBuf;

use ql_parser::parse_source;

fn fixture(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/parser")
        .join(path)
}

#[test]
fn parses_pass_fixture() {
    let source = fs::read_to_string(fixture("pass/basic.ql")).expect("read pass fixture");
    let module = parse_source(&source).expect("pass fixture should parse");

    assert_eq!(module.package.unwrap().path.segments, vec!["demo", "main"]);
    assert_eq!(module.uses.len(), 2);
    assert_eq!(module.items.len(), 5);
}

#[test]
fn parses_control_flow_fixture() {
    let source = fs::read_to_string(fixture("pass/control_flow.ql")).expect("read pass fixture");
    let module = parse_source(&source).expect("control flow fixture should parse");

    assert_eq!(
        module.package.unwrap().path.segments,
        vec!["demo", "control"]
    );
    assert_eq!(module.items.len(), 2);
}

#[test]
fn reports_fail_fixture() {
    let source = fs::read_to_string(fixture("fail/missing_name.ql")).expect("read fail fixture");
    let errors = parse_source(&source).expect_err("fail fixture should not parse");

    assert!(!errors.is_empty());
    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("expected function name"))
    );
}

#[test]
fn reports_bad_match_fixture() {
    let source = fs::read_to_string(fixture("fail/bad_match.ql")).expect("read fail fixture");
    let errors = parse_source(&source).expect_err("bad match fixture should not parse");

    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("expected `=>` in match arm"))
    );
}
