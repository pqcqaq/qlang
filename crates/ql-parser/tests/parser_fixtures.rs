use std::fs;
use std::path::PathBuf;

use ql_ast::{Item, Param, TypeExpr, Visibility};
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
fn parses_phase1_declarations_fixture() {
    let source =
        fs::read_to_string(fixture("pass/phase1_declarations.ql")).expect("read pass fixture");
    let module = parse_source(&source).expect("phase1 declarations fixture should parse");

    assert_eq!(
        module.package.unwrap().path.segments,
        vec!["demo", "phase1"]
    );
    assert!(
        module
            .items
            .iter()
            .any(|item| matches!(item, Item::Const(_)))
    );
    assert!(
        module
            .items
            .iter()
            .any(|item| matches!(item, Item::Static(_)))
    );
    assert!(
        module
            .items
            .iter()
            .any(|item| matches!(item, Item::TypeAlias(alias) if alias.is_opaque))
    );
    assert!(
        module
            .items
            .iter()
            .any(|item| matches!(item, Item::Trait(_)))
    );
    assert!(
        module
            .items
            .iter()
            .any(|item| matches!(item, Item::Impl(_)))
    );
    assert!(
        module
            .items
            .iter()
            .any(|item| matches!(item, Item::Extend(_)))
    );
    assert!(
        module
            .items
            .iter()
            .any(|item| matches!(item, Item::ExternBlock(_)))
    );
    assert!(
        module
            .items
            .iter()
            .any(|item| matches!(item, Item::Function(function) if function.abi.is_some()))
    );

    let extern_block = module
        .items
        .iter()
        .find_map(|item| match item {
            Item::ExternBlock(block) => Some(block),
            _ => None,
        })
        .expect("phase1 fixture should contain extern block");
    assert_eq!(extern_block.visibility, Visibility::Public);
    assert!(matches!(
        extern_block.functions.first(),
        Some(function)
            if matches!(
                function.params.first(),
                Some(Param::Regular {
                    ty: TypeExpr::Pointer { is_const: true, inner },
                    ..
                }) if matches!(
                    inner.as_ref(),
                    TypeExpr::Named { path, args } if path.segments.as_slice() == ["U8"] && args.is_empty()
                )
            )
    ));

    assert!(
        module.items.iter().any(|item| matches!(
            item,
            Item::Function(function)
                if function.name == "keyword_passthrough"
                    && matches!(function.params.first(), Some(Param::Regular { name, .. }) if name == "type")
        ))
    );
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

#[test]
fn reports_bad_extern_fixture() {
    let source = fs::read_to_string(fixture("fail/bad_extern.ql")).expect("read fail fixture");
    let errors = parse_source(&source).expect_err("bad extern fixture should not parse");

    assert!(
        errors
            .iter()
            .any(|error| error.message.contains("expected `fn` in extern block"))
    );
}
