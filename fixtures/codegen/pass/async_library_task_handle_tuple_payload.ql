async fn left() -> Int {
    return 1
}

async fn right() -> Int {
    return 2
}

async fn outer() -> (Task[Int], Task[Int]) {
    return (left(), right())
}

async fn helper() -> Int {
    let pair = await outer()
    let first = await pair[0]
    let second = await pair[1]
    return first + second
}
