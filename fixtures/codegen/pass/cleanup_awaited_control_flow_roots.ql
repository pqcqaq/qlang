use ready as async_alias
use APPLY as async_const_alias

extern "c" fn sink(value: Int)

async fn ready(value: Int) -> Bool {
    return value == 1
}

const APPLY: (Int) -> Task[Bool] = ready

async fn main() -> Int {
    let branch = true
    defer if await (if branch { async_alias } else { async_const_alias })(1) {
        sink(1);
    }
    defer match await (match branch { true => async_const_alias, false => async_alias })(1) {
        true => sink(2),
        false => sink(3),
    }
    return 0
}
