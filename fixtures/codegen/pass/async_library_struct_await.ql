struct Pair {
    left: Bool,
    right: Int,
}

async fn worker() -> Pair {
    return Pair { right: 42, left: true }
}

async fn helper() -> Pair {
    return await worker()
}
