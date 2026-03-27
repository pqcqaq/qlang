mod support;

use ql_hir::{ExprKind, Param, StmtKind, TypeKind};
use ql_resolve::{BuiltinType, TypeResolution};

use support::{find_function, find_item_id, path, resolved};

#[test]
fn resolves_builtin_types_in_function_signatures() {
    let (module, resolution) = resolved(
        r#"
fn render(flag: Bool) -> String {
    "ok"
}
"#,
    );

    let function = find_function(&module, "render");
    let Param::Regular(param) = &function.params[0] else {
        panic!("function should have a regular parameter");
    };
    let return_type = function
        .return_type
        .expect("function should declare a return type");

    assert_eq!(
        resolution.type_resolution(param.ty),
        Some(&TypeResolution::Builtin(BuiltinType::Bool))
    );
    assert_eq!(
        resolution.type_resolution(return_type),
        Some(&TypeResolution::Builtin(BuiltinType::String))
    );
}

#[test]
fn resolves_generic_type_parameters_in_signatures() {
    let (module, resolution) = resolved(
        r#"
fn id[T](value: T) -> T {
    value
}
"#,
    );

    let function = find_function(&module, "id");
    let Param::Regular(param) = &function.params[0] else {
        panic!("function should have a regular parameter");
    };
    let return_type = function
        .return_type
        .expect("function should declare a return type");

    assert!(
        matches!(
            resolution.type_resolution(param.ty),
            Some(TypeResolution::Generic(binding)) if binding.index == 0
        ),
        "parameter type should resolve to the first generic binding"
    );
    assert!(
        matches!(
            resolution.type_resolution(return_type),
            Some(TypeResolution::Generic(binding)) if binding.index == 0
        ),
        "return type should resolve to the first generic binding"
    );
}

#[test]
fn resolves_builtin_types_inside_array_signatures() {
    let (module, resolution) = resolved(
        r#"
fn render(flags: [Bool; 0b100]) -> [String; 0x1] {
    ["ok"]
}
"#,
    );

    let function = find_function(&module, "render");
    let Param::Regular(param) = &function.params[0] else {
        panic!("function should have a regular parameter");
    };
    let return_type = function
        .return_type
        .expect("function should declare a return type");

    let TypeKind::Array {
        element: param_element,
        len: param_len,
    } = &module.ty(param.ty).kind
    else {
        panic!("parameter type should lower to array type");
    };
    let TypeKind::Array {
        element: return_element,
        len: return_len,
    } = &module.ty(return_type).kind
    else {
        panic!("return type should lower to array type");
    };

    assert_eq!(*param_len, 4);
    assert_eq!(*return_len, 1);
    assert_eq!(
        resolution.type_resolution(*param_element),
        Some(&TypeResolution::Builtin(BuiltinType::Bool))
    );
    assert_eq!(
        resolution.type_resolution(*return_element),
        Some(&TypeResolution::Builtin(BuiltinType::String))
    );
}

#[test]
fn resolves_import_aliases_in_type_positions() {
    let source = r#"
use std.collections.HashMap as Map

fn build(cache: Map[String, Int]) -> Map[String, Int] {
    cache
}
"#;
    let (module, resolution) = resolved(source);

    let function = find_function(&module, "build");
    let Param::Regular(param) = &function.params[0] else {
        panic!("function should have a regular parameter");
    };
    let return_type = function
        .return_type
        .expect("function should declare a return type");

    let alias_span = source
        .find("as Map")
        .map(|offset| ql_span::Span::new(offset + 3, offset + 6))
        .expect("import alias definition should exist");

    assert!(
        matches!(
            resolution.type_resolution(param.ty),
            Some(TypeResolution::Import(binding))
                if binding.path == path(&["std", "collections", "HashMap"])
                    && binding.local_name == "Map"
                    && binding.definition_span == alias_span
        ),
        "parameter type should resolve to the source-backed import alias binding"
    );
    assert!(
        matches!(
            resolution.type_resolution(return_type),
            Some(TypeResolution::Import(binding))
                if binding.path == path(&["std", "collections", "HashMap"])
                    && binding.local_name == "Map"
                    && binding.definition_span == alias_span
        ),
        "return type should resolve to the same source-backed import alias binding"
    );
}

#[test]
fn resolves_struct_literal_paths_to_their_declared_item() {
    let (module, resolution) = resolved(
        r#"
struct User {
    name: String,
}

fn make(name: String) -> User {
    User { name }
}
"#,
    );

    let user_item = find_item_id(&module, "User");
    let function = find_function(&module, "make");
    let body = module.block(function.body.expect("function should have body"));
    let tail = body
        .tail
        .expect("function body should have a tail expression");
    let ExprKind::StructLiteral { .. } = &module.expr(tail).kind else {
        panic!("function tail should be a struct literal");
    };

    assert_eq!(
        resolution.struct_literal_resolution(tail),
        Some(&TypeResolution::Item(user_item))
    );
}

#[test]
fn accepts_reserved_task_handle_type_roots_without_unresolved_diagnostics() {
    let (module, resolution) = resolved(
        r#"
fn schedule(task: Task[Int]) -> Task[Int] {
    task
}
"#,
    );

    let function = find_function(&module, "schedule");
    let Param::Regular(param) = &function.params[0] else {
        panic!("function should have a regular parameter");
    };
    let return_type = function
        .return_type
        .expect("function should declare a return type");

    assert_eq!(
        resolution.type_resolution(param.ty),
        None,
        "reserved Task[T] roots should stay out of the resolver map"
    );
    assert_eq!(
        resolution.type_resolution(return_type),
        None,
        "reserved Task[T] return roots should stay out of the resolver map"
    );
    assert!(
        resolution
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.message != "unresolved type `Task`"),
        "Task[T] should not produce unresolved-type diagnostics"
    );
}

#[test]
fn reports_unresolved_bare_named_types() {
    let (module, resolution) = resolved(
        r#"
fn build(input: Missing) -> Int {
    0
}
"#,
    );

    let function = find_function(&module, "build");
    let Param::Regular(param) = &function.params[0] else {
        panic!("function should have a regular parameter");
    };

    assert_eq!(
        resolution.type_resolution(param.ty),
        None,
        "unresolved bare named types should stay unresolved in the map"
    );
    assert!(
        resolution
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == "unresolved type `Missing`"),
        "resolver should emit a conservative unresolved-type diagnostic for bare named types"
    );
}

#[test]
fn reports_unresolved_single_segment_struct_literal_roots() {
    let (module, resolution) = resolved(
        r#"
fn main() -> Int {
    Missing {};
    0
}
"#,
    );

    let function = find_function(&module, "main");
    let body = module.block(function.body.expect("function should have a body"));
    let StmtKind::Expr { expr, .. } = &module.stmt(body.statements[0]).kind else {
        panic!("first statement should be an expression statement");
    };
    let ExprKind::StructLiteral { .. } = &module.expr(*expr).kind else {
        panic!("first statement should be a struct literal");
    };

    assert_eq!(
        resolution.struct_literal_resolution(*expr),
        None,
        "unresolved bare struct literal roots should stay unresolved in the map"
    );
    assert!(
        resolution
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message == "unresolved type `Missing`"),
        "single-segment struct literal roots should emit a conservative unresolved-type diagnostic"
    );
}

#[test]
fn defers_multi_segment_type_path_diagnostics_until_module_resolution_exists() {
    let (module, resolution) = resolved(
        r#"
fn build(input: pkg.Missing) -> Int {
    0
}
"#,
    );

    let function = find_function(&module, "build");
    let Param::Regular(param) = &function.params[0] else {
        panic!("function should have a regular parameter");
    };

    assert_eq!(resolution.type_resolution(param.ty), None);
    assert!(
        resolution
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.message != "unresolved type `pkg`"),
        "multi-segment type paths should stay out of scope until module-path resolution exists"
    );
}
