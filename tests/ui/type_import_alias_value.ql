use add as plus
use APPLY as run
use VALUE as current

fn add(left: Int, right: Int) -> Int {
    return left + right
}

const APPLY: (Int) -> Int = (value) => value + 1
const VALUE: Int = 1

fn main() -> Int {
    current();
    plus(left: 1, right: true);
    run("x");
    return 0
}
