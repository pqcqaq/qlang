use APPLY_CONST as run_const
use APPLY_STATIC as run_static

const APPLY_CONST: (Int) -> Int = (value: Int) => value + 1
static APPLY_STATIC: (Int) -> Int = (value: Int) => value + 2

fn main() -> Int {
    let f = run_const
    let g = run_static
    return APPLY_CONST(10) + APPLY_STATIC(20) + f(30) + g(40)
}
