use worker as async_alias
use APPLY as async_const_alias

extern "c" fn sink(value: Int)

async fn worker(value: Int) -> Int {
    return value + 10
}

const APPLY: (Int) -> Task[Int] = worker

async fn main() -> Int {
    let branch = true
    match await (if branch { async_alias } else { async_const_alias })(3) {
        13 => sink(1),
        _ => sink(0),
    }
    match await (match branch { true => async_const_alias, false => async_alias })(4) {
        14 => sink(2),
        _ => sink(3),
    }
    return 0
}
