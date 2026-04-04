use APPLY_CONST as run_const
use APPLY_STATIC as run_static

async fn worker(value: Int) -> Int {
    return value + 1
}

const APPLY_CONST: (Int) -> Task[Int] = worker
static APPLY_STATIC: (Int) -> Task[Int] = worker

async fn main() -> Int {
    let f = run_const
    let g = run_static
    let first = await APPLY_CONST(10)
    let second = await APPLY_STATIC(20)
    let third = await f(30)
    let fourth = await g(40)
    return first + second + third + fourth
}
