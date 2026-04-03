use make_env as env

struct Inner {
    pair: (Int, Int),
}

struct Env {
    inner: Inner,
}

fn make_env() -> Env {
    return Env { inner: Inner { pair: (1, 2) } }
}

async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    let first = env().inner.pair[0] = await worker(8)
    let second = env().inner.pair[1] = first + 6
    return first + second
}
