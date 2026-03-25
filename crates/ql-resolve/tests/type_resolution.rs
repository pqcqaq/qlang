mod support;

use ql_hir::{ExprKind, Param};
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
fn resolves_import_aliases_in_type_positions() {
    let (module, resolution) = resolved(
        r#"
use std.collections.HashMap as Map

fn build(cache: Map[String, Int]) -> Map[String, Int] {
    cache
}
"#,
    );

    let function = find_function(&module, "build");
    let Param::Regular(param) = &function.params[0] else {
        panic!("function should have a regular parameter");
    };
    let return_type = function
        .return_type
        .expect("function should declare a return type");

    assert_eq!(
        resolution.type_resolution(param.ty),
        Some(&TypeResolution::Import(path(&[
            "std",
            "collections",
            "HashMap"
        ])))
    );
    assert_eq!(
        resolution.type_resolution(return_type),
        Some(&TypeResolution::Import(path(&[
            "std",
            "collections",
            "HashMap"
        ])))
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
