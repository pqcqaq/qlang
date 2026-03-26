use ql_hir::{CallArg, ExprKind, ItemKind, PatternKind, StmtKind, lower_module};
use ql_parser::parse_source;

fn first_function(source: &str) -> (ql_hir::Module, ql_hir::Function) {
    let ast = parse_source(source).expect("source should parse");
    let hir = lower_module(&ast);
    let function = match &hir.item(hir.items[0]).kind {
        ItemKind::Function(function) => function.clone(),
        other => panic!("expected function item, got {other:?}"),
    };
    (hir, function)
}

#[test]
fn lower_module_tracks_pattern_bindings_as_locals() {
    let (hir, function) = first_function(
        r#"
fn main() {
    let (left, right) = pair;
}
"#,
    );

    let body = hir.block(function.body.expect("function should have body"));
    let stmt = hir.stmt(body.statements[0]);
    let pattern_id = match &stmt.kind {
        StmtKind::Let { pattern, .. } => *pattern,
        other => panic!("expected let statement, got {other:?}"),
    };
    let pattern = hir.pattern(pattern_id);

    let PatternKind::Tuple(items) = &pattern.kind else {
        panic!("expected tuple pattern");
    };

    let left = hir.pattern(items[0]);
    let right = hir.pattern(items[1]);
    let left_local = match left.kind {
        PatternKind::Binding(local) => local,
        _ => panic!("expected left binding"),
    };
    let right_local = match right.kind {
        PatternKind::Binding(local) => local,
        _ => panic!("expected right binding"),
    };

    assert_eq!(hir.local(left_local).name, "left");
    assert_eq!(hir.local(right_local).name, "right");
    assert_eq!(hir.locals().len(), 2);
}

#[test]
fn lower_module_normalizes_struct_pattern_shorthand_bindings() {
    let (hir, function) = first_function(
        r#"
fn main() {
    let Point { x, y: alias } = point;
}
"#,
    );

    let body = hir.block(function.body.expect("function should have body"));
    let stmt = hir.stmt(body.statements[0]);
    let pattern_id = match &stmt.kind {
        StmtKind::Let { pattern, .. } => *pattern,
        other => panic!("expected let statement, got {other:?}"),
    };
    let pattern = hir.pattern(pattern_id);

    let PatternKind::Struct { fields, .. } = &pattern.kind else {
        panic!("expected struct pattern");
    };

    let x = hir.pattern(fields[0].pattern);
    let alias = hir.pattern(fields[1].pattern);
    let x_local = match x.kind {
        PatternKind::Binding(local) => local,
        _ => panic!("expected shorthand binding"),
    };
    let alias_local = match alias.kind {
        PatternKind::Binding(local) => local,
        _ => panic!("expected explicit binding"),
    };

    assert_eq!(fields[0].name, "x");
    assert!(fields[0].is_shorthand);
    assert_eq!(hir.local(x_local).name, "x");
    assert_eq!(fields[1].name, "y");
    assert!(!fields[1].is_shorthand);
    assert_eq!(hir.local(alias_local).name, "alias");
    assert_eq!(hir.locals().len(), 2);
}

#[test]
fn lower_module_normalizes_struct_literal_shorthand_fields() {
    let (hir, function) = first_function(
        r#"
fn main() {
    Point { x, y: 2 };
}
"#,
    );

    let body = hir.block(function.body.expect("function should have body"));
    let stmt = hir.stmt(body.statements[0]);
    let expr_id = match &stmt.kind {
        StmtKind::Expr { expr, .. } => *expr,
        other => panic!("expected expression statement, got {other:?}"),
    };
    let expr = hir.expr(expr_id);

    let ExprKind::StructLiteral { fields, .. } = &expr.kind else {
        panic!("expected struct literal");
    };

    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].name, "x");
    assert!(fields[0].is_shorthand);
    assert!(matches!(hir.expr(fields[0].value).kind, ExprKind::Name(ref name) if name == "x"));
    assert_eq!(fields[1].name, "y");
    assert!(!fields[1].is_shorthand);
    assert!(matches!(hir.expr(fields[1].value).kind, ExprKind::Integer(ref value) if value == "2"));
}

#[test]
fn lower_module_preserves_named_call_argument_label_spans() {
    let source = r#"
fn main() {
    run(left: 1);
}
"#;
    let ast = parse_source(source).expect("source should parse");
    let hir = lower_module(&ast);
    let function = match &hir.item(hir.items[0]).kind {
        ItemKind::Function(function) => function,
        other => panic!("expected function item, got {other:?}"),
    };
    let body = hir.block(function.body.expect("function should have body"));
    let stmt = hir.stmt(body.statements[0]);
    let expr_id = match &stmt.kind {
        StmtKind::Expr { expr, .. } => *expr,
        other => panic!("expected expression statement, got {other:?}"),
    };
    let expr = hir.expr(expr_id);

    let ExprKind::Call { args, .. } = &expr.kind else {
        panic!("expected call expression");
    };
    let CallArg::Named {
        name,
        name_span,
        value,
        ..
    } = &args[0]
    else {
        panic!("expected named call argument");
    };

    assert_eq!(name, "left");
    assert_eq!(&source[name_span.start..name_span.end], "left");
    assert!(matches!(hir.expr(*value).kind, ExprKind::Integer(ref value) if value == "1"));
}

#[test]
fn lower_module_preserves_closure_parameter_local_spans() {
    let source = r#"
fn main() {
    let closure = (item, next) => item + next;
}
"#;
    let ast = parse_source(source).expect("source should parse");
    let hir = lower_module(&ast);
    let function = match &hir.item(hir.items[0]).kind {
        ItemKind::Function(function) => function,
        other => panic!("expected function item, got {other:?}"),
    };
    let body = hir.block(function.body.expect("function should have body"));
    let stmt = hir.stmt(body.statements[0]);
    let value = match &stmt.kind {
        StmtKind::Let { value, .. } => *value,
        other => panic!("expected let statement, got {other:?}"),
    };
    let expr = hir.expr(value);

    let ExprKind::Closure { params, .. } = &expr.kind else {
        panic!("expected closure expression");
    };

    assert_eq!(
        &source[hir.local(params[0]).span.start..hir.local(params[0]).span.end],
        "item"
    );
    assert_eq!(
        &source[hir.local(params[1]).span.start..hir.local(params[1]).span.end],
        "next"
    );
}

#[test]
fn lower_module_preserves_receiver_parameter_spans() {
    let source = r#"
impl Counter {
    fn read(var self) -> Int {}
}
"#;
    let ast = parse_source(source).expect("source should parse");
    let hir = lower_module(&ast);
    let impl_block = match &hir.item(hir.items[0]).kind {
        ItemKind::Impl(impl_block) => impl_block,
        other => panic!("expected impl item, got {other:?}"),
    };
    let method = &impl_block.methods[0];
    let ql_hir::Param::Receiver(receiver) = &method.params[0] else {
        panic!("expected receiver parameter");
    };

    assert_eq!(&source[receiver.span.start..receiver.span.end], "var self");
}

#[test]
fn lower_module_preserves_member_name_spans() {
    let source = r#"
fn main() {
    user.name
}
"#;
    let ast = parse_source(source).expect("source should parse");
    let hir = lower_module(&ast);
    let function = match &hir.item(hir.items[0]).kind {
        ItemKind::Function(function) => function,
        other => panic!("expected function item, got {other:?}"),
    };
    let body = hir.block(function.body.expect("function should have body"));
    let expr_id = body
        .tail
        .expect("member expression should be the block tail");
    let expr = hir.expr(expr_id);

    let ExprKind::Member {
        field, field_span, ..
    } = &expr.kind
    else {
        panic!("expected member expression");
    };

    assert_eq!(field, "name");
    assert_eq!(&source[field_span.start..field_span.end], "name");
}
