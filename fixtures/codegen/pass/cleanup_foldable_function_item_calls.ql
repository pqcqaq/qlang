use first as first_alias
use second as second_alias
use ready as ready_alias
use idle as idle_alias

extern "c" fn first()
extern "c" fn second()

fn ready() -> Bool {
    return true
}

fn idle() -> Bool {
    return false
}

const PICK_BOOL: Bool = true
const PICK_INT: Int = 0

fn main() -> Int {
    defer (if PICK_BOOL {
        first
    } else {
        second
    })()
    defer (match PICK_INT {
        0 => first_alias,
        _ => second_alias,
    })()
    defer if (if PICK_BOOL {
        ready_alias
    } else {
        idle_alias
    })() {
        first()
    } else {
        second()
    }
    return 0
}
