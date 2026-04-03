use INDEXES as ALIAS

struct Slots {
    left: Int,
    right: Int,
}

const BASE: Int = 0
const NEXT: Int = BASE + 1
const INDEXES: Slots = Slots { left: BASE, right: NEXT }
static EDGE: Int = NEXT

async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    var pair = (1, 2)
    let left = ALIAS.left + 0
    let right = EDGE - 0
    let first = pair[left] = await worker(8)
    let second = pair[right] = first + 6
    return pair[NEXT - 1] + second
}
