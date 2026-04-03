struct Inner {
    pair: (Int, Int),
}

struct Env {
    inner: Inner,
}

fn main() -> Int {
    let first = (Env { inner: Inner { pair: (1, 2) } }).inner.pair[0] = 7
    let second = (Env { inner: Inner { pair: (1, 2) } }).inner.pair[1] = first + 6
    return first + second
}
