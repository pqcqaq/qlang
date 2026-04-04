use READY as ready
use AMOUNT as amount

extern "c" fn sink(value: Int)
extern "c" fn second()

fn enabled() -> Bool {
    return true
}

fn measure() -> Int {
    return 7
}

const READY: () -> Bool = enabled
const AMOUNT: () -> Int = measure

fn main() -> Int {
    let flag = true
    defer match flag {
        true if ready() => sink(amount()),
        _ => second(),
    }
    return 0
}
