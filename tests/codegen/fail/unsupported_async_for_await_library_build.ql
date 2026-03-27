async fn helper() -> Int {
    for await value in [1, 2, 3] {
        break
    }
    return 0
}
