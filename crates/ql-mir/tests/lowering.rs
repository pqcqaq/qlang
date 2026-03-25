use ql_hir::lower_module as lower_hir;
use ql_mir::{lower_module as lower_mir, render_module};
use ql_parser::parse_source;
use ql_resolve::resolve_module;

fn render(source: &str) -> String {
    let ast = parse_source(source).expect("source should parse");
    let hir = lower_hir(&ast);
    let resolution = resolve_module(&hir);
    let mir = lower_mir(&hir, &resolution);
    render_module(&mir, &hir)
}

#[test]
fn lowers_linear_bindings_and_return() {
    let rendered = render(
        r#"
fn main() -> Int {
    let value = 1
    return value
}
"#,
    );

    assert!(rendered.contains("body 0 main"));
    assert!(rendered.contains("bind_pattern value <- 1"));
    assert!(rendered.contains("assign l0($return) = l"));
    assert!(rendered.contains("return"));
}

#[test]
fn lowers_defer_cleanup_in_lifo_order() {
    let rendered = render(
        r#"
fn main() -> Int {
    defer first()
    {
        defer second()
    }
    return 0
}
"#,
    );

    let inner = rendered
        .find("run_cleanup c1")
        .expect("inner cleanup should be rendered");
    let outer = rendered
        .find("run_cleanup c0")
        .expect("outer cleanup should be rendered");

    assert!(rendered.contains("register_cleanup c0"));
    assert!(rendered.contains("register_cleanup c1"));
    assert!(inner < outer, "inner defer should run before outer defer");
}

#[test]
fn lowers_loop_control_flow_edges() {
    let rendered = render(
        r#"
fn main() -> Int {
    var total = 0
    while total < 3 {
        total = total + 1
        if total == 2 {
            continue
        }
        break
    }
    return total
}
"#,
    );

    assert!(rendered.contains("branch"));
    assert!(rendered.contains("goto bb"));
    assert!(rendered.contains("storage_live"));
    assert!(rendered.contains("storage_dead"));
}

#[test]
fn keeps_match_and_for_as_structural_mir_terminators() {
    let rendered = render(
        r#"
fn main(stream: Stream, command: Command) -> Int {
    for await event in stream {
        match command {
            Command.Quit => 0,
            _ => 1,
        }
    }
    return 1
}
"#,
    );

    assert!(rendered.contains("for await"));
    assert!(rendered.contains("match "));
    assert!(rendered.contains("bind_pattern event <-"));
}

#[test]
fn materializes_explicit_closure_capture_facts() {
    let rendered = render(
        r#"
fn main() -> Int {
    let value = 1
    let make = move (extra) => value + extra
    return 0
}
"#,
    );

    assert!(rendered.contains("[captures: value@"));
    assert!(!rendered.contains("[captures: extra@"));
}

#[test]
fn assigns_stable_closure_ids_in_mir_output() {
    let rendered = render(
        r#"
fn main() -> Int {
    let base = 1
    let add = move (x) => x + base
    return add(2)
}
"#,
    );

    assert!(rendered.contains("closures:"));
    assert!(rendered.contains("cl0 move [captures: base@"));
    assert!(rendered.contains("assign l"));
    assert!(rendered.contains("= closure cl0 move [captures: base@"));
}
