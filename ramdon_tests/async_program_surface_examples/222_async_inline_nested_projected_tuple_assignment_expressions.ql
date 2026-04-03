struct Inner {
    pair: (Int, Int),
}

struct Env {
    inner: Inner,
}

async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    let first = (Env { inner: Inner { pair: (1, 2) } }).inner.pair[0] = await worker(8)
    let second = (Env { inner: Inner { pair: (1, 2) } }).inner.pair[1] = first + 6
    return first + second
}
