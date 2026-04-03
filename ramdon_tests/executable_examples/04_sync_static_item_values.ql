use TOTAL as THRESHOLD
use READY as ENABLED
use BOX as STATE

struct Payload {
    pair: (Int, Int),
    order: (Int, Int),
    ready: Bool,
}

const BASE: (Int, Int) = (2, 3)
static BOX: Payload = Payload {
    pair: BASE,
    order: (1, 0),
    ready: true,
}
const FIRST: Int = STATE.pair[STATE.order[1]]
static TOTAL: Int = FIRST + BASE[STATE.order[0]]
static READY: Bool = TOTAL > FIRST

fn main() -> Int {
    if ENABLED {
        return THRESHOLD
    }
    return 0
}
