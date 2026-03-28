struct Wrap {
    values: [Int; 0],
}

async fn empty_values() -> [Int; 0] {
    return []
}

async fn wrapped() -> Wrap {
    return Wrap { values: [] }
}

async fn helper_values() -> [Int; 0] {
    return await empty_values()
}

async fn helper_wrap() -> Wrap {
    return await wrapped()
}
