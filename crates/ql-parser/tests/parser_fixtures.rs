use std::fs;
use std::path::PathBuf;

use ql_ast::{ExprKind, ItemKind, Param, StmtKind, TypeExprKind, Visibility};
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
            .any(|item| matches!(item.kind, ItemKind::Const(_)))
    );
    assert!(
        module
            .items
            .iter()
            .any(|item| matches!(item.kind, ItemKind::Static(_)))
    );
    assert!(
        module
            .items
            .iter()
            .any(|item| matches!(&item.kind, ItemKind::TypeAlias(alias) if alias.is_opaque))
    );
    assert!(
        module
            .items
            .iter()
            .any(|item| matches!(item.kind, ItemKind::Trait(_)))
    );
    assert!(
        module
            .items
            .iter()
            .any(|item| matches!(item.kind, ItemKind::Impl(_)))
    );
    assert!(
        module
            .items
            .iter()
            .any(|item| matches!(item.kind, ItemKind::Extend(_)))
    );
    assert!(
        module
            .items
            .iter()
            .any(|item| matches!(item.kind, ItemKind::ExternBlock(_)))
    );
    assert!(
        module.items.iter().any(
            |item| matches!(&item.kind, ItemKind::Function(function) if function.abi.is_some())
        )
    );

    let extern_block = module
        .items
        .iter()
        .find_map(|item| match &item.kind {
            ItemKind::ExternBlock(block) => Some(block),
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
                    ty,
                    ..
                }) if matches!(&ty.kind, TypeExprKind::Pointer { is_const: true, inner }
                    if matches!(
                        &inner.kind,
                        TypeExprKind::Named { path, args } if path.segments.as_slice() == ["U8"] && args.is_empty()
                    ))
            )
    ));

    assert!(
        module.items.iter().any(|item| matches!(
            &item.kind,
            ItemKind::Function(function)
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

#[test]
fn parses_control_flow_heads_without_struct_literal_bias() {
    let source = r#"
fn probe() {
    if ready {}
    while ready {}
    for item in ready {}
    match ready {
        _ => {}
    }
}
"#;
    let module = parse_source(source).expect("control-flow heads should parse");
    let function = match &module.items[0].kind {
        ItemKind::Function(function) => function,
        other => panic!("expected function item, got {other:?}"),
    };
    let body = function.body.as_ref().expect("function should have a body");

    assert!(matches!(
        &body.statements[0].kind,
        StmtKind::Expr { expr, .. }
            if matches!(
                &expr.kind,
                ExprKind::If { condition, .. }
                    if matches!(&condition.kind, ExprKind::Name(name) if name == "ready")
            )
    ));
    assert!(matches!(
        &body.statements[1].kind,
        StmtKind::While { condition, .. }
            if matches!(&condition.kind, ExprKind::Name(name) if name == "ready")
    ));
    assert!(matches!(
        &body.statements[2].kind,
        StmtKind::For { iterable, .. }
            if matches!(&iterable.kind, ExprKind::Name(name) if name == "ready")
    ));
    assert!(matches!(
        body.tail.as_deref(),
        Some(expr)
            if matches!(
                &expr.kind,
                ExprKind::Match { value, .. }
                    if matches!(&value.kind, ExprKind::Name(name) if name == "ready")
            )
    ));
}

#[test]
fn preserves_single_element_tuple_types() {
    let source = "fn takes_one(value: (Int,)) -> (Int,) { return value }";
    let module = parse_source(source).expect("single-element tuple types should parse");
    let function = match &module.items[0].kind {
        ItemKind::Function(function) => function,
        other => panic!("expected function item, got {other:?}"),
    };

    assert!(matches!(
        function.params.first(),
        Some(Param::Regular { ty, .. })
            if matches!(&ty.kind, TypeExprKind::Tuple(items) if items.len() == 1)
    ));
    assert!(matches!(
        function.return_type.as_ref(),
        Some(ty)
            if matches!(&ty.kind, TypeExprKind::Tuple(items) if items.len() == 1)
    ));
}

#[test]
fn attaches_spans_to_nested_nodes() {
    let source = "fn main() { let value = 1 }";
    let module = parse_source(source).expect("span fixture should parse");
    let item = &module.items[0];

    assert_eq!(
        &source[item.span.start..item.span.end],
        "fn main() { let value = 1 }"
    );

    let function = match &item.kind {
        ItemKind::Function(function) => function,
        other => panic!("expected function item, got {other:?}"),
    };
    let body = function.body.as_ref().expect("function should have a body");
    assert_eq!(&source[body.span.start..body.span.end], "{ let value = 1 }");

    let stmt = &body.statements[0];
    assert_eq!(&source[stmt.span.start..stmt.span.end], "let value = 1");
    assert!(matches!(
        &stmt.kind,
        StmtKind::Let { value, .. } if &source[value.span.start..value.span.end] == "1"
    ));
}
