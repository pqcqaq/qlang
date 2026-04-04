use APPLY as const_alias
use APPLY_CLOSURE_CONST as closure_const
use APPLY_CLOSURE_STATIC as closure_static

fn add_one(value: Int) -> Int {
    return value + 1
}

const APPLY: (Int) -> Int = add_one
const APPLY_CLOSURE_CONST: (Int) -> Int = (value: Int) => value + 2
static APPLY_CLOSURE_STATIC: (Int) -> Int = (value: Int) => value + 3

fn main() -> Int {
    let branch = true
    defer (if branch { const_alias } else { closure_const })(40)
    defer (match branch {
        true => closure_static,
        false => const_alias,
    })(1)
    return match 1 {
        1 if (if branch { closure_const } else { const_alias })(1) == 3 => 10,
        1 if (match branch { true => closure_static, false => const_alias })(2) == 5 => 20,
        _ => 0,
    }
}
