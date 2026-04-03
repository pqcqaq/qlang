struct Inner {
    pair: (Int, Int),
}

struct Env {
    inner: Inner,
}

fn make_env() -> Env {
    return Env { inner: Inner { pair: (1, 2) } }
}

fn main() -> Int {
    let first = make_env().inner.pair[0] = 7
    let second = make_env().inner.pair[1] = first + 6
    return first + second
}
