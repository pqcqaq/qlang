use APPLY_CONST as run_const
use APPLY_STATIC as run_static

fn add_one(value: Int) -> Int {
    return value + 1
}

const APPLY_CONST: (Int) -> Int = add_one
static APPLY_STATIC: (Int) -> Int = add_one

fn main() -> Int {
    let f = run_const
    let g = run_static
    return APPLY_CONST(10) + APPLY_STATIC(20) + f(30) + g(40)
}
