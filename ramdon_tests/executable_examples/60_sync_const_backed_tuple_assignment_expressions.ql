use INDEXES as ALIAS

struct Slots {
    left: Int,
    right: Int,
}

const BASE: Int = 0
const NEXT: Int = BASE + 1
const INDEXES: Slots = Slots { left: BASE, right: NEXT }
static EDGE: Int = NEXT

fn main() -> Int {
    var pair = (1, 2)
    let first = pair[ALIAS.left + 0] = 7
    let second = pair[EDGE - 0] = first + 6
    return pair[NEXT - 1] + second
}
