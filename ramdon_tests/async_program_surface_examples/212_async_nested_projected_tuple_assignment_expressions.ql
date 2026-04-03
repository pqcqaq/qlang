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
    var env = Env { inner: Inner { pair: (1, 2) } }
    let first = env.inner.pair[0] = await worker(8)
    let second = env.inner.pair[1] = first + 6
    return env.inner.pair[0] + second
}
