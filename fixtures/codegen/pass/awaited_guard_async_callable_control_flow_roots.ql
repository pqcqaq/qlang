use worker as async_alias
use APPLY as async_const_alias

async fn worker(value: Int) -> Int {
    return value + 10
}

const APPLY: (Int) -> Task[Int] = worker

async fn main() -> Int {
    let branch = true
    return match 1 {
        1 if await (if branch { async_alias } else { async_const_alias })(3) == 13 => 10,
        1 if await (match branch { true => async_const_alias, false => async_alias })(4) == 14 => 20,
        _ => 0,
    }
}
