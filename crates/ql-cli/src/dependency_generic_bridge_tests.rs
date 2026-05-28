use super::*;

fn parse_module(source: &str) -> Module {
    ql_parser::parse_source(source).expect("test source should parse")
}

fn function<'a>(module: &'a Module, name: &str) -> &'a FunctionDecl {
    module
        .items
        .iter()
        .find_map(|item| match &item.kind {
            ItemKind::Function(function) if function.name == name => Some(function),
            _ => None,
        })
        .expect("test function should exist")
}

#[test]
fn public_specialization_reports_not_called_for_unused_imported_generic() {
    let dependency_source = r#"
package dep

pub fn identity[T](value: T) -> T {
    return value
}
"#;
    let dependency = parse_module(dependency_source);
    let root = parse_module(
        r#"
use dep.identity as identity

fn main() -> Int {
    return 0
}
"#,
    );

    let status = render_public_function_specialization_status_with_context(
        &["dep".to_owned()],
        function(&dependency, "identity"),
        dependency_source,
        &root,
        &dependency,
        &[],
        &mut BTreeSet::new(),
    );

    assert_eq!(status, PublicFunctionSpecializationRender::NotCalled);
}

#[test]
fn public_specialization_reports_unsupported_for_incomplete_substitutions() {
    let dependency_source = r#"
package dep

pub fn make[T]() -> T {
    return 0
}
"#;
    let dependency = parse_module(dependency_source);
    let root = parse_module(
        r#"
use dep.make as make

fn main() -> Int {
    make()
    return 0
}
"#,
    );

    let status = render_public_function_specialization_status_with_context(
        &["dep".to_owned()],
        function(&dependency, "make"),
        dependency_source,
        &root,
        &dependency,
        &[],
        &mut BTreeSet::new(),
    );

    assert_eq!(status, PublicFunctionSpecializationRender::Unsupported);
}

#[test]
fn public_specialization_rewrites_same_dependency_generic_body_calls() {
    let dependency_source = r#"
package dep

pub fn first[T, N](values: [T; N]) -> T {
    return values[0]
}

pub fn first_wrapped[T, N](values: [T; N]) -> T {
    return first(values)
}
"#;
    let dependency = parse_module(dependency_source);
    let root = parse_module(
        r#"
use dep.first_wrapped as first_wrapped

fn main() -> Int {
    return first_wrapped([1, 2, 3])
}
"#,
    );

    let rendered = render_public_function_specializations(
        &["dep".to_owned()],
        function(&dependency, "first_wrapped"),
        dependency_source,
        &root,
        &dependency,
        &mut BTreeSet::new(),
    )
    .expect("first_wrapped should render a concrete specialization");

    assert!(
        rendered
            .declarations
            .contains("fn __ql_bridge_local_dep_first__generic_Int_3(values: [Int; 3]) -> Int")
    );
    assert!(
        rendered
            .declarations
            .contains("return __ql_bridge_local_dep_first__generic_Int_3(values)")
    );
    assert!(!rendered.declarations.contains("return first(values)"));
    assert_eq!(rendered.call_rewrites.len(), 1);
    assert_eq!(
        rendered.call_rewrites[0].replacement,
        "__ql_bridge_local_dep_first_wrapped__generic_Int_3"
    );
}

#[test]
fn public_specialization_rewrites_helper_after_typed_local_substitution() {
    let dependency_source = r#"
package dep

pub fn helper[T](value: T) -> T {
    return value
}

pub fn wrapped[T](value: T) -> T {
    let local: T = value
    return helper(local)
}
"#;
    let dependency = parse_module(dependency_source);
    let root = parse_module(
        r#"
use dep.wrapped as wrapped

fn main() -> Int {
    return wrapped(1)
}
"#,
    );

    let rendered = render_public_function_specializations(
        &["dep".to_owned()],
        function(&dependency, "wrapped"),
        dependency_source,
        &root,
        &dependency,
        &mut BTreeSet::new(),
    )
    .expect("wrapped should render a concrete specialization");

    assert!(
        rendered
            .declarations
            .contains("fn __ql_bridge_local_dep_helper__generic_Int(value: Int) -> Int")
    );
    assert!(rendered.declarations.contains("let local: Int = value"));
    assert!(
        rendered
            .declarations
            .contains("return __ql_bridge_local_dep_helper__generic_Int(local)")
    );
    assert!(!rendered.declarations.contains("__generic_T"));
    assert_eq!(rendered.call_rewrites.len(), 1);
    assert_eq!(
        rendered.call_rewrites[0].replacement,
        "__ql_bridge_local_dep_wrapped__generic_Int"
    );
}

#[test]
fn public_specialization_rewrites_helper_after_generic_carrier_pattern_binding() {
    let dependency_source = r#"
package dep

pub enum Option[T] {
    Some(T),
    None,
}

pub fn helper[T](value: T) -> T {
    return value
}

pub fn wrapped[T](value: Option[T], fallback: T) -> T {
    return match value {
        Option.Some(inner) => helper(inner),
        Option.None => fallback,
    }
}
"#;
    let dependency = parse_module(dependency_source);
    let root = parse_module(
        r#"
use dep.Option as Option
use dep.wrapped as wrapped

fn main() -> Int {
    return wrapped(Option.Some(1), 0)
}
"#,
    );

    let rendered = render_public_function_specializations(
        &["dep".to_owned()],
        function(&dependency, "wrapped"),
        dependency_source,
        &root,
        &dependency,
        &mut BTreeSet::new(),
    )
    .expect("wrapped should render a concrete specialization");

    assert!(
        rendered
            .declarations
            .contains("fn __ql_bridge_local_dep_helper__generic_Int(value: Int) -> Int")
    );
    assert!(
        rendered
            .declarations
            .contains("Option.Some(inner) => __ql_bridge_local_dep_helper__generic_Int(inner)")
    );
    assert!(!rendered.declarations.contains("__generic_T"));
    assert_eq!(rendered.call_rewrites.len(), 1);
    assert_eq!(
        rendered.call_rewrites[0].replacement,
        "__ql_bridge_local_dep_wrapped__generic_Int"
    );
}

#[test]
fn public_specialization_preserves_non_generic_body_helper_calls() {
    let dependency_source = r#"
package dep

pub fn helper(value: Int) -> Int {
    return value + 1
}

pub fn wrapped[T](value: T, count: Int) -> Int {
    return helper(count)
}
"#;
    let dependency = parse_module(dependency_source);
    let root = parse_module(
        r#"
use dep.wrapped as wrapped

fn main() -> Int {
    return wrapped(0, 1)
}
"#,
    );

    let rendered = render_public_function_specializations(
        &["dep".to_owned()],
        function(&dependency, "wrapped"),
        dependency_source,
        &root,
        &dependency,
        &mut BTreeSet::new(),
    )
    .expect("wrapped should render a concrete specialization");

    assert!(
        rendered.declarations.contains(
            "fn __ql_bridge_local_dep_wrapped__generic_Int(value: Int, count: Int) -> Int"
        )
    );
    assert!(rendered.declarations.contains("return helper(count)"));
    assert!(
        !rendered
            .declarations
            .contains("__ql_bridge_local_dep_helper")
    );
    assert_eq!(rendered.call_rewrites.len(), 1);
    assert_eq!(
        rendered.call_rewrites[0].replacement,
        "__ql_bridge_local_dep_wrapped__generic_Int"
    );
}

#[test]
fn public_specialization_preserves_unresolved_array_length_generics() {
    let dependency_source = r#"
package dep

pub fn total[N](values: [Int; N]) -> Int {
    var sum = 0
    for value in values {
        sum = sum + value
    }
    return sum
}

pub fn mirror_sum[N](values: [Int; N]) -> Int {
    return total(values)
}
"#;
    let dependency = parse_module(dependency_source);
    let root = parse_module(
        r#"
use dep.mirror_sum as mirror_sum

fn score[N](values: [Int; N]) -> Int {
    return mirror_sum(values)
}
"#,
    );

    let rendered = render_public_function_specializations(
        &["dep".to_owned()],
        function(&dependency, "mirror_sum"),
        dependency_source,
        &root,
        &dependency,
        &mut BTreeSet::new(),
    )
    .expect("mirror_sum should render a length-generic specialization");

    assert!(
        rendered
            .declarations
            .contains("fn __ql_bridge_local_dep_mirror_sum__generic_N[N](values: [Int; N]) -> Int")
    );
    assert!(
        rendered
            .declarations
            .contains("fn __ql_bridge_local_dep_total__generic_N[N](values: [Int; N]) -> Int")
    );
    assert!(
        rendered
            .declarations
            .contains("return __ql_bridge_local_dep_total__generic_N(values)")
    );
    assert_eq!(rendered.call_rewrites.len(), 1);
    assert_eq!(
        rendered.call_rewrites[0].replacement,
        "__ql_bridge_local_dep_mirror_sum__generic_N"
    );
}

#[test]
fn public_specialization_rewrites_imported_dependency_generic_body_calls() {
    let helper_source = r#"
package helper

pub fn reverse_array[T, N](values: [T; N]) -> [T; N] {
    return values
}
"#;
    let dependency_source = r#"
package dep

use helper.reverse_array as rev

pub fn reverse_wrapped[T, N](values: [T; N]) -> [T; N] {
    return rev(values)
}
"#;
    let helper = parse_module(helper_source);
    let dependency = parse_module(dependency_source);
    let root = parse_module(
        r#"
use dep.reverse_wrapped as reverse_wrapped

fn run() -> [String; 3] {
    return reverse_wrapped(["north", "east", "south"])
}
"#,
    );
    let helper_import_path = vec!["helper".to_owned()];
    let helper_module = SpecializationModule {
        module_import_path: &helper_import_path,
        contents: helper_source,
        module: &helper,
    };

    let rendered = render_public_function_specialization_status_with_context(
        &["dep".to_owned()],
        function(&dependency, "reverse_wrapped"),
        dependency_source,
        &root,
        &dependency,
        &[helper_module],
        &mut BTreeSet::new(),
    );
    let PublicFunctionSpecializationRender::Rendered(rendered) = rendered else {
        panic!("reverse_wrapped should render imported helper specialization");
    };

    assert!(rendered.declarations.contains(
        "fn __ql_bridge_local_helper_reverse_array__generic_String_3(values: [String; 3]) -> [String; 3]"
    ));
    assert!(
        rendered
            .declarations
            .contains("return __ql_bridge_local_helper_reverse_array__generic_String_3(values)")
    );
    assert!(!rendered.declarations.contains("return rev(values)"));
    assert_eq!(rendered.call_rewrites.len(), 1);
    assert_eq!(
        rendered.call_rewrites[0].replacement,
        "__ql_bridge_local_dep_reverse_wrapped__generic_String_3"
    );
}

#[test]
fn public_specialization_rewrites_grouped_imported_dependency_generic_body_calls() {
    let helper_source = r#"
package helper

pub fn wrap_value[T](value: T) -> T {
    return value
}
"#;
    let dependency_source = r#"
package dep

use helper.{wrap_value as wrap}

pub fn wrapped[T](value: T) -> T {
    return wrap(value)
}
"#;
    let helper = parse_module(helper_source);
    let dependency = parse_module(dependency_source);
    let root = parse_module(
        r#"
use dep.wrapped as wrapped

fn run() -> Bool {
    return wrapped(true)
}
"#,
    );
    let helper_import_path = vec!["helper".to_owned()];
    let helper_module = SpecializationModule {
        module_import_path: &helper_import_path,
        contents: helper_source,
        module: &helper,
    };

    let rendered = render_public_function_specialization_status_with_context(
        &["dep".to_owned()],
        function(&dependency, "wrapped"),
        dependency_source,
        &root,
        &dependency,
        &[helper_module],
        &mut BTreeSet::new(),
    );
    let PublicFunctionSpecializationRender::Rendered(rendered) = rendered else {
        panic!("wrapped should render grouped imported helper specialization");
    };

    assert!(
        rendered
            .declarations
            .contains("fn __ql_bridge_local_helper_wrap_value__generic_Bool(value: Bool) -> Bool")
    );
    assert!(
        rendered
            .declarations
            .contains("return __ql_bridge_local_helper_wrap_value__generic_Bool(value)")
    );
    assert!(!rendered.declarations.contains("return wrap(value)"));
    assert_eq!(rendered.call_rewrites.len(), 1);
    assert_eq!(
        rendered.call_rewrites[0].replacement,
        "__ql_bridge_local_dep_wrapped__generic_Bool"
    );
}
