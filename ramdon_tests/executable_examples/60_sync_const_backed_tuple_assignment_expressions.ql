use INDEXES as ALIAS

struct Slots {
    left: Int,
    right: Int,
}

const BASE: Int = 0
const NEXT: Int = BASE + 1
const INDEXES: Slots = Slots { left: BASE, right: NEXT }
static EDGE: Int = NEXT
const PICKED: Slots = if NEXT == 1 {
    ALIAS
} else {
    Slots { left: EDGE, right: BASE }
}
static SELECTED_RIGHT: Int = match NEXT {
    1 if PICKED.right == 1 => PICKED.right,
    _ => BASE,
}

fn main() -> Int {
    var pair = (1, 2)
    let left = PICKED.left + 0
    let right = SELECTED_RIGHT - 0
    let first = pair[left] = 7
    let second = pair[right] = first + 6
    let third = pair[if true { 0 } else { 1 }] = first
    let fourth = pair[match 1 {
        1 => 1,
        _ => 0,
    }] = second
    return pair[if true { 0 } else { 1 }] + pair[match 1 {
        1 => 1,
        _ => 0,
    }]
}
