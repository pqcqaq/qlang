use ql_borrowck::{analyze_module as analyze_borrowck, render_result};
use ql_hir::lower_module as lower_hir;
use ql_mir::lower_module as lower_mir;
use ql_parser::parse_source;
use ql_resolve::resolve_module;
use ql_typeck::analyze_module as analyze_types;

fn diagnostic_messages(source: &str) -> Vec<String> {
    let ast = parse_source(source).expect("source should parse");
    let hir = lower_hir(&ast);
    let resolution = resolve_module(&hir);
    let typeck = analyze_types(&hir, &resolution);
    let mir = lower_mir(&hir, &resolution);
    let borrowck = analyze_borrowck(&hir, &resolution, &typeck, &mir);

    borrowck
        .diagnostics()
        .iter()
        .map(|diagnostic| diagnostic.message.clone())
        .collect()
}

#[test]
fn reports_use_after_move_from_move_self_method() {
    let diagnostics = diagnostic_messages(
        r#"
struct User {
    name: String,
}

impl User {
    fn into_json(move self) -> String {
        return self.name
    }
}

fn main() -> String {
    let user = User { name: "ql" }
    user.into_json()
    return user.name
}
"#,
    );

    assert!(diagnostics.contains(&"local `user` was used after move".to_string()));
}

#[test]
fn reports_maybe_moved_after_branch_join() {
    let diagnostics = diagnostic_messages(
        r#"
struct User {
    name: String,
}

impl User {
    fn into_json(move self) -> String {
        return self.name
    }
}

fn main(flag: Bool) -> String {
    let user = User { name: "ql" }
    if flag {
        user.into_json()
    } else {
        ""
    }
    return user.name
}
"#,
    );

    assert!(
        diagnostics
            .contains(&"local `user` may have been moved on another control-flow path".to_string())
    );
}

#[test]
fn reassigning_a_local_makes_it_available_again() {
    let diagnostics = diagnostic_messages(
        r#"
struct User {
    name: String,
}

impl User {
    fn into_json(move self) -> String {
        return self.name
    }
}

fn fresh_user() -> User {
    return User { name: "new" }
}

fn main() -> String {
    let user = User { name: "old" }
    user.into_json();
    user = fresh_user();
    return user.name
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got {diagnostics:?}"
    );
}

#[test]
fn readonly_and_mutable_receivers_do_not_count_as_move() {
    let diagnostics = diagnostic_messages(
        r#"
struct Counter {
    value: Int,
}

impl Counter {
    fn read(self) -> Int {
        return self.value
    }

    fn bump(var self) -> Int {
        self.value = self.value + 1
        return self.value
    }
}

fn main() -> Int {
    let counter = Counter { value: 1 }
    counter.bump()
    return counter.read()
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics, got {diagnostics:?}"
    );
}

#[test]
fn ambiguous_method_candidates_do_not_trigger_consumption() {
    let diagnostics = diagnostic_messages(
        r#"
struct User {
    name: String,
}

impl User {
    fn into_json(move self) -> String {
        return self.name
    }
}

extend User {
    fn into_json(move self) -> String {
        return self.name
    }
}

fn main() -> String {
    let user = User { name: "ql" }
    user.into_json();
    return user.name
}
"#,
    );

    assert!(
        diagnostics.is_empty(),
        "expected no diagnostics for ambiguous method candidates, got {diagnostics:?}"
    );
}

#[test]
fn renders_block_state_facts_for_debugging() {
    let source = r#"
struct User {
    name: String,
}

impl User {
    fn into_json(move self) -> String {
        return self.name
    }
}

fn main() -> String {
    let user = User { name: "ql" }
    user.into_json()
    return user.name
}
"#;
    let ast = parse_source(source).expect("source should parse");
    let hir = lower_hir(&ast);
    let resolution = resolve_module(&hir);
    let typeck = analyze_types(&hir, &resolution);
    let mir = lower_mir(&hir, &resolution);
    let borrowck = analyze_borrowck(&hir, &resolution, &typeck, &mir);
    let rendered = render_result(&borrowck, &mir);

    assert!(rendered.contains("ownership main"));
    assert!(rendered.contains("bb0 in=["));
    assert!(rendered.contains("consume(move self into_json)"));
}
