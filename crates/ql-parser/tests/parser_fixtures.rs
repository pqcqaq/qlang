use std::fs;
use std::path::PathBuf;

use ql_ast::{ExprKind, ItemKind, Param, PatternKind, StmtKind, TypeExprKind, Visibility};
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
fn parses_top_level_extern_function_definitions() {
    let source = r#"
extern "c" pub fn q_add(left: Int, right: Int) -> Int {
    return left + right
}
"#;
    let module = parse_source(source).expect("extern definition fixture should parse");
    let function = match &module.items[0].kind {
        ItemKind::Function(function) => function,
        other => panic!("expected function item, got {other:?}"),
    };

    assert_eq!(function.abi.as_deref(), Some("c"));
    assert!(matches!(function.visibility, Visibility::Public));
    assert!(
        function.body.is_some(),
        "extern function definition should keep its body"
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

#[test]
fn captures_precise_identifier_spans_for_semantic_nodes() {
    let source = r#"
fn sample[T](value: Int) {
    let Point { x, y: alias } = point
    Point { x, y: alias };
    run(left: 1);
    user.name;
    let closure = (item) => item
}
"#;
    let module = parse_source(source).expect("span fixture should parse");
    let function = match &module.items[0].kind {
        ItemKind::Function(function) => function,
        other => panic!("expected function item, got {other:?}"),
    };

    assert_eq!(
        &source[function.name_span.start..function.name_span.end],
        "sample"
    );
    assert_eq!(
        &source[function.generics[0].name_span.start..function.generics[0].name_span.end],
        "T"
    );
    let Param::Regular {
        name_span: param_span,
        ..
    } = &function.params[0]
    else {
        panic!("expected regular parameter");
    };
    assert_eq!(&source[param_span.start..param_span.end], "value");

    let body = function.body.as_ref().expect("function should have body");
    let StmtKind::Let { pattern, .. } = &body.statements[0].kind else {
        panic!("expected let statement");
    };
    let PatternKind::Struct { fields, .. } = &pattern.kind else {
        panic!("expected struct pattern");
    };
    assert_eq!(
        &source[fields[0].name_span.start..fields[0].name_span.end],
        "x"
    );
    assert_eq!(
        &source[fields[1].name_span.start..fields[1].name_span.end],
        "y"
    );

    let StmtKind::Expr { expr, .. } = &body.statements[1].kind else {
        panic!("expected expression statement");
    };
    let ExprKind::StructLiteral { fields, .. } = &expr.kind else {
        panic!("expected struct literal");
    };
    assert_eq!(
        &source[fields[0].name_span.start..fields[0].name_span.end],
        "x"
    );
    assert_eq!(
        &source[fields[1].name_span.start..fields[1].name_span.end],
        "y"
    );

    let StmtKind::Expr { expr, .. } = &body.statements[3].kind else {
        panic!("expected member expression statement");
    };
    let ExprKind::Member { field_span, .. } = &expr.kind else {
        panic!("expected member expression");
    };
    assert_eq!(&source[field_span.start..field_span.end], "name");

    let StmtKind::Expr { expr, .. } = &body.statements[2].kind else {
        panic!("expected call statement");
    };
    let ExprKind::Call { args, .. } = &expr.kind else {
        panic!("expected call expression");
    };
    let ql_ast::CallArg::Named { name_span, .. } = &args[0] else {
        panic!("expected named call argument");
    };
    assert_eq!(&source[name_span.start..name_span.end], "left");

    let StmtKind::Let { value, .. } = &body.statements[4].kind else {
        panic!("expected closure binding");
    };
    let ExprKind::Closure { params, .. } = &value.kind else {
        panic!("expected closure expression");
    };
    assert_eq!(&source[params[0].span.start..params[0].span.end], "item");
}

#[test]
fn captures_precise_receiver_parameter_spans() {
    let source = r#"
impl Counter {
    fn read(var self) -> Int {}
}
"#;
    let module = parse_source(source).expect("receiver fixture should parse");
    let impl_block = match &module.items[0].kind {
        ItemKind::Impl(impl_block) => impl_block,
        other => panic!("expected impl item, got {other:?}"),
    };
    let function = &impl_block.methods[0];
    let Param::Receiver {
        span: receiver_span,
        ..
    } = &function.params[0]
    else {
        panic!("expected receiver parameter");
    };

    assert_eq!(&source[receiver_span.start..receiver_span.end], "var self");
}
