mod support;

use support::diagnostic_messages;

#[test]
fn reports_await_outside_async_functions() {
    let diagnostics = diagnostic_messages(
        r#"
fn worker() -> Int {
    return 1
}

fn main() -> Int {
    return await worker()
}
"#,
    );

    assert!(
        diagnostics.contains(&"`await` is only allowed inside `async fn`".to_string()),
        "expected async-boundary diagnostic, got {diagnostics:?}"
    );
}

#[test]
fn reports_spawn_outside_async_functions() {
    let diagnostics = diagnostic_messages(
        r#"
fn worker() -> Int {
    return 1
}

fn main() -> Int {
    spawn worker()
    return 0
}
"#,
    );

    assert!(
        diagnostics.contains(&"`spawn` is only allowed inside `async fn`".to_string()),
        "expected async-boundary diagnostic, got {diagnostics:?}"
    );
}

#[test]
fn allows_await_and_spawn_inside_async_functions() {
    let diagnostics = diagnostic_messages(
        r#"
fn worker() -> Int {
    return 1
}

async fn main() -> Int {
    spawn worker()
    return await worker()
}
"#,
    );

    assert!(
        diagnostics
            .iter()
            .all(|message| !message.contains("only allowed inside `async fn`")),
        "did not expect async-boundary diagnostics in async function, got {diagnostics:?}"
    );
}

#[test]
fn reports_for_await_outside_async_functions() {
    let diagnostics = diagnostic_messages(
        r#"
fn main() -> Int {
    for await value in [1, 2, 3] {
        let current = value
    }
    return 0
}
"#,
    );

    assert!(
        diagnostics.contains(&"`for await` is only allowed inside `async fn`".to_string()),
        "expected async-boundary diagnostic, got {diagnostics:?}"
    );
}

#[test]
fn allows_for_await_inside_async_functions() {
    let diagnostics = diagnostic_messages(
        r#"
async fn main() -> Int {
    for await value in [1, 2, 3] {
        let current = value
    }
    return 0
}
"#,
    );

    assert!(
        diagnostics
            .iter()
            .all(|message| !message.contains("`for await` is only allowed inside `async fn`")),
        "did not expect for-await boundary diagnostics in async function, got {diagnostics:?}"
    );
}
