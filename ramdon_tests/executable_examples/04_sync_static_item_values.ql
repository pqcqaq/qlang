use LIMIT as THRESHOLD
use READY as ENABLED
use LIMITS as VALUES

static LIMIT: Int = 2
static READY: Bool = true
static LIMITS: [Int; 3] = [1, 3, 5]

fn main() -> Int {
    let values = VALUES
    let total = THRESHOLD + values[1]
    if ENABLED {
        return total
    }
    return 0
}
