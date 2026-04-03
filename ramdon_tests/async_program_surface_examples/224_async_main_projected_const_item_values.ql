use TOTAL as THRESHOLD
use READY as ENABLED
use BOX as STATE
use FIRST as MATCH_KEY

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
const BRANCHED: Int = if ENABLED && STATE.ready {
    THRESHOLD
} else {
    0
}
static SELECTED: Int = match MATCH_KEY {
    MATCH_KEY if READY => BRANCHED,
    _ => 0,
}

async fn worker(value: Int) -> Int {
    return value
}

async fn main() -> Int {
    if ENABLED && STATE.ready {
        return await worker(SELECTED)
    }
    return 0
}
